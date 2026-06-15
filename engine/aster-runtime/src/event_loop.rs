//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/event_loop.rs
//! 功能概述：事件循环处理器 — 实现 winit `ApplicationHandler` trait，
//!           桥接 winit 窗口事件与 `App` 的子系统调度。
//!           负责帧循环的 delta_time 计算、渲染触发、窗口生命周期管理。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - winit（ApplicationHandler / Event / WindowEvent / ActiveEventLoop / ControlFlow）
//! - crate::app::App（持有所有子系统）
//! - crate::input_manager::GameAction（事件→动作映射结果）
//!
//! 对应任务：PH1-T21 — 主事件循环 + App 项目入口
//! 架构位置：aster-runtime — 驱动 App 各子系统按帧循环运行

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowAttributes;

use crate::app::App;
use crate::input_manager::GameAction;

/// 事件循环处理器 — 实现 winit `ApplicationHandler` trait。
///
/// 持有 `App` 实例（包含所有引擎子系统），在 winit 事件回调中
/// 驱动帧循环的各个环节：输入处理 → 状态更新 → 渲染 → 呈现。
///
/// # 生命周期
///
/// ```text
/// EventLoop::new()
///   → ApplicationHandler::resumed()     — 创建窗口 + App::init_gpu()
///   → ApplicationHandler::window_event() — 每事件：输入/Resize/Redraw/Close
///   → ApplicationHandler::about_to_wait() — request_redraw() 驱动连续渲染
/// ```
///
/// # 帧循环模型
///
/// 使用 `ControlFlow::Poll`（连续轮询）+ `Window::request_redraw()` 维持 60fps。
/// 每收到 `RedrawRequested` 事件时执行一帧渲染。
///
/// # 使用示例
///
/// ```no_run
/// use aster_runtime::{App, AppEventLoop};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let (app, event_loop) = App::open(Path::new("my_game/"))?;
/// let mut handler = AppEventLoop::new(app);
/// event_loop.run_app(&mut handler).expect("事件循环运行失败");
/// # Ok(())
/// # }
/// ```
pub struct EventLoop {
    /// 引擎 App 实例 — `resumed()` 回调前 GPU 相关字段为 None（窗口尚未创建）
    app: Option<App>,
}

impl EventLoop {
    /// 创建事件循环处理器。
    ///
    /// # 参数
    /// - `app`: 由 `App::open()` 返回的已初始化 App（非 GPU 部分已就绪）
    pub fn new(app: App) -> Self {
        Self { app: Some(app) }
    }

    /// 消费处理器，返回内部 App（如果事件循环异常退出时可恢复状态）。
    ///
    /// # 返回值
    /// - `Some(App)`: App 实例（如事件循环尚未启动或已退出）
    /// - `None`: App 已被消费
    #[allow(dead_code)]
    pub fn into_app(mut self) -> Option<App> {
        self.app.take()
    }
}

impl ApplicationHandler for EventLoop {
    // ========================================================================
    // resumed — 窗口创建/恢复事件
    // ========================================================================

    /// 应用首次启动或从挂起恢复时调用。
    ///
    /// 在此处创建 winit Window 并调用 `App::init_gpu()` 完成 GPU 初始化。
    /// 幂等：第二次及后续调用直接返回（窗口已存在）。
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let app = match self.app.as_mut() {
            Some(a) => a,
            None => return,
        };

        // 已初始化则跳过（恢复事件）
        if app.gpu_context.is_some() {
            return;
        }

        // 步骤 1：创建窗口
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Asterism — 群星引擎")
                        .with_inner_size(winit::dpi::LogicalSize::new(
                            app.game_context.resolution.0 as f64,
                            app.game_context.resolution.1 as f64,
                        )),
                )
                .expect("创建窗口失败"),
        );

        // 步骤 2：初始化 GPU 子系统
        app.init_gpu(window);

        // 步骤 3：设置控制流为连续轮询（60fps）
        event_loop.set_control_flow(ControlFlow::Poll);

        log::info!(
            "[EventLoop] 引擎初始化完成 — {}×{} — 入口场景: {}",
            app.game_context.resolution.0,
            app.game_context.resolution.1,
            app.game_context.entry_scene_id,
        );
    }

    // ========================================================================
    // window_event — 窗口事件分发
    // ========================================================================

    /// 处理所有窗口事件。
    ///
    /// 事件处理优先级：
    /// 1. `Resized` — 更新 GpuContext + GameRenderer 尺寸
    /// 2. `RedrawRequested` — 渲染一帧
    /// 3. `CloseRequested` — 退出事件循环
    /// 4. 其他 → `InputManager::process_event()` → dispatch GameAction
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let app = match self.app.as_mut() {
            Some(a) => a,
            None => return,
        };

        match &event {
            // ── 窗口尺寸变化 ──────────────────────────────────────
            WindowEvent::Resized(physical_size) => {
                app.handle_resize(physical_size.width, physical_size.height);
                return;
            }

            // ── 重绘请求 — 渲染一帧 ───────────────────────────────
            WindowEvent::RedrawRequested => {
                app.render_frame();

                // 检查是否需要退出（render_frame 可能因 SurfaceLost 设置 is_running=false）
                if !app.is_running {
                    event_loop.exit();
                    return;
                }

                // 继续请求下一帧
                app.request_redraw();
                return;
            }

            // ── 窗口关闭 ──────────────────────────────────────────
            WindowEvent::CloseRequested => {
                app.is_running = false;
                event_loop.exit();
                return;
            }

            _ => {}
        }

        // ── 通过 InputManager 处理输入事件 ─────────────────────────
        let action = app.process_input(&event);
        match action {
            GameAction::Advance => {
                app.advance();
                // 检查是否是场景结束导致的退出
                if !app.is_running {
                    event_loop.exit();
                }
            }
            GameAction::Quit | GameAction::OpenMenu => {
                app.is_running = false;
                event_loop.exit();
            }
            GameAction::Skip
            | GameAction::Auto
            | GameAction::QuickSave
            | GameAction::QuickLoad
            | GameAction::ToggleFullscreen => {
                // Phase 1 预留，后续 Phase 实现
                log::debug!("[EventLoop] 未实现的 GameAction: {:?}", action);
            }
            GameAction::None => {}
        }
    }

    // ========================================================================
    // about_to_wait — 空闲时触发
    // ========================================================================

    /// 事件队列清空、即将进入等待时调用。
    ///
    /// 请求下一次重绘以维持连续渲染循环。
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(app) = self.app.as_ref() {
            app.request_redraw();
        }
    }
}
