//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/examples/load_project.rs
//! 功能概述：GameLoader 人工验证示例 — 加载 `templates/default_project/` 模板项目，
//!           打印 GameManifest 的完整内容，用于人工验证 PH1-T15 的 MV01/MV02。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//!
//! 运行方式：
//!   cargo run --package aster-runtime --example load_project
//!
//! 预期输出：
//!   - 项目名: "My First Visual Novel"
//!   - 2 个角色: sayori（小百合）、akane（小茜）
//!   - 2 个场景: prologue（入口）、chapter1/sakura_road
//!   - build.toml 完整配置

use std::path::Path;

use aster_runtime::GameLoader;

fn main() {
    // 模板项目路径（相对于 workspace 根目录）
    let project_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project");

    println!("╔══════════════════════════════════════════════════╗");
    println!("║     Asterism — GameLoader 人工验证示例          ║");
    println!("╠══════════════════════════════════════════════════╣");
    println!("║  项目路径: {} ║", project_path.display());
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    match GameLoader::load(&project_path) {
        Ok(manifest) => {
            print_manifest(&manifest);
            println!();
            println!("✅ GameLoader 加载成功！请验证以上输出是否符合预期。");
        }
        Err(err) => {
            eprintln!("❌ 加载失败: {err}");
            std::process::exit(1);
        }
    }
}

/// 格式化打印 GameManifest 的全部内容
fn print_manifest(manifest: &aster_runtime::GameManifest) {
    // ─── 项目元数据 ───
    println!("┌─ 项目元数据 (aster.toml) ──────────────────────────┐");
    println!("│  名称:      {}", manifest.project.name);
    println!("│  版本:      {}", manifest.project.version);
    println!("│  入口场景:  {}", manifest.project.entry_scene);
    println!(
        "│  分辨率:    {}×{}",
        manifest.project.resolution.width, manifest.project.resolution.height
    );
    println!("│  语言:      {}", manifest.project.settings.language);
    println!("│  文字速度:  {:?}", manifest.project.settings.text_speed);
    println!(
        "│  BGM 音量:  {:.1}",
        manifest.project.settings.default_bgm_volume
    );
    println!(
        "│  SE 音量:   {:.1}",
        manifest.project.settings.default_se_volume
    );
    println!(
        "│  语音音量:  {:.1}",
        manifest.project.settings.default_voice_volume
    );
    println!("└────────────────────────────────────────────────────┘");
    println!();

    // ─── 角色表 ───
    println!("┌─ 角色表 (characters/*.asterchar) ──────────────────┐");
    println!(
        "│  角色总数: {}                                  ",
        manifest.characters.len()
    );
    for (id, character) in &manifest.characters {
        println!("│");
        println!("│  [{id}]");
        println!("│    显示名:   {}", character.name);
        println!("│    颜色:     {}", character.display_color);
        if let Some(desc) = &character.description {
            println!("│    简介:     {desc}");
        }
        if let Some(bday) = &character.birthday {
            println!("│    生日:     {bday}");
        }
        println!("│    默认位置: {:?}", character.default_position);
        println!("│    立绘表情: {} 个", character.sprites.len());
        for (emotion, asset_id) in &character.sprites {
            println!("│      {:12} → AssetId({})", emotion, asset_id.0);
        }
        if let Some(voice) = &character.voice {
            println!("│    语音:     音量 {:.1}", voice.volume);
        } else {
            println!("│    语音:     未启用");
        }
    }
    println!("└────────────────────────────────────────────────────┘");
    println!();

    // ─── 场景清单 ───
    println!("┌─ 场景清单 (scripts/**/*.aster) ────────────────────┐");
    println!("│  场景总数: {}", manifest.scenes.len());
    for scene in &manifest.scenes {
        let entry_mark = if scene.is_entry { " ← 入口" } else { "" };
        println!(
            "│  [{}] {}{entry_mark}",
            scene.scene_id,
            scene.file_path.display()
        );
    }
    println!("└────────────────────────────────────────────────────┘");
    println!();

    // ─── 构建配置 ───
    let config = &manifest.build_config;
    println!("┌─ 构建配置 (build.toml) ────────────────────────────┐");
    println!("│  编译目标:    {}", config.compile.target);
    println!("│  优化:        {}", bool_zh(config.compile.optimize));
    println!("│  压缩:        {}", bool_zh(config.compile.minify));
    println!("│  包含模式:    {} 条", config.include.patterns.len());
    for pattern in &config.include.patterns {
        println!("│    {}", pattern);
    }
    println!("│  排除模式:    {} 条", config.exclude.patterns.len());
    for pattern in &config.exclude.patterns {
        println!("│    {}", pattern);
    }
    println!("│  归档格式:    {}", config.archive.format);
    println!("│  加密:        {}", bool_zh(config.archive.encrypt));
    println!("└────────────────────────────────────────────────────┘");
}

/// 布尔值转中文（用于输出美化）
fn bool_zh(b: bool) -> &'static str {
    if b { "是" } else { "否" }
}
