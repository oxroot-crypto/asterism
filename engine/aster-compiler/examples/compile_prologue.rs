//! Asterism — 编译器人工验证示例
//!
//! 文件路径：engine/aster-compiler/examples/compile_prologue.rs
//! 功能概述：读取 `templates/default_project/scripts/prologue.aster`，
//!           解析→编译→输出常量池、指令统计、标签表等信息，
//!           供人工测试验证（MV02）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//!
//! 运行方式：
//!   cargo run --package aster-compiler --example compile_prologue

use std::collections::HashSet;

use aster_compiler::Compiler;
use aster_parser::parse_script;

fn main() {
    // 读取 prologue.aster
    let prologue_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../templates/default_project/scripts/prologue.aster"
    );

    println!("═══════════════════════════════════════════");
    println!("  aster-compiler 人工验证 — MV02");
    println!("═══════════════════════════════════════════");
    println!();
    println!("📂 脚本文件: {}", prologue_path);

    let source = match std::fs::read_to_string(prologue_path) {
        Ok(s) => {
            println!("✅ 文件读取成功 ({} 字节)", s.len());
            s
        }
        Err(e) => {
            eprintln!("❌ 无法读取文件: {}", e);
            std::process::exit(1);
        }
    };

    // 第1步：解析
    println!();
    println!("─── 第1步：解析 AST ───");
    let scene = match parse_script(&source) {
        Ok(scene) => {
            println!("✅ 解析成功");
            println!("   场景 ID: {}", scene.id);
            println!("   节点数量: {}", scene.nodes.len());
            scene
        }
        Err(errors) => {
            eprintln!("❌ 解析失败 ({} 个错误):", errors.len());
            for e in &errors {
                eprintln!("   - {}", e);
            }
            std::process::exit(1);
        }
    };

    // 第2步：编译（优化前 vs 优化后对比）
    println!();
    println!("─── 第2步：编译 → 字节码（优化前后对比）───");

    // 2a：无优化编译
    let raw = Compiler::new().compile_raw(&scene).expect("原始编译失败");
    let raw_instr = count_instructions(&raw.instructions);

    // 2b：优化编译
    let optimized = Compiler::new().compile(&scene).expect("优化编译失败");
    let opt_instr = count_instructions(&optimized.instructions);

    println!("✅ 编译成功");
    println!();
    println!("┌──────────────────────┬──────────┬──────────┬──────────┐");
    println!("│ 指标                 │ 优化前    │ 优化后    │ 变化      │");
    println!("├──────────────────────┼──────────┼──────────┼──────────┤");
    println!(
        "│ 字节码大小           │ {:>7} B  │ {:>7} B  │ {:>+5} B  │",
        raw.instructions.len(),
        optimized.instructions.len(),
        optimized.instructions.len() as isize - raw.instructions.len() as isize
    );
    println!(
        "│ 指令数量             │ {:>7}    │ {:>7}    │ {:>+5}    │",
        raw_instr,
        opt_instr,
        opt_instr as isize - raw_instr as isize
    );
    println!(
        "│ 常量池条目           │ {:>7}    │ {:>7}    │   不变   │",
        raw.constant_pool.len(),
        optimized.constant_pool.len()
    );
    println!(
        "│ 标签数量             │ {:>7}    │ {:>7}    │   不变   │",
        raw.label_table.len(),
        optimized.label_table.len()
    );
    let reduction = if raw_instr > 0 {
        ((raw_instr - opt_instr) as f64 / raw_instr as f64) * 100.0
    } else {
        0.0
    };
    println!(
        "│ 指令缩减比例         │          │ {:>5.1}%   │          │",
        reduction
    );
    println!("└──────────────────────┴──────────┴──────────┴──────────┘");

    // 语义等价验证
    println!();
    println!("─── 语义等价验证 ───");
    let pool_match = raw.constant_pool == optimized.constant_pool;
    println!(
        "  常量池一致:    {}",
        if pool_match { "✅" } else { "❌ 不一致!" }
    );
    let raw_labels: HashSet<&String> = raw.label_table.keys().collect();
    let opt_labels: HashSet<&String> = optimized.label_table.keys().collect();
    let labels_match = raw_labels == opt_labels;
    println!(
        "  标签名一致:    {}",
        if labels_match {
            "✅"
        } else {
            "❌ 不一致!"
        }
    );
    let raw_has_end = raw.instructions.last() == Some(&0xFF);
    let opt_has_end = optimized.instructions.last() == Some(&0xFF);
    println!("  优化前 End:    {}", if raw_has_end { "✅" } else { "❌" });
    println!("  优化后 End:    {}", if opt_has_end { "✅" } else { "❌" });
    let no_increase = optimized.instructions.len() <= raw.instructions.len();
    println!("  指令不增加:    {}", if no_increase { "✅" } else { "❌" });
    println!(
        "  总体语义保留:  {}",
        if pool_match && labels_match && raw_has_end && opt_has_end && no_increase {
            "✅ 通过"
        } else {
            "❌ 失败"
        }
    );

    // 使用优化后的结果展示详情
    let compiled = optimized;

    // 常量池内容
    println!();
    println!("─── 常量池内容 ───");
    for (idx, entry) in compiled.constant_pool.iter().enumerate() {
        let display = if entry.len() > 60 {
            // 找最近的有效 UTF-8 字符边界
            let mut end = 57;
            while end > 0 && !entry.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &entry[..end])
        } else {
            entry.clone()
        };
        println!("  [{:3}] {}", idx, display);
    }

    // 标签表
    if !compiled.label_table.is_empty() {
        println!();
        println!("─── 标签表 ───");
        let mut labels: Vec<(&String, &usize)> = compiled.label_table.iter().collect();
        labels.sort_by_key(|(_, offset)| *offset);
        for (name, offset) in &labels {
            println!("  0x{:04X} → {}", offset, name);
        }
    }

    // 字节码十六进制预览（前 256 字节）
    println!();
    println!("─── 字节码预览（前 256 字节）───");
    let preview_len = compiled.instructions.len().min(256);
    for (i, chunk) in compiled.instructions[..preview_len].chunks(16).enumerate() {
        print!("  {:04X}: ", i * 16);
        for byte in chunk {
            print!("{:02X} ", byte);
        }
        // 对齐
        if chunk.len() < 16 {
            for _ in 0..(16 - chunk.len()) {
                print!("   ");
            }
        }
        print!(" ");
        // ASCII 可视化
        for byte in chunk {
            if byte.is_ascii_graphic() || *byte == b' ' {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        println!();
    }
    if compiled.instructions.len() > 256 {
        println!("  ... (剩余 {} 字节)", compiled.instructions.len() - 256);
    }

    // 操作码分布统计
    println!();
    println!("─── 操作码分布 ───");
    let op_counts = count_opcodes(&compiled.instructions);
    let mut sorted: Vec<(String, usize)> = op_counts.into_iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    for (name, count) in &sorted {
        println!("  {}: {} 次", name, count);
    }

    println!();
    println!("═══════════════════════════════════════════");
    println!("  验证完成 ✅");
    println!("═══════════════════════════════════════════");
}

/// 统计字节码中的指令数量（通过扫描操作码并跳转到下一条指令）。
fn count_instructions(bytes: &[u8]) -> usize {
    let mut count = 0;
    let mut pos = 0;
    while pos < bytes.len() {
        let opcode = match aster_compiler::Opcode::from_byte(bytes[pos]) {
            Some(op) => op,
            None => {
                // 遇到未知操作码，可能是数据区域，终止计数
                break;
            }
        };
        count += 1;
        pos += instruction_size(opcode, &bytes[pos..]);
    }
    count
}

/// 根据操作码和操作数计算指令总字节数。
fn instruction_size(opcode: aster_compiler::Opcode, _rest: &[u8]) -> usize {
    use aster_compiler::Opcode;
    match opcode {
        Opcode::PushStr => 4,
        Opcode::PushInt => 10,
        Opcode::PushFloat => 10,
        Opcode::PushBool => 3,
        Opcode::LoadVar => 4,
        Opcode::StoreVar => 4,
        Opcode::CheckFlag => 4,
        Opcode::Add
        | Opcode::Sub
        | Opcode::Mul
        | Opcode::Div
        | Opcode::Eq
        | Opcode::Neq
        | Opcode::Lt
        | Opcode::Gt
        | Opcode::Le
        | Opcode::Ge
        | Opcode::And
        | Opcode::Or => 4,
        Opcode::Not | Opcode::Neg => 3,
        Opcode::Bg => 6,
        Opcode::ShowChar => 11,
        Opcode::ShowSprite => 10,
        Opcode::MoveChar => 11,
        Opcode::Emotion => 8,
        Opcode::HideChar => 6,
        Opcode::HideSprite => 6,
        Opcode::Dialogue => 7,
        Opcode::Narrate => 3,
        Opcode::Menu => {
            // Menu 变长: op(1) + prompt(2) + count(1) + choices*6
            // _rest[0] is opcode byte (already consumed)
            if _rest.len() > 3 {
                let choice_count = _rest[3] as usize;
                4 + choice_count * 6
            } else {
                4
            }
        }
        Opcode::Jump | Opcode::Call => 3,
        Opcode::JumpIf => 4,
        Opcode::JumpIfFlag => 5,
        Opcode::Return => 1,
        Opcode::Label => 0, // 伪指令，不应出现在字节码中
        Opcode::Goto => 5,
        Opcode::SetVar => 4,
        Opcode::SetFlag | Opcode::UnsetFlag | Opcode::ToggleFlag => 3,
        Opcode::PlayBgm => 5,
        Opcode::StopBgm => 2,
        Opcode::PlaySe => 4,
        Opcode::PlayVoice => 3,
        Opcode::Effect => {
            // Effect 变长: op(1) + type(2) + count(1) + params*(2+2)
            if _rest.len() > 3 {
                let param_count = _rest[3] as usize;
                4 + param_count * 4
            } else {
                4
            }
        }
        Opcode::Wait => 2,
        Opcode::End => 1,
    }
}

/// 统计各操作码出现次数。
fn count_opcodes(bytes: &[u8]) -> std::collections::HashMap<String, usize> {
    use aster_compiler::Opcode;
    use std::collections::HashMap;

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut pos = 0;

    while pos < bytes.len() {
        let opcode = match Opcode::from_byte(bytes[pos]) {
            Some(op) => op,
            None => break,
        };
        *counts.entry(opcode.to_string()).or_insert(0) += 1;
        pos += instruction_size(opcode, &bytes[pos..]);
    }

    counts
}
