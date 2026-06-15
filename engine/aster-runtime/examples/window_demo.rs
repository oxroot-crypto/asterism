//! Asterism — 引擎集成演示（PH1-T19 DialogueController 集成）
//!
//! 运行：cargo run --package aster-runtime --example window_demo
//! 操作：鼠标左键/Enter/Space=推进（打字中=跳过，完成=下一句）  数字1-9=选择  Esc=退出

use std::fs;
use std::path::Path;
use std::sync::Arc;

use aster_compiler::{GameCompileInput, GameCompiler};
use aster_core::Scene;
use aster_renderer::{GpuContext, RenderConfig};
use aster_runtime::{
    GameContext, GameLoader, GameRenderer,
    scene_manager::{SceneManager, SceneState},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes},
};

struct App {
    gpu: Option<GpuContext>,
    renderer: Option<GameRenderer>,
    manager: Option<SceneManager>,
    window: Option<Arc<Window>>,
    resize_pending: bool,
}

impl App {
    fn new() -> Self {
        Self {
            gpu: None,
            renderer: None,
            manager: None,
            window: None,
            resize_pending: false,
        }
    }

    fn init(&mut self, window: Arc<Window>, el: &ActiveEventLoop) {
        let size = window.inner_size();
        let config = RenderConfig {
            width: size.width,
            height: size.height,
            ..Default::default()
        };
        let gpu = GpuContext::new(window.clone(), &config).expect("GPU 初始化失败");
        let format = gpu.surface_config().format;

        let project_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project");

        let mut renderer = GameRenderer::new(
            gpu.device(),
            gpu.queue(),
            format,
            size.width,
            size.height,
            project_path.clone(),
        );
        let manifest = GameLoader::load(&project_path).expect("加载项目失败");
        let mut parsed: Vec<(String, Scene)> = Vec::new();
        for entry in &manifest.scenes {
            let source = fs::read_to_string(project_path.join(&entry.file_path)).expect("读取失败");
            parsed.push((
                entry.scene_id.clone(),
                aster_parser::parse_script(&source).expect("解析失败"),
            ));
        }
        let input = GameCompileInput {
            game_name: &manifest.project.name,
            game_version: &manifest.project.version,
            entry_scene_id: &manifest.project.entry_scene,
            scenes: &parsed,
            characters: &manifest.characters,
            build_config: &manifest.build_config,
        };
        let compiled = GameCompiler::compile(input).expect("编译失败");
        let ctx = GameContext::new(manifest, compiled);
        let mut manager = SceneManager::new(ctx);
        manager.load_scene("prologue").expect("加载 prologue 失败");

        println!("╔═══════════════════════════════════════════════════════╗");
        println!(
            "║   Asterism 引擎演示 — {}×{}                         ║",
            size.width, size.height
        );
        println!("╠═══════════════════════════════════════════════════════╣");
        println!("║  🖱左键/Enter/Space=推进  ⌨1-9=选择菜单  Esc=退出    ║");
        println!("╚═══════════════════════════════════════════════════════╝");

        // 执行首帧（到第一个对话暂停点）
        let _ = manager.update(Some(&mut renderer));

        self.gpu = Some(gpu);
        self.renderer = Some(renderer);
        self.manager = Some(manager);
        self.window = Some(window);
        el.set_control_flow(ControlFlow::Poll);
    }

    fn advance(&mut self) {
        let (Some(mgr), Some(rnd)) = (&mut self.manager, &mut self.renderer) else {
            return;
        };
        match *mgr.state() {
            SceneState::Playing => {
                // 委托给 DialogueController：
                // - 打字机进行中 → 跳过动画（typewriter.skip()）
                // - 打字机完成 → 推进 VM 到下一句
                mgr.on_click(Some(rnd)).ok();
                print_state(mgr);
            }
            SceneState::Paused => {
                mgr.on_click(Some(rnd)).ok();
            }
            _ => {}
        }
    }

    fn choose(&mut self, idx: usize) {
        let (Some(mgr), Some(rnd)) = (&mut self.manager, &mut self.renderer) else {
            return;
        };
        if *mgr.state() == SceneState::AtMenu {
            mgr.select_choice(idx, Some(rnd)).ok();
            print_state(mgr);
        }
    }

    fn render_frame(&mut self) {
        if self.resize_pending
            && let (Some(gpu), Some(rnd), Some(w)) =
                (&mut self.gpu, &mut self.renderer, &self.window)
        {
            let size = w.inner_size();
            if size.width > 0 && size.height > 0 {
                gpu.resize(size.width, size.height);
                rnd.resize(size.width, size.height);
                self.resize_pending = false;
            }
        }

        // 每帧推进打字机动画（DialogueController → Renderer 可见范围同步）
        if let (Some(mgr), Some(rnd)) = (&mut self.manager, &mut self.renderer) {
            mgr.update_dialogue(
                std::time::Duration::from_millis(16), // ~60fps
                &mut Some(rnd),
            );
        }

        let (Some(gpu), Some(rnd)) = (&mut self.gpu, &mut self.renderer) else {
            return;
        };
        match gpu.acquire_frame() {
            Ok(mut frame) => {
                rnd.render(&mut frame.encoder, &frame.view);
                gpu.present(frame);
            }
            Err(_) => {
                // resize 中会连续失败，忽略
            }
        }
    }
}

fn print_state(mgr: &SceneManager) {
    match *mgr.state() {
        SceneState::Ended => println!("[Demo] 场景结束 🎉"),
        SceneState::AtMenu => println!(
            "[Demo] 📋 菜单: \"{}\" ({}选项, 按1-{}选择)",
            mgr.menu_prompt().unwrap_or("?"),
            mgr.menu_choice_count().unwrap_or(0),
            mgr.menu_choice_count().unwrap_or(0)
        ),
        _ => {}
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Asterism — 群星引擎演示")
                        .with_inner_size(winit::dpi::LogicalSize::new(1920.0, 1080.0)),
                )
                .unwrap(),
        );
        self.init(window, event_loop);
    }

    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        _: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(_) => {
                self.resize_pending = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.advance(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: key,
                        ..
                    },
                ..
            } => match key {
                Key::Named(NamedKey::Escape) => el.exit(),
                Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => self.advance(),
                Key::Character(ref ch) => {
                    if let Ok(n) = ch.parse::<usize>()
                        && (1..=9).contains(&n)
                    {
                        self.choose(n - 1);
                    }
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                self.render_frame();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn main() {
    EventLoop::new()
        .expect("EventLoop 创建失败")
        .run_app(&mut App::new())
        .expect("EventLoop 运行失败");
}
