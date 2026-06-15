//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/examples/compile_game.rs
//! 功能概述：GameCompiler 人工验证示例 — 编译 `templates/default_project/` 的全部场景，
//!           展示批量编译、跨场景引用验证、BuildInfo 统计。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//!
//! 运行方式：
//!   cargo run --package aster-compiler --example compile_game
//!
//! 预期输出：
//!   - 2 个场景编译成功（prologue, chapter1/sakura_road）
//!   - 每个场景的字节码指令数
//!   - BuildInfo 构建统计
//!   - 跨场景引用验证结果

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use aster_compiler::{GameCompileInput, GameCompiler};
use aster_core::BuildConfig;

fn main() {
    let project_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project");

    let scripts_dir = project_path.join("scripts");

    println!("╔══════════════════════════════════════════════════╗");
    println!("║   Asterism — GameCompiler 人工验证示例         ║");
    println!("╠══════════════════════════════════════════════════╣");
    println!("║  项目路径: {:<36} ║", project_path.display());
    println!("║  脚本目录: {:<36} ║", scripts_dir.display());
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    // ── Step 1: 发现并解析所有 .aster 文件 ──────────────────────────
    println!("┌─ Step 1: 发现场景文件 ─────────────────────────────┐");

    let mut scenes: Vec<(String, aster_core::Scene)> = Vec::new();
    let mut parse_errors: Vec<String> = Vec::new();

    match discover_aster_files(&scripts_dir) {
        Ok(files) => {
            println!("│  发现 {} 个 .aster 文件", files.len());
            for (scene_id, file_path) in &files {
                println!("│    {}  ({})", scene_id, file_path.display());
            }
            println!("│");

            // 解析每个文件
            for (scene_id, file_path) in &files {
                let source = fs::read_to_string(file_path).unwrap_or_else(|e| {
                    panic!("无法读取文件 {}: {}", file_path.display(), e);
                });

                match aster_parser::parse_script(&source) {
                    Ok(scene) => {
                        println!(
                            "│  ✅ 解析成功: {} ({} 个节点)",
                            scene_id,
                            scene.nodes.len()
                        );
                        scenes.push((scene_id.clone(), scene));
                    }
                    Err(errs) => {
                        for err in &errs {
                            let msg = format!("  ❌ {} 解析失败: {}", scene_id, err);
                            println!("│{}", msg);
                            parse_errors.push(msg);
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("❌ 无法扫描脚本目录: {}", e);
            std::process::exit(1);
        }
    }

    println!("└────────────────────────────────────────────────────┘");
    println!();

    if !parse_errors.is_empty() {
        eprintln!("❌ 存在 {} 个解析错误，编译中止。", parse_errors.len());
        std::process::exit(1);
    }

    if scenes.is_empty() {
        eprintln!("❌ 未找到任何 .aster 场景文件。");
        std::process::exit(1);
    }

    // ── Step 2: 加载构建配置 ───────────────────────────────────────
    println!("┌─ Step 2: 构建配置 ─────────────────────────────────┐");

    let build_config = BuildConfig::default();
    println!("│  编译目标: {}", build_config.compile.target);
    println!("│  优化:     {}", bool_zh(build_config.compile.optimize));
    println!("│  压缩:     {}", bool_zh(build_config.compile.minify));
    println!("│  ℹ️  使用默认配置（修改 templates/default_project/build.toml 后需手动改代码）");

    println!("└────────────────────────────────────────────────────┘");
    println!();

    // ── Step 3: 批量编译 ──────────────────────────────────────────
    println!("┌─ Step 3: 批量编译 ─────────────────────────────────┐");

    let characters = HashMap::new(); // 本示例仅演示场景编译
    let entry_scene = scenes.first().map(|(id, _)| id.as_str()).unwrap_or("");

    let input = GameCompileInput {
        game_name: "My First Visual Novel",
        game_version: "0.1.0",
        entry_scene_id: entry_scene,
        scenes: &scenes,
        characters: &characters,
        build_config: &build_config,
    };

    match GameCompiler::compile(input) {
        Ok(compiled) => {
            println!("│  ✅ 全部场景编译成功！");
            println!("│");
            println!("│  ┌─ 场景编译详情 ─────────────────────────────┐");
            for (scene_id, cs) in &compiled.scenes {
                let label_count = cs.label_table.len();
                let byte_len = cs.instructions.len();
                let pool_size = cs.constant_pool.len();
                println!(
                    "│  │  {:<28} 指令={:>4} bytes  标签={:>2}  常量={:>3} │",
                    scene_id, byte_len, label_count, pool_size
                );
            }
            println!("│  └────────────────────────────────────────────┘");
            println!("│");
            println!("│  ┌─ BuildInfo ────────────────────────────────┐");
            println!(
                "│  │  源文件数:     {}",
                compiled.build_info.source_file_count
            );
            println!(
                "│  │  总指令字节:   {}",
                compiled.build_info.total_instructions
            );
            println!(
                "│  │  优化级别:     {}",
                compiled.build_info.optimization_level
            );
            println!(
                "│  │  构建时间:     {}",
                compiled.build_info.build_timestamp
            );
            println!("│  └────────────────────────────────────────────┘");

            // 展示每个场景的 label_table
            println!("│");
            println!("│  ┌─ 跨场景导航 (label_table) ──────────────────┐");
            for (scene_id, cs) in &compiled.scenes {
                if cs.label_table.is_empty() {
                    println!("│  │  {}: (无标签)", scene_id);
                } else {
                    println!("│  │  {}:", scene_id);
                    for (label, offset) in &cs.label_table {
                        println!("│  │    {} → offset {}", label, offset);
                    }
                }
            }
            println!("│  └────────────────────────────────────────────┘");

            println!("└────────────────────────────────────────────────────┘");
            println!();
            println!("✅ GameCompiler 编译成功！");
            println!();
            println!("👆 请验证以上输出：");
            println!("  MV01: 2 个场景编译成功，BuildInfo 显示源文件数=2、总指令数>0");
            println!(
                "  MV02: 如修改 prologue.aster 的 goto 目标为不存在的场景，重新运行应看到中文错误"
            );
        }
        Err(errors) => {
            println!("│  ❌ 编译失败，共 {} 个错误：", errors.len());
            for (i, err) in errors.iter().enumerate() {
                println!("│  {}. {}", i + 1, err);
            }
            println!("└────────────────────────────────────────────────────┘");
            eprintln!();
            eprintln!("❌ 编译失败，请检查以上错误。");
            std::process::exit(1);
        }
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 递归扫描 scripts/ 目录，发现所有 .aster 文件。
///
/// 返回 `(scene_id, file_path)` 列表，其中 scene_id 从文件路径推导：
/// - `scripts/prologue.aster` → `"prologue"`
/// - `scripts/chapter1/sakura_road.aster` → `"chapter1/sakura_road"`
fn discover_aster_files(
    scripts_dir: &Path,
) -> Result<Vec<(String, std::path::PathBuf)>, std::io::Error> {
    let mut files = Vec::new();
    if !scripts_dir.exists() {
        return Ok(files);
    }
    discover_recursive(scripts_dir, scripts_dir, &mut files)?;

    // 按 scene_id 排序保证输出稳定
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// 递归辅助函数：遍历目录，发现 .aster 文件。
fn discover_recursive(
    base_dir: &Path,
    current_dir: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(base_dir).unwrap_or(&path).to_path_buf();

        if path.is_dir() {
            discover_recursive(base_dir, &path, files)?;
        } else if path.extension().is_some_and(|e| e == "aster") {
            let scene_id = derive_scene_id(&relative);
            files.push((scene_id, path.clone()));
        }
    }
    Ok(())
}

/// 从相对路径推导场景 ID。
///
/// `scripts/prologue.aster` → `"prologue"`
/// `scripts/chapter1/sakura_road.aster` → `"chapter1/sakura_road"`
fn derive_scene_id(path: &Path) -> String {
    let mut s = path.to_string_lossy().replace('\\', "/");
    // 去掉 .aster 扩展名
    if s.ends_with(".aster") {
        s = s[..s.len() - 6].to_string();
    }
    s
}

/// 布尔值转中文。
fn bool_zh(b: bool) -> &'static str {
    if b { "是" } else { "否" }
}
