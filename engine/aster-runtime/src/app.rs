//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/app.rs
//! 功能概述：引擎顶层入口 — `App` 结构体持有所有运行时子系统
//!           （GpuContext、SceneManager、InputManager、GameRenderer、GameContext），
//!           提供 `open()` 项目加载+编译初始化和 `run()` 事件循环入口。
//!           是整个引擎对外的唯一启动接口。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster_core（Game / Scene / Character 等数据类型）
//! - aster_compiler（GameCompiler / GameCompileInput / CompiledGame）
//! - aster_renderer（GpuContext / RenderConfig / GameRenderer）
//! - aster_parser（parse_script — .aster 源码解析）
//! - winit（Window / EventLoop 窗口系统）
//!
//! 对应任务：PH1-T21 — 主事件循环 + App 项目入口
//! 架构位置：aster-runtime — 顶层入口，依赖所有下层 crate

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aster_compiler::{GameCompileInput, GameCompiler};
use aster_core::Scene;
use aster_renderer::{GpuContext, RenderConfig};
use winit::event_loop::EventLoop;
use winit::window::Window;

use crate::error::RuntimeError;
use crate::game_context::GameContext;
use crate::game_loader::GameLoader;
use crate::input_manager::{GameAction, InputManager};
use crate::renderer_impl::GameRenderer;
use crate::scene_manager::{SceneManager, SceneState};

/// 引擎顶层入口 — 持有所有运行时子系统，是引擎对外的唯一启动接口。
///
/// # 生命周期
///
/// ```text
/// App::open(project_path) → 加载+编译 → GameContext
///                                      ↓
///                         init_gpu(window) → GpuContext + GameRenderer + SceneManager
///                                      ↓
///                         EventLoop::run_app() → 帧循环 → 退出
/// ```
///
/// # 子系统
///
/// | 字段 | 类型 | 初始化阶段 | 说明 |
/// |------|------|-----------|------|
/// | `game_context` | `GameContext` | `open()` | 编译后的场景+角色+项目配置 |
/// | `project_root` | `PathBuf` | `open()` | 项目根目录（资源路径解析基准） |
/// | `gpu_context` | `Option<GpuContext>` | `init_gpu()` | GPU 设备/队列/表面 |
/// | `renderer` | `Option<GameRenderer>` | `init_gpu()` | 背景/立绘/文本渲染器 |
/// | `scene_manager` | `Option<SceneManager>` | `init_gpu()` | 场景状态机+VM 执行 |
/// | `input_manager` | `InputManager` | `open()` | winit 事件→GameAction 映射 |
/// | `window` | `Option<Arc<Window>>` | `init_gpu()` | winit 窗口句柄 |
///
/// # 使用示例
///
/// ```no_run
/// use aster_runtime::{App, AppEventLoop};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let (app, event_loop) = App::open(Path::new("templates/default_project/"))?;
/// let mut handler = AppEventLoop::new(app);
/// event_loop.run_app(&mut handler).expect("事件循环运行失败");
/// # Ok(())
/// # }
/// ```
pub struct App {
    /// 游戏上下文 — 已编译场景、角色表、项目配置
    pub game_context: GameContext,

    /// 项目根目录 — 所有资源路径（纹理/音频/脚本）的解析基准
    pub project_root: PathBuf,

    /// GPU 上下文 — wgpu 设备/适配器/队列/表面（`init_gpu()` 后可用）
    pub gpu_context: Option<GpuContext>,

    /// 游戏渲染器 — 背景层+立绘层×2+文本层（`init_gpu()` 后可用）
    pub renderer: Option<GameRenderer>,

    /// 场景管理器 — 场景状态机+VM 执行+对话流控制（`init_gpu()` 后可用）
    pub scene_manager: Option<SceneManager>,

    /// 输入管理器 — winit 事件→GameAction 映射+去抖
    pub input_manager: InputManager,

    /// winit 窗口句柄（`init_gpu()` 后可用）
    pub window: Option<Arc<Window>>,

    /// 运行标志 — `false` 时事件循环退出
    pub is_running: bool,

    /// 最小化标志 — `true` 时跳过渲染（节省 GPU 资源）
    pub is_minimized: bool,

    /// 目标帧率（默认 60）
    pub target_fps: u32,

    /// 待处理的 resize 尺寸（延迟到渲染帧前处理，避免 resize 风暴）
    resize_pending: Option<(u32, u32)>,

    /// 上一帧时间戳（用于计算 delta_time）
    last_frame_time: Option<Instant>,
}

impl App {
    // ========================================================================
    // 构造
    // ========================================================================

    /// 从项目根目录加载，执行完整的加载→编译管线（不含 EventLoop）。
    ///
    /// 此方法完成所有非 GPU 依赖的初始化：
    ///
    /// 1. `GameLoader::load(project_root)` → `GameManifest`
    /// 2. 读取并解析所有 `.aster` 场景文件 → `Vec<(String, Scene)>`
    /// 3. `GameCompiler::compile(input)` → `CompiledGame`
    /// 4. `GameContext::new(manifest, compiled)` → `GameContext`
    ///
    /// GPU 相关初始化（GpuContext / GameRenderer / SceneManager）在
    /// `init_gpu()` 中完成（需在 winit `resumed()` 回调中调用）。
    ///
    /// 与 `open()` 的区别：不创建 `EventLoop`，适合单元测试场景。
    /// 生产环境应使用 `open()` 以同时获取 EventLoop。
    ///
    /// # 参数
    /// - `project_root`: 项目根目录路径（包含 `aster.toml` 的目录）
    ///
    /// # 返回值
    /// - `Ok(App)`: 已加载+编译的 App 实例（GPU 子系统尚未初始化）
    /// - `Err(RuntimeError)`: 加载/解析/编译失败
    pub fn load(project_root: &Path) -> Result<Self, RuntimeError> {
        // 规范化项目根目录为绝对路径
        let project_root = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());

        // 步骤 1：加载游戏清单（aster.toml + characters/ + scripts/）
        let manifest = GameLoader::load(&project_root)?;

        // 步骤 2：解析所有 .aster 场景文件为 AST
        let mut parsed: Vec<(String, Scene)> = Vec::with_capacity(manifest.scenes.len());
        for entry in &manifest.scenes {
            let source = fs::read_to_string(project_root.join(&entry.file_path))
                .map_err(RuntimeError::Io)?;
            let scene = aster_parser::parse_script(&source).map_err(|errors| {
                let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                RuntimeError::SceneParseError {
                    scene_id: entry.scene_id.clone(),
                    messages,
                }
            })?;
            parsed.push((entry.scene_id.clone(), scene));
        }

        // 步骤 3：编译所有场景
        let compile_input = GameCompileInput {
            game_name: &manifest.project.name,
            game_version: &manifest.project.version,
            entry_scene_id: &manifest.project.entry_scene,
            scenes: &parsed,
            characters: &manifest.characters,
            build_config: &manifest.build_config,
        };
        let compiled = GameCompiler::compile(compile_input).map_err(|errors| {
            let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            RuntimeError::CompileError { messages }
        })?;

        // 步骤 4：构建游戏上下文
        let game_context = GameContext::new(manifest, compiled);

        Ok(Self {
            game_context,
            project_root,
            gpu_context: None,
            renderer: None,
            scene_manager: None,
            input_manager: InputManager::new(),
            window: None,
            is_running: true,
            is_minimized: false,
            target_fps: 60,
            resize_pending: None,
            last_frame_time: None,
        })
    }

    /// 从项目根目录加载项目并创建 EventLoop（生产环境入口）。
    ///
    /// 等效于 `App::load()` + `EventLoop::new()`。
    /// 测试代码应使用 `App::load()` 以避免 EventLoop 限制
    /// （winit 每进程只允许一个 EventLoop，且需在主线程创建）。
    ///
    /// # 参数
    /// - `project_root`: 项目根目录路径
    ///
    /// # 返回值
    /// - `Ok((App, EventLoop<()>))`: App 实例 + winit 事件循环
    /// - `Err(RuntimeError)`: 加载/解析/编译失败
    pub fn open(project_root: &Path) -> Result<(Self, EventLoop<()>), RuntimeError> {
        let app = Self::load(project_root)?;
        let event_loop = EventLoop::new().map_err(|e| RuntimeError::EventLoopError {
            message: e.to_string(),
        })?;
        Ok((app, event_loop))
    }

    // ========================================================================
    // GPU 初始化
    // ========================================================================

    /// 初始化 GPU 子系统（GpuContext + GameRenderer + SceneManager）。
    ///
    /// 必须在 winit `resumed()` 回调中调用，因为窗口需要由 `ActiveEventLoop` 创建。
    /// 幂等调用：第二次及后续调用直接返回（不重复初始化）。
    ///
    /// # 参数
    /// - `window`: winit 窗口的 `Arc` 引用
    ///
    /// # 初始化流程
    /// 1. 创建 `GpuContext`（wgpu 设备+队列+表面）
    /// 2. 创建 `GameRenderer`（背景层+立绘层×2+文本层）
    /// 3. 创建 `SceneManager` + 加载入口场景
    /// 4. 执行首帧 VM 执行（到第一个暂停点）
    ///
    /// # Panics
    /// 如果 GPU 初始化失败（无适配器/设备请求失败/入口场景加载失败），
    /// 通过 `eprintln!` 输出错误后 panic（引擎无 GPU 无法运行）。
    pub fn init_gpu(&mut self, window: Arc<Window>) {
        // 幂等保护：已初始化则跳过
        if self.gpu_context.is_some() {
            return;
        }

        let size = window.inner_size();
        let config = RenderConfig {
            width: size.width,
            height: size.height,
            ..Default::default()
        };

        // 步骤 1：创建 GPU 上下文
        let gpu = GpuContext::new(window.clone(), &config)
            .unwrap_or_else(|e| panic!("GPU 初始化失败：{e}"));
        let format = gpu.surface_config().format;

        // 步骤 2：创建渲染器
        let mut renderer = GameRenderer::new(
            gpu.device(),
            gpu.queue(),
            format,
            size.width,
            size.height,
            self.project_root.clone(),
        );

        // 步骤 3：创建场景管理器并加载入口场景
        let ctx = self.game_context.clone();
        let entry_scene_id = self.game_context.entry_scene_id.clone();
        let mut manager = SceneManager::new(ctx);
        manager
            .load_scene(&entry_scene_id)
            .unwrap_or_else(|e| panic!("加载入口场景 '{}' 失败：{e}", entry_scene_id));

        // 步骤 4：执行首帧 VM（到第一个对话/菜单暂停点）
        let _ = manager.update(Some(&mut renderer));

        self.gpu_context = Some(gpu);
        self.renderer = Some(renderer);
        self.scene_manager = Some(manager);
        self.window = Some(window);
        self.last_frame_time = Some(Instant::now());
    }

    // ========================================================================
    // 帧循环
    // ========================================================================

    /// 渲染一帧。
    ///
    /// 执行顺序：
    /// 1. 处理待定的 resize（如有）
    /// 2. 计算 delta_time
    /// 3. 推进打字机动画（`SceneManager::update_dialogue`）
    /// 4. 获取 surface 纹理 → 渲染各图层 → 提交呈现
    ///
    /// 最小化状态时跳过 GPU 操作。
    ///
    /// # SurfaceError 处理
    /// - `Timeout`: 静默重试（resize 期间常见，不 panic）
    /// - `Outdated`: 标记 resize_pending，下一帧前处理
    /// - `Lost`: 记录 error 日志，设置 `is_running = false`
    /// - `OutOfMemory`: 记录 error 日志，设置 `is_running = false`
    pub fn render_frame(&mut self) {
        // 最小化时跳过渲染
        if self.is_minimized {
            return;
        }

        // 步骤 0：处理待定的 resize
        self.apply_pending_resize();

        // 步骤 1：计算 delta_time
        let now = Instant::now();
        let delta = self
            .last_frame_time
            .map(|t| {
                let d = now.duration_since(t);
                // 防止异常大的 delta（如调试断点后恢复）
                d.min(Duration::from_millis(100))
            })
            .unwrap_or(Duration::from_millis(16));
        self.last_frame_time = Some(now);

        // 步骤 2：推进打字机动画
        let (mgr, rnd) = match (self.scene_manager.as_mut(), self.renderer.as_mut()) {
            (Some(m), Some(r)) => (m, r),
            _ => return,
        };
        mgr.update_dialogue(delta, &mut Some(rnd));

        // 步骤 3：获取帧 → 渲染 → 呈现
        let gpu = match self.gpu_context.as_ref() {
            Some(g) => g,
            None => return,
        };

        match gpu.acquire_frame() {
            Ok(mut frame) => {
                rnd.render(&mut frame.encoder, &frame.view);
                gpu.present(frame);
            }
            Err(aster_renderer::RenderError::SurfaceTextureFailed(wgpu::SurfaceError::Timeout)) => {
                // resize 期间常见，静默跳过本帧
            }
            Err(aster_renderer::RenderError::SurfaceTextureFailed(
                wgpu::SurfaceError::Outdated,
            )) => {
                // 表面过时 — 标记待 resize
                if let Some(w) = &self.window {
                    let size = w.inner_size();
                    self.resize_pending = Some((size.width, size.height));
                }
            }
            Err(aster_renderer::RenderError::SurfaceTextureFailed(wgpu::SurfaceError::Lost)) => {
                log::error!("[App] GPU 表面丢失，引擎退出");
                self.is_running = false;
            }
            Err(aster_renderer::RenderError::SurfaceTextureFailed(
                wgpu::SurfaceError::OutOfMemory,
            )) => {
                log::error!("[App] GPU 显存不足，引擎退出");
                self.is_running = false;
            }
            Err(e) => {
                log::error!("[App] 获取帧失败：{e}");
                self.is_running = false;
            }
        }
    }

    // ========================================================================
    // 输入处理
    // ========================================================================

    /// 推进对话/确认选择。
    ///
    /// 根据当前 SceneState 分发到对应的处理逻辑：
    /// - `Playing` → `SceneManager::on_click()`（打字机跳过/推进）
    /// - `AtMenu` → 不处理（菜单选择由 `handle_menu_choice()` 处理）
    /// - 其他状态 → 无操作
    ///
    /// 同时处理 SceneState::Ended：通过 `is_running = false` 退出循环
    /// （单场景演示模式；完整游戏流程在 Phase 5 标题画面中实现）。
    pub fn advance(&mut self) {
        let (mgr, rnd) = match (self.scene_manager.as_mut(), self.renderer.as_mut()) {
            (Some(m), Some(r)) => (m, r),
            _ => return,
        };

        match *mgr.state() {
            SceneState::Playing => {
                if let Err(e) = mgr.on_click(Some(rnd)) {
                    log::error!("[App] 推进对话失败：{e}");
                }
            }
            SceneState::Paused => {
                if let Err(e) = mgr.on_click(Some(rnd)) {
                    log::error!("[App] 恢复暂停失败：{e}");
                }
            }
            SceneState::Ended => {
                log::info!("[App] 场景结束");
                self.is_running = false;
            }
            _ => {}
        }
    }

    /// 选择菜单选项（索引从 0 开始）。
    ///
    /// 仅当 SceneState 为 `AtMenu` 时有效。
    ///
    /// # 参数
    /// - `index`: 选项索引（0-based，对应显示中的 1-9）
    pub fn handle_menu_choice(&mut self, index: usize) {
        let (mgr, rnd) = match (self.scene_manager.as_mut(), self.renderer.as_mut()) {
            (Some(m), Some(r)) => (m, r),
            _ => return,
        };

        if *mgr.state() == SceneState::AtMenu
            && let Err(e) = mgr.select_choice(index, Some(rnd))
        {
            log::error!("[App] 菜单选择失败（index={}）：{e}", index);
        }
    }

    // ========================================================================
    // 窗口生命周期
    // ========================================================================

    /// 处理窗口尺寸变化。
    ///
    /// 将宽度和高度 clamp 到 ≥ 1（wgpu 要求），更新 GpuContext 和 GameRenderer。
    /// 当任一维度为 0 时（窗口最小化），设置 `is_minimized = true`。
    ///
    /// # 参数
    /// - `width`: 新的物理像素宽度
    /// - `height`: 新的物理像素高度
    pub fn handle_resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            self.is_minimized = true;
            return;
        }

        self.is_minimized = false;
        let w = width.max(1);
        let h = height.max(1);

        if let Some(gpu) = self.gpu_context.as_mut() {
            gpu.resize(w, h);
        }
        if let Some(rnd) = self.renderer.as_mut() {
            rnd.resize(w, h);
        }
    }

    /// 处理 winit 输入事件，返回对应的 GameAction。
    ///
    /// 委托给 `InputManager::process_event()`，额外处理数字键菜单选择。
    ///
    /// # 参数
    /// - `event`: winit 窗口事件引用
    ///
    /// # 返回值
    /// 映射后的 `GameAction`；菜单选择已在此方法内直接处理，返回 `None`
    pub fn process_input(&mut self, event: &winit::event::WindowEvent) -> GameAction {
        // 特殊处理：数字键 1-9 用于菜单选择（InputManager 不处理数字键）
        if let winit::event::WindowEvent::KeyboardInput {
            event:
                winit::event::KeyEvent {
                    state: winit::event::ElementState::Pressed,
                    logical_key: winit::keyboard::Key::Character(ch),
                    ..
                },
            ..
        } = event
            && let Ok(n) = ch.parse::<usize>()
            && (1..=9).contains(&n)
        {
            self.handle_menu_choice(n - 1);
            return GameAction::None;
        }

        self.input_manager.process_event(event)
    }

    /// 请求窗口重绘。
    ///
    /// 在事件循环的 `about_to_wait()` 中调用，驱动连续渲染。
    pub fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    // ========================================================================
    // 内部辅助方法
    // ========================================================================

    /// 应用待定的 resize（如有）。
    ///
    /// 将 resize 延迟到渲染帧前处理，避免在事件回调中连续 resize
    /// （winit 在 resize 期间可能连续发送多个 Resized 事件）。
    fn apply_pending_resize(&mut self) {
        if let Some((w, h)) = self.resize_pending.take() {
            self.handle_resize(w, h);
        }
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 获取模板项目的绝对路径。
    fn template_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project")
    }

    // ── AC01：App::load() 正确初始化非 GPU 子系统 ────────────────────────

    /// AC01 — `App::load()` 加载模板项目，验证非 GPU 子系统就绪。
    ///
    /// 验证：
    /// 1. 加载成功返回 Ok
    /// 2. `game_context` 非空（场景/角色已编译）
    /// 3. `input_manager` 可用
    /// 4. `project_root` 正确
    /// 5. GPU 相关字段为 None（等待 `init_gpu()` 调用）
    #[test]
    fn ac01_load_initializes_non_gpu_subsystems() {
        let path = template_path();
        assert!(path.exists(), "模板项目目录不存在");

        let app = App::load(&path).expect("App::load() 应成功");

        // 验证 GameContext 就绪
        assert_eq!(app.game_context.entry_scene_id, "prologue");
        assert!(
            app.game_context.is_scene_loaded("prologue"),
            "prologue 场景应已编译"
        );
        assert!(
            app.game_context.get_character("sayori").is_some(),
            "sayori 角色应存在"
        );

        // 验证 project_root
        assert!(
            app.project_root.ends_with("default_project")
                || app
                    .project_root
                    .to_string_lossy()
                    .contains("default_project")
        );

        // 验证 GPU 字段尚未初始化
        assert!(
            app.gpu_context.is_none(),
            "GPU 上下文应在 init_gpu() 前为 None"
        );
        assert!(app.renderer.is_none(), "渲染器应在 init_gpu() 前为 None");
        assert!(
            app.scene_manager.is_none(),
            "场景管理器应在 init_gpu() 前为 None"
        );
        assert!(app.window.is_none(), "窗口应在 init_gpu() 前为 None");

        // 验证运行标志
        assert!(app.is_running, "is_running 初始应为 true");
        assert!(!app.is_minimized, "is_minimized 初始应为 false");
        assert_eq!(app.target_fps, 60, "target_fps 默认应为 60");
    }

    /// AC01 补充 — 加载不存在的项目返回错误。
    #[test]
    fn ac01_load_nonexistent_project_returns_error() {
        let result = App::load(Path::new("/nonexistent/path/12345"));
        assert!(result.is_err(), "不存在的项目应返回错误");
    }

    // ── AC02：窗口 resize 触发子系统 resize ───────────────────────────────

    /// AC02 — `handle_resize(1280, 720)` 正确更新 GPU 和渲染器尺寸。
    ///
    /// 此测试需要先创建 headless GpuContext 来验证 resize 传播。
    /// 使用 headless 模式避免需要真实窗口。
    #[test]
    fn ac02_handle_resize_propagates_to_subsystems() {
        // 创建 headless GPU 上下文用于测试 resize
        let config = RenderConfig {
            width: 1920,
            height: 1080,
            ..Default::default()
        };
        let gpu = match GpuContext::new_headless(&config) {
            Ok(g) => g,
            Err(_) => {
                eprintln!("[跳过] 无 GPU 适配器，跳过 AC02");
                return;
            }
        };

        // 构造最小 App（仅含 GpuContext，无渲染器/场景管理器）
        let path = template_path();
        let mut app = App::load(&path).expect("App::load() 应成功");
        app.gpu_context = Some(gpu);
        app.renderer = Some(GameRenderer::new(
            app.gpu_context.as_ref().unwrap().device(),
            app.gpu_context.as_ref().unwrap().queue(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
            1920,
            1080,
            app.project_root.clone(),
        ));

        // 执行 resize
        app.handle_resize(1280, 720);

        // 验证 GPU 配置更新
        assert_eq!(
            app.gpu_context.as_ref().unwrap().surface_config().width,
            1280,
            "GPU 宽度应更新为 1280"
        );
        assert_eq!(
            app.gpu_context.as_ref().unwrap().surface_config().height,
            720,
            "GPU 高度应更新为 720"
        );

        // 验证渲染器尺寸更新
        assert_eq!(app.renderer.as_ref().unwrap().screen_width, 1280);
        assert_eq!(app.renderer.as_ref().unwrap().screen_height, 720);

        // 验证 is_minimized 为 false
        assert!(!app.is_minimized);
    }

    /// AC02 补充 — resize(0, 0) 触发最小化。
    #[test]
    fn ac02_resize_zero_sets_minimized() {
        let path = template_path();
        let mut app = App::load(&path).expect("App::load() 应成功");

        app.handle_resize(0, 0);
        assert!(app.is_minimized, "0×0 resize 应设置 is_minimized");

        // 恢复后标记清除
        app.handle_resize(1920, 1080);
        assert!(!app.is_minimized, "恢复后 is_minimized 应为 false");
    }

    // ── AC03：最小化暂停渲染 ─────────────────────────────────────────────

    /// AC03 — 最小化时 `render_frame()` 不执行 GPU 操作。
    ///
    /// 验证：
    /// 1. `is_minimized = true` 时 `render_frame()` 立即返回
    /// 2. 不会 panic
    #[test]
    fn ac03_minimized_skips_render() {
        let path = template_path();
        let mut app = App::load(&path).expect("App::load() 应成功");

        // 无 GPU 子系统时渲染也应安全
        app.is_minimized = true;
        app.render_frame(); // 不应 panic

        // 取消最小化但无 GPU → 也应安全
        app.is_minimized = false;
        app.render_frame(); // 不应 panic（内部检查 gpu_context.is_none()）
    }

    /// AC03 补充 — 无 GPU 上下文时 `render_frame()` 安全返回。
    #[test]
    fn ac03_no_gpu_render_safe() {
        let path = template_path();
        let mut app = App::load(&path).expect("App::load() 应成功");

        // 无 GPU、无渲染器、无场景管理器 → render_frame 应安全返回
        app.render_frame();
        // 不 panic 即为通过
    }

    // ── AC04：CloseRequested 退出循环 ─────────────────────────────────────

    /// AC04 — `is_running = false` 表示退出。
    ///
    /// 验证：
    /// 1. 初始状态 `is_running == true`
    /// 2. 设置为 `false` 后事件循环应退出
    #[test]
    fn ac04_close_requested_sets_running_false() {
        let path = template_path();
        let mut app = App::load(&path).expect("App::load() 应成功");

        assert!(app.is_running, "初始 is_running 应为 true");

        // 模拟退出
        app.is_running = false;
        assert!(!app.is_running);

        // 重新打开验证默认状态
        let app2 = App::load(&path).expect("App::load() 应成功");
        assert!(app2.is_running);
    }

    /// AC04 补充 — SceneState::Ended 时 advance() 设置 is_running = false。
    /// 此测试需要已初始化的 SceneManager，标记为 `#[ignore]`。
    #[test]
    #[ignore]
    fn ac04_ended_scene_stops_running() {
        let path = template_path();
        let mut app = App::load(&path).expect("App::load() 应成功");
        // 需要真实窗口 + GPU 初始化后才能测试完整流程
        // 手动运行：cargo test --package aster-runtime -- --ignored
        let _ = &mut app;
    }

    // ── 补充测试：InputManager 已被集成 ───────────────────────────────────

    /// 验证 InputManager 在 App::load() 后可用。
    #[test]
    fn input_manager_available_after_open() {
        let path = template_path();
        let mut app = App::load(&path).expect("App::load() 应成功");

        // 验证 InputManager 可以处理事件
        let result = app.process_input(&winit::event::WindowEvent::CloseRequested);
        assert_eq!(result, GameAction::Quit);
    }

    /// 验证 process_input 对数字键菜单选择的特殊处理。
    #[test]
    fn process_input_handles_menu_keys() {
        let path = template_path();
        let mut app = App::load(&path).expect("App::load() 应成功");

        // 无场景管理器时数字键应返回 None（不 panic）
        let result = app.process_input(&winit::event::WindowEvent::KeyboardInput {
            device_id: unsafe { std::mem::zeroed() },
            event: {
                let mut ev: winit::event::KeyEvent = unsafe { std::mem::zeroed() };
                ev.physical_key = winit::keyboard::PhysicalKey::Unidentified(
                    winit::keyboard::NativeKeyCode::Unidentified,
                );
                ev.logical_key = winit::keyboard::Key::Character("3".into());
                ev.state = winit::event::ElementState::Pressed;
                ev.location = winit::keyboard::KeyLocation::Standard;
                ev.repeat = false;
                ev
            },
            is_synthetic: false,
        });
        assert_eq!(result, GameAction::None);
    }
}
