//! Asterism — 引擎集成演示（PH1-T21 主事件循环 + App 入口）
//!
//! 运行：cargo run --package aster-runtime --example window_demo
//! 操作：鼠标左键/Enter/Space=推进（打字中=跳过，完成=下一句）  数字1-9=选择菜单  Esc=退出
//! 架构：使用 `App::open()` 加载项目 + `AppEventLoop` 驱动帧循环

use std::path::Path;

use aster_runtime::{App, AppEventLoop};

fn main() {
    // 定位模板项目目录
    let project_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project");

    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║        Asterism 引擎演示 — PH1-T21 主事件循环         ║");
    println!("╠═══════════════════════════════════════════════════════╣");
    println!("║  🖱左键/Enter/Space=推进  ⌨数字1-9=选择菜单  Esc=退出 ║");
    println!("╚═══════════════════════════════════════════════════════╝");

    // 加载项目（解析+编译 → GameContext）
    let (app, event_loop) = App::open(&project_path).expect("加载项目失败");

    // 启动事件循环（winit resumed → init_gpu → 帧循环 → 退出）
    let mut handler = AppEventLoop::new(app);
    event_loop.run_app(&mut handler).expect("事件循环运行失败");
}
