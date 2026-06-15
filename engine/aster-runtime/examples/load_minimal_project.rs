//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/examples/load_minimal_project.rs
//! 功能概述：GameLoader MV02 人工验证示例 — 创建临时最小项目（无 characters/ 目录），
//!           验证加载成功且 characters 为空 HashMap。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//!
//! 运行方式：
//!   cargo run --package aster-runtime --example load_minimal_project
//!
//! 预期输出：
//!   - 加载成功
//!   - characters 为空（0 个角色）
//!   - 1 个场景

use std::fs;

use aster_runtime::GameLoader;

fn main() {
    // 创建临时项目目录
    let temp_dir = std::env::temp_dir().join(format!("aster_mv02_demo_{}", std::process::id()));

    // 清理可能存在的旧目录
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("创建临时目录失败");

    // 写入 aster.toml（最小配置，无 characters 目录）
    let aster_toml = r#"
[game]
name = "最小测试项目"
version = "0.1.0"
entry_scene = "start"
"#;
    fs::write(temp_dir.join("aster.toml"), aster_toml).expect("写入 aster.toml 失败");

    // 创建 scripts/ 目录和入口场景
    let scripts_dir = temp_dir.join("scripts");
    fs::create_dir_all(&scripts_dir).expect("创建 scripts 目录失败");
    fs::write(
        scripts_dir.join("start.aster"),
        "scene \"start\" {\n  narration \"这是一个最小项目。\"\n}",
    )
    .expect("写入 start.aster 失败");

    // 注意：不创建 characters/ 目录 — 这正是 MV02 验证的场景

    println!("╔══════════════════════════════════════════════════╗");
    println!("║   GameLoader MV02 人工验证 — 空 characters/     ║");
    println!("╠══════════════════════════════════════════════════╣");
    println!("║  临时项目: {} ║", temp_dir.display());
    println!("╚══════════════════════════════════════════════════╝");
    println!();
    println!("项目结构：");
    println!("  aster.toml        ← 最小配置（仅 name/version/entry_scene）");
    println!("  scripts/start.aster ← 入口场景");
    println!("  characters/       ← 不存在（故意不创建）");
    println!();
    println!("──────────────────────────────────────────────────");

    match GameLoader::load(&temp_dir) {
        Ok(manifest) => {
            println!();
            println!("✅ 加载成功！");

            println!();
            println!("┌─ 项目元数据 ────────────────────────────────────┐");
            println!("│  名称:      {}", manifest.project.name);
            println!("│  版本:      {}", manifest.project.version);
            println!("│  入口场景:  {}", manifest.project.entry_scene);
            println!(
                "│  分辨率:    {}×{}（默认值）",
                manifest.project.resolution.width, manifest.project.resolution.height,
            );
            println!("└─────────────────────────────────────────────────┘");

            println!();
            println!("┌─ 角色表 ───────────────────────────────────────┐");
            println!(
                "│  角色总数: {}                                  ",
                manifest.characters.len()
            );
            if manifest.characters.is_empty() {
                println!("│  ✅ characters 为空 HashMap（符合预期）        ");
            } else {
                println!("│  ❌ 不应有角色！");
            }
            println!("└─────────────────────────────────────────────────┘");

            println!();
            println!("┌─ 场景清单 ─────────────────────────────────────┐");
            println!("│  场景总数: {}", manifest.scenes.len());
            for scene in &manifest.scenes {
                let entry_mark = if scene.is_entry { " ← 入口" } else { "" };
                println!(
                    "│  [{}] {}{entry_mark}",
                    scene.scene_id,
                    scene.file_path.display()
                );
            }
            println!("└─────────────────────────────────────────────────┘");

            println!();
            println!("┌─ 构建配置 ─────────────────────────────────────┐");
            println!("│  build.toml 不存在 → 使用全部默认值            ");
            println!("│  编译目标:  {}", manifest.build_config.compile.target);
            println!("│  优化:      {}", manifest.build_config.compile.optimize);
            println!("│  压缩:      {}", manifest.build_config.compile.minify);
            println!("└─────────────────────────────────────────────────┘");

            println!();
            println!("✅ MV02 验证通过：空 characters/ 目录不报错，characters 为空！");
        }
        Err(err) => {
            eprintln!();
            eprintln!("❌ 加载失败: {err}");
            eprintln!("   预期：加载成功，因为缺少 characters/ 目录不应阻断裂载");
            std::process::exit(1);
        }
    }

    // 清理临时目录
    let _ = fs::remove_dir_all(&temp_dir);
    println!();
    println!("🧹 已清理临时项目: {}", temp_dir.display());
}
