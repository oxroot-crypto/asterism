//! Asterism — 引擎集成演示（PH2-T08: 运行时集成）
//!
//! 运行：cargo run --package aster-runtime --example window_demo
//!
//! ## Phase 2 功能验证
//!
//! 本 demo 集成了 Phase 2 全部子系统（音频/资源/存档），
//! 可用于人工验证 PH2-T01 ~ PH2-T08 的所有 MV（人工测试验证项）。
//!
//! ## 操作说明
//!
//! | 按键 | 功能 |
//! |------|------|
//! | 🖱左键 / Enter / Space | 推进对话（打字中=跳过，完成=下一句） |
//! | 数字 1-9 | 选择菜单选项 |
//! | F5 | 快速存档（保存到槽位 98） |
//! | F9 | 快速读档（从槽位 98 恢复） |
//! | Esc | 退出引擎 |
//!
//! ## 验证清单
//!
//! - PH2-T01 MV01/MV02: 场景开始后 BGM 应自动播放且循环流畅
//! - PH2-T02 MV01/MV02: SE 与 BGM 同时发声，互不干扰
//! - PH2-T03 MV01/MV02: BGM fade_in/fade_out 过渡平滑
//! - PH2-T04 MV01/MV02: 资源正确加载，背景/立绘正常显示
//! - PH2-T05 MV01/MV02: 场景切换时缓存复用，内存稳定
//! - PH2-T06 MV01-MV03: 存档文件正确生成，读档完整恢复，损坏检测
//! - PH2-T07 MV01-MV06: 存档 UI 各项操作（需通过脚本调用）
//! - PH2-T08 MV01-MV07: 完整集成流程

use std::path::Path;

use aster_runtime::{App, AppEventLoop};

fn main() {
    // 定位模板项目目录
    let project_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project");

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Asterism 引擎集成演示 — Phase 2 运行时集成验证         ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Phase 2 子系统: 音频(BGM+SE+fade) + 资源(LRU缓存) + 存档   ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  🖱左键/Enter/Space = 推进    ⌨数字1-9 = 选择菜单           ║");
    println!("║  F5 = 快速存档    F9 = 快速读档    Esc = 退出               ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  项目路径: {}", project_path.display());
    println!("  存档目录: {}/save/", project_path.display());
    println!();

    // 加载项目（解析+编译 → GameContext → 子系统初始化）
    let (app, event_loop) = App::open(&project_path).expect("加载项目失败");

    println!("  ✓ 项目加载完成，音频/资源/存档子系统已初始化");
    println!("  ✓ 按 Enter 开始体验...");
    println!();

    // 启动事件循环
    let mut handler = AppEventLoop::new(app);
    event_loop.run_app(&mut handler).expect("事件循环运行失败");

    println!();
    println!("  引擎已正常退出。感谢使用 Asterism Phase 2 集成演示！");
}
