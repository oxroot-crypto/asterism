//! Asterism — 游戏引擎运行时入口
//!
//! 文件路径：engine/aster-runtime/src/main.rs
//! 功能概述：引擎二进制入口 — 接受 `--project` 参数指定项目路径，
//!           加载并运行视觉小说项目。Phase 2 集成所有子系统（音频/资源/存档）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//!
//! ## 用法
//!
//! ```bash
//! aster-runtime --project templates/default_project
//! ```
//!
//! ## 操作
//!
//! | 按键 | 功能 |
//! |------|------|
//! | Enter/Space/鼠标左键 | 推进 |
//! | 数字1-9 | 选择菜单 |
//! | F5 | 快速存档 |
//! | F9 | 快速读档 |
//! | Esc | 退出 |

use std::path::{Path, PathBuf};

use aster_runtime::{App, AppEventLoop};

fn main() {
    let project_path = parse_args();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           Asterism 群星引擎 — Phase 2 v0.2.0-alpha          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  🖱左键/Enter/Space=推进  ⌨数字1-9=选择  F5=存档 F9=读档   ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  项目: {}", project_path.display());

    match App::open(&project_path) {
        Ok((app, event_loop)) => {
            println!("  引擎已就绪，音频/资源/存档子系统已初始化");
            let mut handler = AppEventLoop::new(app);
            event_loop.run_app(&mut handler).expect("事件循环异常退出");
        }
        Err(e) => {
            eprintln!("❌ 加载项目失败: {}", e);
            eprintln!("  请确认项目路径正确且包含 aster.toml 文件");
            std::process::exit(1);
        }
    }

    println!("  引擎已正常退出");
}

/// 解析命令行参数，提取 `--project <path>`。
fn parse_args() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--project" | "-p" => {
                i += 1;
                if i < args.len() {
                    return PathBuf::from(&args[i]);
                }
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    // 默认：相对于可执行文件的模板项目路径
    let default = Path::new(env!("CARGO_MANIFEST_DIR")).join("../templates/default_project");
    eprintln!("⚠ 未指定 --project，使用默认项目: {}", default.display());
    default
}

fn print_help() {
    println!("Asterism 群星引擎 — Phase 2 v0.2.0-alpha");
    println!();
    println!("用法: aster-runtime [选项]");
    println!();
    println!("选项:");
    println!("  --project, -p <PATH>  指定项目根目录路径");
    println!("  --help, -h            显示此帮助信息");
}
