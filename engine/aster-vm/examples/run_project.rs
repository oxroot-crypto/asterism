//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/examples/run_project.rs
//! 功能概述：项目运行器 — 指定脚本目录，加载入口场景执行。
//!           自动处理跨场景 Goto（加载目标 .aster → 编译 → 同 VM 继续执行）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 用法:
//!   cargo run -p aster-vm --example run_project <scripts_dir> --entry <entry_scene_id>
//!   cargo run -p aster-vm --example run_project examples --entry demo_ph1_t14_scene_a

use std::path::{Path, PathBuf};
use std::time::Instant;

use aster_compiler::Compiler;
use aster_parser::parse_script;
use aster_vm::{EngineCommand, Vm, VmAction};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut scripts_dir_str = String::new();
    let mut entry_scene_id = String::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--entry" => {
                i += 1;
                if i < args.len() {
                    entry_scene_id = args[i].clone();
                    i += 1;
                }
            }
            other if !other.starts_with("--") => {
                scripts_dir_str = other.to_string();
                i += 1;
            }
            _ => {
                eprintln!("未知参数: {}", args[i]);
                i += 1;
            }
        }
    }

    // 默认值
    if scripts_dir_str.is_empty() {
        scripts_dir_str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples").to_string();
    }
    if entry_scene_id.is_empty() {
        entry_scene_id = "prologue".to_string();
    }

    let scripts_dir = PathBuf::from(&scripts_dir_str);

    println!("╔══════════════════════════════════════════════════════╗");
    println!("║   Asterism VM — 项目运行器                          ║");
    println!("╚══════════════════════════════════════════════════════╝");
    println!();
    println!("📁 脚本目录: {:?}", scripts_dir);
    println!("📄 入口场景: {}", entry_scene_id);
    println!();

    // 加载入口场景
    let entry_file = scripts_dir.join(format!("{}.aster", entry_scene_id));
    if !entry_file.exists() {
        eprintln!("❌ 入口场景文件不存在: {:?}", entry_file);
        eprintln!("   用法: run_project <scripts_dir> --entry <scene_id>");
        std::process::exit(1);
    }

    let source = read_file(entry_file.to_str().unwrap());
    let compiled = parse_and_compile(&source);
    println!(
        "  入口场景编译: {} bytes 字节码",
        compiled.instructions.len()
    );
    println!();

    let mut vm = Vm::new();
    let mut total_steps: u64 = 0;
    let mut total_errors: u64 = 0;
    let execute_start = Instant::now();

    println!("══════════════════════════════════════════════════════");
    println!("── VM 开始执行 ──");
    println!();

    execute_scene_loop(
        &mut vm,
        &compiled,
        &scripts_dir,
        &mut total_steps,
        &mut total_errors,
    );

    let elapsed = execute_start.elapsed();

    print_vm_state(&vm);
    println!();
    println!("── 执行摘要 ──");
    println!("   总步数: {}", total_steps);
    println!("   错误数: {}", total_errors);
    println!("   总耗时: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
}

/// 主执行循环 — 递归处理 Goto 跨场景跳转。
fn execute_scene_loop(
    vm: &mut Vm,
    compiled: &aster_compiler::CompiledScene,
    scripts_dir: &Path,
    total_steps: &mut u64,
    total_errors: &mut u64,
) {
    loop {
        *total_steps += 1;
        if *total_steps > 100_000 {
            println!();
            println!("⚠️ 已达最大步数限制 ({})，强制终止", 100_000);
            return;
        }

        let action = vm.step(compiled);

        match &action {
            VmAction::SceneEnd => return,

            VmAction::Command(cmd) => match cmd {
                EngineCommand::SetDialogue { speaker, text, .. } => {
                    let short: String = if text.chars().count() > 60 {
                        text.chars().take(57).chain("...".chars()).collect()
                    } else {
                        text.clone()
                    };
                    println!("  💬 {}: \"{}\"", speaker, short);
                }
                EngineCommand::SetNarration { text } => {
                    let display: String = if text.chars().count() > 70 {
                        text.chars().take(67).chain("...".chars()).collect()
                    } else {
                        text.clone()
                    };
                    println!("  📖 \"{}\"", display);
                }
                EngineCommand::Goto { scene, label } => {
                    println!("  🔀 GOTO → \"{}\" label=\"{}\"", scene, label);
                    let target_file = scripts_dir.join(format!("{}.aster", scene));
                    if !target_file.exists() {
                        eprintln!("❌ 目标场景不存在: {:?}", target_file);
                        *total_errors += 1;
                        return;
                    }
                    println!();
                    println!("── 加载场景: {:?} ──", target_file);
                    println!();
                    let target_source = read_file(target_file.to_str().unwrap());
                    let target_compiled = parse_and_compile(&target_source);

                    if !label.is_empty() {
                        if let Some(&offset) = target_compiled.label_table.get(label) {
                            vm.set_pc(offset);
                            println!("   → 标签 \"{}\" 偏移 {}", label, offset);
                        } else {
                            eprintln!("⚠️ 标签 \"{}\" 未找到，从入口开始", label);
                            vm.set_pc(0);
                        }
                    } else {
                        vm.set_pc(0);
                    }

                    execute_scene_loop(
                        vm,
                        &target_compiled,
                        scripts_dir,
                        total_steps,
                        total_errors,
                    );
                    return;
                }
                EngineCommand::Error { message } => {
                    *total_errors += 1;
                    if *total_errors <= 10 {
                        println!("  ⚠️  [{}] {}", total_errors, message);
                    }
                }
                _ => {
                    println!("  🎬 {}", format_command(cmd));
                }
            },

            VmAction::ShowMenu { prompt, choices } => {
                println!();
                println!("┌─ 选择支 ───────────────────");
                println!("│ 📋 {}", prompt);
                println!("│");
                for (i, choice) in choices.iter().enumerate() {
                    let text = pool_str(&compiled.constant_pool, choice.text_idx);
                    println!("│   {}. {}", i + 1, text);
                }
                println!("└─────────────────────────────");

                let selection = read_choice(choices.len());
                vm.set_pc(choices[selection].target_offset as usize);
                println!("   → 偏移 {}", choices[selection].target_offset);
            }

            VmAction::WaitForInput => {
                print!("   [Enter] ");
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let mut buf = String::new();
                let _ = std::io::stdin().read_line(&mut buf);
            }
        }
    }
}

fn read_file(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ 无法读取: {} ({})", path, e);
            std::process::exit(1);
        }
    }
}

fn parse_and_compile(source: &str) -> aster_compiler::CompiledScene {
    let scene = match parse_script(source) {
        Ok(s) => s,
        Err(errors) => {
            eprintln!("❌ 解析失败 ({} 个错误)", errors.len());
            for e in &errors {
                eprintln!("   {}", e);
            }
            std::process::exit(1);
        }
    };
    match Compiler::new().compile(&scene) {
        Ok(c) => c,
        Err(errors) => {
            eprintln!("❌ 编译失败 ({} 个错误)", errors.len());
            for e in &errors {
                eprintln!("   {}", e);
            }
            std::process::exit(1);
        }
    }
}

fn format_command(cmd: &EngineCommand) -> String {
    fn reg_str(reg: u8) -> String {
        if reg == 0xFF {
            "<default>".into()
        } else {
            format!("r{}", reg)
        }
    }
    match cmd {
        EngineCommand::SetBg { asset, .. } => format!("SetBg \"{}\"", asset),
        EngineCommand::ShowChar { char, emotion, .. } => {
            format!("ShowChar \"{}\" emo=\"{}\"", char, emotion)
        }
        EngineCommand::HideChar { char, .. } => format!("HideChar \"{}\"", char),
        EngineCommand::PlayBgm { asset, .. } => format!("PlayBgm \"{}\"", asset),
        EngineCommand::StopBgm { .. } => "StopBgm".into(),
        EngineCommand::PlaySe { asset, .. } => format!("PlaySe \"{}\"", asset),
        EngineCommand::PlayVoice { asset } => format!("PlayVoice \"{}\"", asset),
        EngineCommand::ShowSprite { asset, .. } => format!("ShowSprite \"{}\"", asset),
        EngineCommand::HideSprite { asset, .. } => format!("HideSprite \"{}\"", asset),
        EngineCommand::MoveChar { char, .. } => format!("MoveChar \"{}\"", char),
        EngineCommand::Emotion { char, emotion, .. } => {
            format!("Emotion \"{}\" → \"{}\"", char, emotion)
        }
        EngineCommand::Wait { dur_reg } => format!("Wait dur={}", reg_str(*dur_reg)),
        EngineCommand::Effect { effect_type, .. } => format!("Effect \"{}\"", effect_type),
        _ => format!("{:?}", cmd),
    }
}

fn print_vm_state(vm: &Vm) {
    let vars = vm.variables();
    let flags = vm.flags();
    if !vars.is_empty() || !flags.is_empty() {
        println!();
        println!("── VM 最终状态 ──");
        if !vars.is_empty() {
            println!("📦 变量 ({} 个):", vars.len());
            let mut names: Vec<&String> = vars.iter().map(|(k, _)| k).collect();
            names.sort();
            for name in names {
                if let Some(v) = vars.get(name) {
                    println!("   ${} = {:?}", name, v);
                }
            }
        }
        if !flags.is_empty() {
            println!("🚩 旗标 ({} 个):", flags.len());
            let mut names: Vec<&String> = flags.iter().collect();
            names.sort();
            for name in names {
                println!("   %{}", name);
            }
        }
    }
}

fn pool_str(pool: &[String], idx: u16) -> &str {
    if idx == 0xFFFF {
        return "<none>";
    }
    pool.get(idx as usize).map(|s| s.as_str()).unwrap_or("<?>")
}

fn read_choice(count: usize) -> usize {
    use std::io::{self, BufRead, Write};
    loop {
        print!("   请选择 (1-{}): ", count);
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line).is_ok()
            && let Ok(n) = line.trim().parse::<usize>()
            && n >= 1
            && n <= count
        {
            return n - 1;
        }
        println!("   无效，请重试。");
    }
}
