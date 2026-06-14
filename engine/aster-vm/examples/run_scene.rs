//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/examples/run_scene.rs
//! 功能概述：端到端示例 — 解析 .aster 脚本 → 编译 → VM 执行，
//!           逐条输出 VM 动作序列，演示完整的脚本执行管线。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14

use std::time::Instant;

use aster_compiler::Compiler;
use aster_parser::parse_script;
use aster_vm::{EngineCommand, Vm, VmAction};

fn main() {
    // ── 读取脚本文件 ────────────────────────────────────────────────
    // 支持命令行参数指定脚本路径，默认使用 PH1-T13 演示脚本
    let default_script = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/demo_ph1_t13.aster");
    let script_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| default_script.to_string());

    let source = match std::fs::read_to_string(&script_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ 无法读取脚本文件: {}", e);
            eprintln!("   路径: {}", script_path);
            eprintln!("   用法: run_scene [script.aster]");
            std::process::exit(1);
        }
    };

    println!("╔══════════════════════════════════════════════════════╗");
    println!("║   Asterism VM — prologue.aster 端到端执行演示       ║");
    println!("╚══════════════════════════════════════════════════════╝");
    println!();
    println!("📄 脚本: templates/default_project/scripts/prologue.aster");
    println!(
        "📏 大小: {} 字节, {} 行",
        source.len(),
        source.lines().count()
    );

    // ── 第1步：解析 ──────────────────────────────────────────────────
    println!();
    println!("── 第1步：解析 .aster → AST ──");
    let parse_start = Instant::now();
    let scene = match parse_script(&source) {
        Ok(scene) => {
            let elapsed = parse_start.elapsed();
            println!(
                "✅ 解析成功 — 场景 '{}', {} 个 SceneNode, 耗时 {:.2}ms",
                scene.id,
                scene.nodes.len(),
                elapsed.as_secs_f64() * 1000.0
            );
            scene
        }
        Err(errors) => {
            eprintln!("❌ 解析失败 — {} 个错误:", errors.len());
            for e in &errors {
                eprintln!("   {}", e);
            }
            std::process::exit(1);
        }
    };

    // ── 第2步：编译 ──────────────────────────────────────────────────
    println!();
    println!("── 第2步：编译 AST → 字节码 ──");
    let compile_start = Instant::now();
    let compiler = Compiler::new();
    let compiled = match compiler.compile(&scene) {
        Ok(compiled) => {
            let elapsed = compile_start.elapsed();
            println!(
                "✅ 编译成功 — {} bytes 字节码, {} 个常量池条目, {} 个标签, 耗时 {:.2}ms",
                compiled.instructions.len(),
                compiled.constant_pool.len(),
                compiled.label_table.len(),
                elapsed.as_secs_f64() * 1000.0
            );
            compiled
        }
        Err(errors) => {
            eprintln!("❌ 编译失败 — {} 个错误:", errors.len());
            for e in &errors {
                eprintln!("   {}", e);
            }
            std::process::exit(1);
        }
    };

    // ── 打印常量池 ──────────────────────────────────────────────────
    println!();
    println!("── 常量池（前 30 条）──");
    for (i, entry) in compiled.constant_pool.iter().enumerate().take(30) {
        let display = if entry.len() > 60 {
            format!("{}...", &entry[..57])
        } else {
            entry.clone()
        };
        println!("  pool[{}] = \"{}\"", i, display);
    }
    if compiled.constant_pool.len() > 30 {
        println!("  ... 还有 {} 条", compiled.constant_pool.len() - 30);
    }

    // ── 打印标签表 ──────────────────────────────────────────────────
    if !compiled.label_table.is_empty() {
        println!();
        println!("── 标签表 ──");
        for (name, offset) in &compiled.label_table {
            println!("  @{} → 偏移 {}", name, offset);
        }
    }

    // ── 打印字节码（前 40 bytes） ───────────────────────────────────
    println!();
    println!("── 字节码（前 80 bytes，hex dump）──");
    for (i, chunk) in compiled.instructions.iter().take(80).enumerate() {
        if i % 16 == 0 && i > 0 {
            println!();
        }
        print!("{:02X} ", chunk);
    }
    println!();
    if compiled.instructions.len() > 80 {
        println!("  ... 还有 {} bytes", compiled.instructions.len() - 80);
    }

    // ── 打印标签表 + 周围字节 ──────────────────────────────────
    if !compiled.label_table.is_empty() {
        println!();
        println!("── 标签表（含周围字节 hex）──");
        for (name, offset) in &compiled.label_table {
            let start = offset.saturating_sub(4);
            let end = (*offset + 16).min(compiled.instructions.len());
            let hex: Vec<String> = compiled.instructions[start..end]
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    if start + i == *offset {
                        format!("[{:02X}]", b)
                    } else {
                        format!("{:02X}", b)
                    }
                })
                .collect();
            println!("  @{} → {}: {}", name, offset, hex.join(" "));
        }
    }

    // ── 第3步：VM 执行（交互式）──────────────────────────────────
    println!();
    println!("══════════════════════════════════════════════════════");
    println!("── 第3步：VM 执行字节码（交互模式）──");
    println!("══════════════════════════════════════════════════════");
    println!("💡 提示：遇到对话/旁白时按 Enter 继续，菜单时输入数字选择");
    println!();

    let mut vm = Vm::new();
    let mut step_count: u64 = 0;
    let mut error_count: u64 = 0;
    let execute_start = Instant::now();

    loop {
        step_count += 1;

        // 安全检查
        if step_count > 100_000 {
            println!();
            println!("⚠️ 已达到最大步数限制，强制终止");
            break;
        }

        let action = vm.step(&compiled);

        match &action {
            VmAction::SceneEnd => {
                let elapsed = execute_start.elapsed();
                println!();
                println!("✅ 场景执行完毕 (SceneEnd)");
                println!(
                    "   总步数: {}, 错误数: {}, 总耗时: {:.2}ms",
                    step_count,
                    error_count,
                    elapsed.as_secs_f64() * 1000.0
                );
                break;
            }

            VmAction::Command(cmd) => match cmd {
                // ── 对话 / 旁白：显示文本，等待 Enter ──
                EngineCommand::SetDialogue {
                    speaker_idx,
                    text_idx,
                    voice_idx: _,
                } => {
                    let speaker = pool_str(&compiled.constant_pool, *speaker_idx);
                    let text = pool_str(&compiled.constant_pool, *text_idx);
                    println!("  💬 {}: \"{}\"", speaker, text);
                }

                EngineCommand::SetNarration { text_idx } => {
                    let text = pool_str(&compiled.constant_pool, *text_idx);
                    println!("  📖 \"{}\"", text);
                }

                // ── 错误：静默计数，只显示前 10 条 ──
                EngineCommand::Error { message } => {
                    error_count += 1;
                    if error_count <= 10 {
                        let short = if message.len() > 100 {
                            format!("{}...", &message[..97])
                        } else {
                            message.clone()
                        };
                        println!("  ⚠️  [{}] {}", error_count, short);
                    } else if error_count == 11 {
                        println!("  ... 后续错误已省略（共 {} 条未实现指令）", error_count);
                    }
                }

                // ── 其他渲染/音频命令：简要输出 ──
                other => {
                    let desc = format_command(&compiled.constant_pool, other);
                    println!("  🎬 {}", desc);
                }
            },

            VmAction::ShowMenu {
                prompt_idx,
                choices,
            } => {
                let prompt = pool_str(&compiled.constant_pool, *prompt_idx);
                println!();
                println!("┌─ 选择支 ───────────────────");
                println!("│ 📋 {}", prompt);
                println!("│");
                for (i, choice) in choices.iter().enumerate() {
                    let text = pool_str(&compiled.constant_pool, choice.text_idx);
                    println!("│   {}. {}", i + 1, text);
                }
                println!("└─────────────────────────────");

                // 等待用户选择
                let selection = read_choice(choices.len());
                let chosen_offset = choices[selection].target_offset as usize;
                vm.set_pc(chosen_offset);
                println!("   → 跳转到偏移 {}", chosen_offset);
            }

            VmAction::WaitForInput => {
                // 自动继续（仅菜单需要交互）
            }
        }
    }
}

/// 从常量池解析字符串索引
fn pool_str(pool: &[String], idx: u16) -> &str {
    if idx == 0xFFFF {
        return "<none>";
    }
    pool.get(idx as usize).map(|s| s.as_str()).unwrap_or("<?>")
}

/// 读取用户菜单选择（1-based → 0-based 索引）
fn read_choice(count: usize) -> usize {
    use std::io::{self, BufRead, Write};
    loop {
        print!("   请选择 (1-{}): ", count);
        let _ = io::stdout().flush();
        let stdin = io::stdin();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_ok()
            && let Ok(n) = line.trim().parse::<usize>()
            && n >= 1
            && n <= count
        {
            return n - 1;
        }
        println!("   无效输入，请重新选择。");
    }
}

/// 将 EngineCommand 格式化为人类可读的字符串。
///
/// 使用常量池将索引解析为实际字符串值，便于理解 VM 输出。
fn format_command(constant_pool: &[String], cmd: &EngineCommand) -> String {
    /// 解析寄存器索引（0xFF = 未使用）
    fn reg_str(reg: u8) -> String {
        if reg == 0xFF {
            "<default>".to_string()
        } else {
            format!("r{}", reg)
        }
    }

    /// 解析位置字节为可读字符串
    fn pos_str(pos_byte: u8) -> &'static str {
        match pos_byte {
            0 => "Left",
            1 => "Center",
            2 => "Right",
            3 => "Custom",
            _ => "?",
        }
    }

    match cmd {
        EngineCommand::SetBg {
            asset_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            format!(
                "🖼  SetBg \"{}\" trans={} dur={}",
                pool_str(constant_pool, *asset_idx),
                pool_str(constant_pool, *trans_kind_idx),
                reg_str(*dur_reg)
            )
        }

        EngineCommand::ShowChar {
            char_idx,
            pos_byte,
            emotion_idx,
            trans_kind_idx,
            dur_reg,
            ..
        } => {
            format!(
                "👤 ShowChar \"{}\" at {} emotion=\"{}\" trans={} dur={}",
                pool_str(constant_pool, *char_idx),
                pos_str(*pos_byte),
                pool_str(constant_pool, *emotion_idx),
                pool_str(constant_pool, *trans_kind_idx),
                reg_str(*dur_reg)
            )
        }

        EngineCommand::HideChar {
            char_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            format!(
                "🙈 HideChar \"{}\" trans={} dur={}",
                pool_str(constant_pool, *char_idx),
                pool_str(constant_pool, *trans_kind_idx),
                reg_str(*dur_reg)
            )
        }

        EngineCommand::SetDialogue {
            speaker_idx,
            text_idx,
            voice_idx,
        } => {
            let speaker = pool_str(constant_pool, *speaker_idx);
            let text = pool_str(constant_pool, *text_idx);
            let voice = if *voice_idx != 0xFFFF {
                format!(" voice=\"{}\"", pool_str(constant_pool, *voice_idx))
            } else {
                String::new()
            };
            // 截断过长的文本
            let text_display = if text.len() > 50 {
                format!("{}...", &text[..47])
            } else {
                text.to_string()
            };
            format!(
                "💬 SetDialogue \"{}\" → \"{}\"{}",
                speaker, text_display, voice
            )
        }

        EngineCommand::SetNarration { text_idx } => {
            let text = pool_str(constant_pool, *text_idx);
            let text_display = if text.len() > 60 {
                format!("{}...", &text[..57])
            } else {
                text.to_string()
            };
            format!("📖 SetNarration \"{}\"", text_display)
        }

        EngineCommand::PlayBgm {
            asset_idx,
            fade_reg,
            looping,
        } => {
            format!(
                "🎵 PlayBgm \"{}\" fade={} loop={}",
                pool_str(constant_pool, *asset_idx),
                reg_str(*fade_reg),
                looping
            )
        }

        EngineCommand::StopBgm { fade_reg } => {
            format!("🔇 StopBgm fade={}", reg_str(*fade_reg))
        }

        EngineCommand::PlaySe {
            asset_idx,
            fade_reg,
        } => {
            format!(
                "🔔 PlaySe \"{}\" fade={}",
                pool_str(constant_pool, *asset_idx),
                reg_str(*fade_reg)
            )
        }

        EngineCommand::PlayVoice { asset_idx } => {
            format!("🎙  PlayVoice \"{}\"", pool_str(constant_pool, *asset_idx))
        }

        EngineCommand::ShowSprite {
            asset_idx,
            x_reg,
            y_reg,
            scale_reg,
            alpha_reg,
            ..
        } => {
            format!(
                "✨ ShowSprite \"{}\" at (r{}, r{}) scale=r{} alpha=r{}",
                pool_str(constant_pool, *asset_idx),
                x_reg,
                y_reg,
                scale_reg,
                alpha_reg
            )
        }

        EngineCommand::HideSprite {
            asset_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            format!(
                "🧹 HideSprite \"{}\" trans={} dur={}",
                pool_str(constant_pool, *asset_idx),
                pool_str(constant_pool, *trans_kind_idx),
                reg_str(*dur_reg)
            )
        }

        EngineCommand::MoveChar {
            char_idx,
            pos_byte,
            emotion_idx,
            trans_kind_idx,
            dur_reg,
            ..
        } => {
            format!(
                "🚶 MoveChar \"{}\" → {} emotion=\"{}\" trans={} dur={}",
                pool_str(constant_pool, *char_idx),
                pos_str(*pos_byte),
                pool_str(constant_pool, *emotion_idx),
                pool_str(constant_pool, *trans_kind_idx),
                reg_str(*dur_reg)
            )
        }

        EngineCommand::Emotion {
            char_idx,
            emotion_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            format!(
                "😊 Emotion \"{}\" → \"{}\" trans={} dur={}",
                pool_str(constant_pool, *char_idx),
                pool_str(constant_pool, *emotion_idx),
                pool_str(constant_pool, *trans_kind_idx),
                reg_str(*dur_reg)
            )
        }

        EngineCommand::Wait { dur_reg } => {
            format!("⏱  Wait dur={}", reg_str(*dur_reg))
        }

        EngineCommand::Effect { type_idx, params } => {
            let effect_type = pool_str(constant_pool, *type_idx);
            format!("🎬 Effect \"{}\" ({} params)", effect_type, params.len())
        }

        EngineCommand::Error { message } => {
            // 截断过长的错误消息
            let display = if message.len() > 80 {
                format!("{}...", &message[..77])
            } else {
                message.clone()
            };
            format!("❌ ERROR: {}", display)
        }
    }
}
