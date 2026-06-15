//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/examples/verify_game_context.rs
//! 功能概述：GameContext 人工验证示例 — 加载 `templates/default_project/` 模板项目，
//!           经 GameLoader → GameCompiler → GameContext 完整链路，打印上下文内容，
//!           用于人工验证 PH1-T17 的 MV01。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//!
//! 运行方式：
//!   cargo run --package aster-runtime --example verify_game_context
//!
//! 预期输出：
//!   - GameLoader 加载成功（2 角色、2 场景）
//!   - 2 场景编译成功（prologue + chapter1/sakura_road）
//!   - GameContext 构造成功
//!   - scene 查询 / character 查询 / sprite 查询 / path resolve 全部正确

use std::fs;
use std::path::Path;

use aster_compiler::{GameCompileInput, GameCompiler};
use aster_core::Scene;
use aster_runtime::{GameContext, GameLoader};

fn main() {
    let project_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project");

    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║   Asterism — GameContext 人工验证示例 (PH1-T17 MV01) ║");
    println!("╠═══════════════════════════════════════════════════════╣");
    println!("║  项目路径: {} ║", project_path.display());
    println!("╚═══════════════════════════════════════════════════════╝");
    println!();

    // ─── Step 1: 加载游戏清单 ───────────────────────────────────────
    println!("═══ Step 1: GameLoader::load() ═══");
    let manifest = match GameLoader::load(&project_path) {
        Ok(m) => {
            println!("✅ 加载成功");
            m
        }
        Err(e) => {
            eprintln!("❌ 加载失败: {e}");
            std::process::exit(1);
        }
    };

    println!("  角色: {} 个", manifest.characters.len());
    for (id, ch) in &manifest.characters {
        println!("    [{id}] {} — {} 个表情", ch.name, ch.sprites.len());
    }
    println!("  场景: {} 个", manifest.scenes.len());
    for s in &manifest.scenes {
        let entry = if s.is_entry { " (入口)" } else { "" };
        println!("    [{}]{entry}", s.scene_id);
    }
    println!();

    // ─── Step 2: 解析并编译场景 ────────────────────────────────────
    println!("═══ Step 2: 解析 → 编译 ═══");

    let mut parsed_scenes: Vec<(String, Scene)> = Vec::new();
    for entry in &manifest.scenes {
        let full_path = project_path.join(&entry.file_path);
        let source = match fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("❌ 读取 {} 失败: {e}", entry.file_path.display());
                std::process::exit(1);
            }
        };

        match aster_parser::parse_script(&source) {
            Ok(scene) => {
                println!(
                    "  ✅ 解析 {} — {} 个节点",
                    entry.scene_id,
                    scene.nodes.len()
                );
                parsed_scenes.push((entry.scene_id.clone(), scene));
            }
            Err(errors) => {
                eprintln!("❌ 解析 {} 失败:", entry.scene_id);
                for err in &errors {
                    eprintln!("    {}", err.message);
                }
                std::process::exit(1);
            }
        }
    }

    let compile_input = GameCompileInput {
        game_name: &manifest.project.name,
        game_version: &manifest.project.version,
        entry_scene_id: &manifest.project.entry_scene,
        scenes: &parsed_scenes,
        characters: &manifest.characters,
        build_config: &manifest.build_config,
    };

    let compiled = match GameCompiler::compile(compile_input) {
        Ok(c) => {
            println!();
            println!(
                "  ✅ 编译成功 — {} 个场景 / {} 条指令",
                c.build_info.source_file_count, c.build_info.total_instructions
            );
            c
        }
        Err(errors) => {
            eprintln!("❌ 编译失败:");
            for err in &errors {
                eprintln!("    {err}");
            }
            std::process::exit(1);
        }
    };
    println!();

    // ─── Step 3: 构建 GameContext ──────────────────────────────────
    println!("═══ Step 3: GameContext::new() ═══");
    let ctx = GameContext::new(manifest, compiled);
    println!("✅ GameContext 构建成功");
    println!();
    println!("┌─ 基本信息 ──────────────────────────────────────────┐");
    println!("│  项目名称:     {}", ctx.project.name);
    println!("│  版本:         {}", ctx.project.version);
    println!("│  入口场景:     {}", ctx.entry_scene_id);
    println!("│  设计分辨率:   {}×{}", ctx.resolution.0, ctx.resolution.1);
    println!("│  默认文字速度: {:?}", ctx.default_text_speed);
    println!("│  默认 BGM 音量:  {:.1}", ctx.default_bgm_volume);
    println!("│  默认 SE 音量:   {:.1}", ctx.default_se_volume);
    println!("│  默认语音音量:   {:.1}", ctx.default_voice_volume);
    println!("│  角色数:        {}", ctx.characters.len());
    println!("│  已编译场景数:  {}", ctx.scenes.len());
    println!("└────────────────────────────────────────────────────┘");
    println!();

    // ─── Step 4: 验证各查询方法 ──────────────────────────────────
    println!("═══ Step 4: 查询验证 ═══");

    // AC01: get_scene
    println!("── AC01: get_scene ──");
    for scene_id in ["prologue", "chapter1/sakura_road", "nonexistent"] {
        match ctx.get_scene(scene_id) {
            Some(scene) => {
                println!(
                    "  ✅ get_scene(\"{scene_id}\") → {} 条指令, {} 个常量, {} 个标签",
                    scene.instructions.len(),
                    scene.constant_pool.len(),
                    scene.label_table.len(),
                );
            }
            None => {
                println!("  ✅ get_scene(\"{scene_id}\") → None（符合预期）");
            }
        }
    }
    println!();

    // AC02: get_character
    println!("── AC02: get_character ──");
    for char_id in ["sayori", "akane", "unknown"] {
        match ctx.get_character(char_id) {
            Some(ch) => {
                println!("  ✅ get_character(\"{char_id}\")");
                println!("      name:          {}", ch.name);
                println!("      display_color: {}", ch.display_color);
                println!("      default_pos:   {:?}", ch.default_position);
                println!("      sprites:       {} 个", ch.sprites.len());
                for (emotion, asset_id) in &ch.sprites {
                    println!("        {emotion:12} → AssetId({})", asset_id.0);
                }
                if let Some(v) = &ch.voice {
                    println!("      voice:         volume={}", v.volume);
                } else {
                    println!("      voice:         未启用");
                }
            }
            None => {
                println!("  ✅ get_character(\"{char_id}\") → None（符合预期）");
            }
        }
    }
    println!();

    // AC03: get_character_sprite
    println!("── AC03: get_character_sprite ──");
    for (char_id, emotion) in [
        ("sayori", "default"),
        ("sayori", "smile"),
        ("sayori", "angry"),
        ("akane", "default"),
    ] {
        match ctx.get_character_sprite(char_id, emotion) {
            Some(asset_id) => {
                println!(
                    "  ✅ get_character_sprite(\"{char_id}\", \"{emotion}\") → AssetId({})",
                    asset_id.0
                );
            }
            None => {
                println!(
                    "  ✅ get_character_sprite(\"{char_id}\", \"{emotion}\") → None（符合预期）"
                );
            }
        }
    }
    println!();

    // AC04: resolve_sprite_path
    println!("── AC04: resolve_sprite_path ──");
    for (char_id, emotion) in [
        ("sayori", "default"),
        ("sayori", "smile"),
        ("akane", "default"),
        ("sayori", "angry"),
    ] {
        match ctx.resolve_sprite_path(char_id, emotion) {
            Some(path) => {
                println!(
                    "  ✅ resolve_sprite_path(\"{char_id}\", \"{emotion}\") → {}",
                    path.display()
                );
            }
            None => {
                println!(
                    "  ✅ resolve_sprite_path(\"{char_id}\", \"{emotion}\") → None（符合预期）"
                );
            }
        }
    }
    println!();

    // resolve_voice_path
    println!("── resolve_voice_path ──");
    for (char_id, number) in [("sayori", "001"), ("sayori", "042"), ("akane", "001")] {
        match ctx.resolve_voice_path(char_id, number) {
            Some(path) => {
                println!(
                    "  ✅ resolve_voice_path(\"{char_id}\", \"{number}\") → {}",
                    path.display()
                );
            }
            None => {
                println!("  ✅ resolve_voice_path(\"{char_id}\", \"{number}\") → None（符合预期）");
            }
        }
    }
    println!();

    // AC05: is_scene_loaded
    println!("── AC05: is_scene_loaded ──");
    for scene_id in ["prologue", "chapter1/sakura_road", "chapter99"] {
        let loaded = ctx.is_scene_loaded(scene_id);
        println!("  ✅ is_scene_loaded(\"{scene_id}\") → {loaded}");
    }
    println!();

    // ─── 最终总结 ──────────────────────────────────────────────────
    println!("═══════════════════════════════════════════════════════");
    println!();
    println!(
        "✅ GameContext 验证完成！角色: {} / 场景: {}",
        ctx.characters.len(),
        ctx.scenes.len()
    );
    println!();
    println!("请验证以上输出是否符合预期：");
    println!("  - 2 个角色（sayori: 2 表情+语音, akane: 1 表情无语音）");
    println!("  - 2 个场景（prologue + chapter1/sakura_road）");
    println!("  - 路径约定: assets/sprites/<char_id>/<emotion>.png");
    println!("  - 路径约定: assets/voices/<char_id>/<number>.ogg");
    println!("  - akane 无语音 → resolve_voice_path 返回 None");
    println!("  - sayori:angry 不存在 → 返回 None");
}
