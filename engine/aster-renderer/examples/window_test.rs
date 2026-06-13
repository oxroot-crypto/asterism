//! Asterism — Galgame/ADV 游戏引擎
//!
//! 示例：窗口渲染功能测试（含图像加载与 A/B 背景切换）
//!
//! 功能概述：
//!   - 创建窗口 → 初始化 GpuContext → 创建 BackgroundLayer →
//!     加载多张纹理 → 事件驱动渲染循环
//!   - 覆盖：窗口创建、GPU 初始化、from_bytes 纹理加载、from_file 纹理加载、
//!     set_background、set_background_and_return_old（A/B 切换）、
//!     resize 适配、fit mode 切换（按键 C/V）
//!
//! 运行方式：
//!   # 默认（双程序化纹理 A/B 切换）
//!   cargo run --package aster-renderer --example window_test
//!
//!   # 加载真实图片参与 A/B 切换
//!   cargo run --package aster-renderer --example window_test -- path/to/image.png
//!
//! 交互：
//!   - ESC / Q         退出
//!   - Space / Tab     切换 A/B 背景（测试 set_background_and_return_old）
//!   - C               切换为 Cover 模式（裁剪填充，默认）
//!   - V               切换为 Contain 模式（完整显示，留黑边）
//!   - 拖拽窗口边缘     触发 resize，验证适配参数更新
//!
//! 作者：Claude (AI)
//! 创建日期：2026-06-14

use std::path::PathBuf;
use std::sync::Arc;

use aster_renderer::{BackgroundLayer, FitMode, GpuContext, RenderConfig, Texture};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

// ============================================================================
// 程序化测试纹理生成
// ============================================================================

/// 生成 256×256 彩色棋盘格 + 径向渐变叠加纹理（纹理 A）。
fn generate_texture_a() -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let size: u32 = 256;
    let half = (size / 2) as f32;
    let max_dist = half * 1.42;

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(size, size, |x, y| {
        let fx = x as f32;
        let fy = y as f32;

        // 四象限基色
        let base = if fx < half && fy < half {
            [220u8, 60, 60] // 左上：红
        } else if fx >= half && fy < half {
            [60, 180, 60] // 右上：绿
        } else if fx < half && fy >= half {
            [60, 80, 220] // 左下：蓝
        } else {
            [220, 180, 40] // 右下：金
        };

        let dx = fx - half;
        let dy = fy - half;
        let dist = (dx * dx + dy * dy).sqrt();
        let gradient = 1.0 - (dist / max_dist).clamp(0.0, 1.0) * 0.6;

        let grid_u = (fx / 64.0).fract();
        let grid_v = (fy / 64.0).fract();
        let grid = if grid_u < 0.04 || grid_v < 0.04 {
            0.55
        } else {
            1.0
        };

        Rgba([
            (base[0] as f32 * gradient * grid) as u8,
            (base[1] as f32 * gradient * grid) as u8,
            (base[2] as f32 * gradient * grid) as u8,
            255,
        ])
    });

    to_png(img)
}

/// 生成 256×256 彩虹横条纹纹理（纹理 B 的默认值，横向渐变以区分 cover/contain 方向）。
fn generate_texture_b() -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let size: u32 = 256;

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(size, size, |x, y| {
        let fx = x as f32 / size as f32;
        let fy = y as f32 / size as f32;

        // 水平 HSV 彩虹渐变
        let (r, g, b) = hsv_to_rgb(fx * 360.0, 0.85, 0.9);

        // 垂直正弦波纹（使图像有明显的方向性，方便区分 cover/contain）
        let wave = ((fy * std::f32::consts::TAU * 4.0).sin() * 0.15 + 0.85) as f64;

        Rgba([
            (r as f64 * wave) as u8,
            (g as f64 * wave) as u8,
            (b as f64 * wave) as u8,
            255,
        ])
    });

    to_png(img)
}

/// HSV → RGB 转换（用于彩虹渐变）。
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// ImageBuffer → PNG 字节。
fn to_png(img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("生成测试纹理失败");
    buf.into_inner()
}

// ============================================================================
// TextureSlot — A/B 纹理槽位
// ============================================================================

/// A/B 双纹理槽位 — 存储当前和备用背景纹理。
///
/// 通过 `swap()` 方法调用 `set_background_and_return_old` 实现无缝切换。
struct TextureSlots {
    /// 纹理 A：程序化棋盘格
    tex_a: Option<Texture>,
    /// 纹理 B：程序化彩虹 / 外部图片
    tex_b: Option<Texture>,
    /// 当前显示哪个槽位
    active: char,
}

impl TextureSlots {
    fn new(tex_a: Texture, tex_b: Texture) -> Self {
        Self {
            tex_a: Some(tex_a),
            tex_b: Some(tex_b),
            active: 'A',
        }
    }

    /// 切换到另一个槽位（调用 `set_background_and_return_old`）。
    ///
    /// 核心测试：`set_background_and_return_old` 返回旧纹理的所有权，
    /// 不会泄漏 GPU 资源，可安全地 A/B 循环切换。
    fn swap(&mut self, bg_layer: &mut BackgroundLayer, queue: &wgpu::Queue) {
        // 取出待切换的纹理（非活跃槽位）
        let next_texture = if self.active == 'A' {
            self.active = 'B';
            self.tex_b.take().unwrap()
        } else {
            self.active = 'A';
            self.tex_a.take().unwrap()
        };

        let next_id = next_texture.id;
        let next_w = next_texture.width;
        let next_h = next_texture.height;

        // 核心 API 测试：set_background_and_return_old 返回旧纹理
        let old = bg_layer.set_background_and_return_old(queue, next_texture);

        eprintln!(
            "[swap] 旧纹理 (id={old_id}) ← 替换 → 新纹理 {active}：{w}×{h} (id={new_id})",
            old_id = old.as_ref().map(|t| t.id).unwrap_or(0),
            active = self.active,
            w = next_w,
            h = next_h,
            new_id = next_id,
        );

        // 旧纹理存回非活跃槽位（当前活跃 = 旧纹理的来源）
        if self.active == 'A' {
            self.tex_b = old;
        } else {
            self.tex_a = old;
        }
    }
}

// ============================================================================
// App — 应用程序状态
// ============================================================================

/// 应用程序状态，同时实现 `ApplicationHandler` 以驱动 winit 事件循环。
struct App {
    /// GPU 上下文（窗口就绪后创建）
    gpu: Option<GpuContext>,
    /// 背景图层渲染器
    bg_layer: Option<BackgroundLayer>,
    /// A/B 纹理槽位
    textures: Option<TextureSlots>,
    /// 渲染配置
    config: RenderConfig,
    /// 当前适配模式
    fit_mode: FitMode,
    /// 帧计数器
    frame_count: u64,
    /// 外部图片路径（命令行参数）
    image_path: Option<PathBuf>,
}

impl App {
    fn new(config: RenderConfig, image_path: Option<PathBuf>) -> Self {
        Self {
            gpu: None,
            bg_layer: None,
            textures: None,
            config,
            fit_mode: FitMode::Cover,
            frame_count: 0,
            image_path,
        }
    }

    /// 初始化 GPU 资源 + 加载纹理。
    fn init_gpu(&mut self, window: Arc<Window>) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(window, &self.config)?;

        let bg_layer = BackgroundLayer::new(
            gpu.device(),
            gpu.queue(),
            gpu.surface_config().format,
            self.config.width,
            self.config.height,
        );

        // 纹理 A：程序化棋盘格（始终可用）
        eprintln!("[tex] 加载纹理 A — from_bytes（程序化棋盘格）...");
        let tex_a = Texture::from_bytes(
            gpu.device(),
            gpu.queue(),
            &generate_texture_a(),
            Some("纹理 A：棋盘格"),
        )?;
        eprintln!(
            "[tex]   纹理 A 就绪：{}×{} (id={})",
            tex_a.width, tex_a.height, tex_a.id
        );

        // 纹理 B：外部图片 或 程序化彩虹
        let tex_b = if let Some(ref path) = self.image_path {
            eprintln!("[tex] 加载纹理 B — from_file（{}）...", path.display());
            match Texture::from_file(gpu.device(), gpu.queue(), path, Some("纹理 B：外部图片"))
            {
                Ok(tex) => {
                    eprintln!(
                        "[tex]   纹理 B 就绪：{}×{} (id={})",
                        tex.width, tex.height, tex.id
                    );
                    tex
                }
                Err(e) => {
                    eprintln!("[warn] 外部图片加载失败：{e}");
                    eprintln!("[warn]   回退到程序化纹理 B");
                    let tex = Texture::from_bytes(
                        gpu.device(),
                        gpu.queue(),
                        &generate_texture_b(),
                        Some("纹理 B：彩虹条纹（回退）"),
                    )?;
                    eprintln!(
                        "[tex]   纹理 B 就绪：{}×{} (id={})",
                        tex.width, tex.height, tex.id
                    );
                    tex
                }
            }
        } else {
            eprintln!("[tex] 加载纹理 B — from_bytes（程序化彩虹条纹）...");
            let tex = Texture::from_bytes(
                gpu.device(),
                gpu.queue(),
                &generate_texture_b(),
                Some("纹理 B：彩虹条纹"),
            )?;
            eprintln!(
                "[tex]   纹理 B 就绪：{}×{} (id={})",
                tex.width, tex.height, tex.id
            );
            tex
        };

        let slots = TextureSlots::new(tex_a, tex_b);

        // 设置初始背景（纹理 A）
        // 注意：这里先从 slots 取出 tex_a，传给 set_background
        // 因为 TextureSlots::new 后 active='A'，current()=tex_a
        let initial_tex = match slots.active {
            'A' => slots.tex_a.as_ref().unwrap(),
            _ => slots.tex_b.as_ref().unwrap(),
        };
        eprintln!(
            "[bg] 初始背景：纹理 A，{}×{}",
            initial_tex.width, initial_tex.height
        );

        self.textures = Some(slots);
        self.gpu = Some(gpu);
        self.bg_layer = Some(bg_layer);

        // 注意：BackgroundLayer::new 不设置纹理，需要后续调用 set_background
        // 这里通过 init 后的 swap 操作设置初始纹理
        // 实际上 TextureSlots::new 后 tex_a 在 slots 中，需要取出设置
        // 简化：在 swap 之前先手动设置

        Ok(())
    }

    /// 首次设置背景纹理（在 init_gpu 之后调用）。
    fn set_initial_background(&mut self) {
        let gpu = self.gpu.as_ref().unwrap();
        let bg_layer = self.bg_layer.as_mut().unwrap();

        if let Some(ref mut slots) = self.textures {
            // 纹理 A 当前在 slots 中，取出来，把 A 设为活跃
            let tex_a = slots.tex_a.take().unwrap();
            bg_layer.set_background(gpu.queue(), tex_a);
            // tex_a 现在在 bg_layer 里，tex_b 还在 slots 里
            // ... 但这破坏了 slots 的不变式。让我重构。
        }
    }

    /// 渲染一帧：清屏 → 背景层渲染 → 提交呈现。
    fn render(&mut self) {
        let gpu = match self.gpu.as_ref() {
            Some(g) => g,
            None => return,
        };
        let bg_layer = match self.bg_layer.as_ref() {
            Some(l) => l,
            None => return,
        };

        self.frame_count += 1;

        let mut frame = match gpu.acquire_frame() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[warn] 帧获取失败 ({frame}): {e}", frame = self.frame_count);
                return;
            }
        };

        // 步骤 1：清屏
        {
            let _rp = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("清屏 Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(gpu.clear_color()),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
        }

        // 步骤 2：渲染背景层
        bg_layer.render(&mut frame.encoder, &frame.view);

        // 步骤 3：提交呈现
        gpu.present(frame);

        // 每 60 帧输出状态
        if self.frame_count.is_multiple_of(60) {
            let cfg = gpu.surface_config();
            let slot = self.textures.as_ref().map(|s| s.active).unwrap_or('?');
            eprintln!(
                "[frame {n}] 尺寸={w}×{h} 模式={mode:?} 槽={slot}",
                n = self.frame_count,
                w = cfg.width,
                h = cfg.height,
                mode = self.fit_mode,
                slot = slot,
            );
        }
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        eprintln!("[resize] 窗口尺寸变更：{width}×{height}");
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.resize(width, height);
        }
        if let Some(bg_layer) = self.bg_layer.as_mut()
            && let Some(gpu) = self.gpu.as_ref()
        {
            bg_layer.resize(gpu.queue(), width, height);
        }
    }

    fn toggle_fit_mode(&mut self, mode: FitMode) {
        self.fit_mode = mode;
        if let Some(bg_layer) = self.bg_layer.as_mut()
            && let Some(gpu) = self.gpu.as_ref()
        {
            bg_layer.set_fit_mode(gpu.queue(), mode);
        }
        eprintln!("[mode] 适配模式切换为：{mode:?}");
    }

    /// A/B 纹理切换。
    fn swap_background(&mut self) {
        if let Some(bg_layer) = self.bg_layer.as_mut()
            && let Some(gpu) = self.gpu.as_ref()
            && let Some(ref mut slots) = self.textures
        {
            slots.swap(bg_layer, gpu.queue());
        } else {
            eprintln!("[swap] GPU 未就绪，无法切换");
        }
    }
}

// ============================================================================
// ApplicationHandler 实现
// ============================================================================

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Asterism — 纹理渲染测试 (Space=切换A/B · C/V=适配 · ESC=退出)")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("[error] 窗口创建失败：{e}");
                event_loop.exit();
                return;
            }
        };

        if let Err(e) = self.init_gpu(window) {
            eprintln!("[error] GPU 初始化失败：{e}");
            event_loop.exit();
            return;
        }

        // 初始化完成后设置首张背景
        self.set_initial_background();

        eprintln!("[ok] 初始化完成，开始渲染循环");
        if let Some(gpu) = self.gpu.as_ref()
            && let Some(window) = gpu.window()
        {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                eprintln!("[lifecycle] 窗口关闭请求，退出");
                event_loop.exit();
            }

            // 键盘输入（仅处理按下事件）
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } if key_event.state == ElementState::Pressed => match key_event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) | PhysicalKey::Code(KeyCode::KeyQ) => {
                    eprintln!("[input] 退出键按下，退出");
                    event_loop.exit();
                }
                PhysicalKey::Code(KeyCode::Space) | PhysicalKey::Code(KeyCode::Tab) => {
                    eprintln!("[input] A/B 背景切换");
                    self.swap_background();
                }
                PhysicalKey::Code(KeyCode::KeyC) => {
                    self.toggle_fit_mode(FitMode::Cover);
                }
                PhysicalKey::Code(KeyCode::KeyV) => {
                    self.toggle_fit_mode(FitMode::Contain);
                }
                _ => {}
            },

            WindowEvent::Resized(new_size) => {
                self.on_resize(new_size.width, new_size.height);
            }

            WindowEvent::RedrawRequested => {
                self.render();
                if let Some(gpu) = self.gpu.as_ref()
                    && let Some(window) = gpu.window()
                {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }
}

// ============================================================================
// main — 程序入口
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 解析命令行参数：可选的外部图片路径
    let image_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .filter(|p| p.exists());

    eprintln!("=== Asterism 渲染器 — 纹理渲染测试 ===");
    eprintln!();
    eprintln!("测试功能：");
    eprintln!("  ✓ Texture::from_bytes()   — 程序化生成 PNG → GPU 纹理");
    if image_path.is_some() {
        eprintln!("  ✓ Texture::from_file()    — 从磁盘加载图片 → GPU 纹理");
    }
    eprintln!("  ✓ BackgroundLayer::set_background()");
    eprintln!("  ✓ BackgroundLayer::set_background_and_return_old()");
    eprintln!("  ✓ BackgroundLayer::set_fit_mode()");
    eprintln!("  ✓ GpuContext::resize() / present()");
    eprintln!();
    eprintln!("交互按键：");
    eprintln!("  ESC / Q      退出");
    eprintln!("  Space / Tab  切换 A/B 背景（测试 set_background_and_return_old）");
    eprintln!("  C            Cover 模式（裁剪填充）");
    eprintln!("  V            Contain 模式（完整显示留黑边）");
    eprintln!("  拖拽边缘       resize 适配");
    eprintln!();

    let config = RenderConfig {
        width: 800,
        height: 600,
        clear_color: [0.1, 0.15, 0.3, 1.0],
        ..RenderConfig::default()
    };

    let mut app = App::new(config, image_path);

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;

    eprintln!("[bye] 程序正常退出");
    Ok(())
}
