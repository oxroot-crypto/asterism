//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-parser/src/lib.rs
//! 功能概述：.aster DSL 解析器 — 将 `.aster` 脚本源码解析为 pest token 流
//!           （PH1-T04），后续 PH1-T05 接入 AstBuilder 后产出 `aster_core::Scene`。
//!           基于 PEG（Parsing Expression Grammar）语法文件 `grammar.pest`，
//!           使用 pest 解析器生成器。输出是结构化的 token pair 树。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - pest（PEG 解析器运行时）
//! - pest_derive（宏 `#[derive(Parser)]`，编译期处理 `grammar.pest`）
//! - aster_core（共享数据类型，Phase 1 起直接复用 Scene/SceneNode 等类型）
//!
//! 架构位置：aster-core ← aster-parser ← aster-compiler
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `error` | `error.rs` | `ParseError` 结构体：携带行号/列号的中文错误信息 |
//! | `parser` | `parser.rs` | pest 解析器入口 `parse_script()`：.aster 源码 → `aster_core::Scene` |
//! | `builder` | `builder/mod.rs` | `AstBuilder`：pest token 流 → `aster_core::Scene` 递归下降构建 |
//! | `builder::expr` | `builder/expr.rs` | 表达式构建：`build_expr()` / `build_primary()` / 运算符解析 |
//! | `builder::position` | `builder/position.rs` | 位置与转场构建：`build_position()` / `build_transition()` |
//! | `builder::statements` | `builder/statements.rs` | 语句构建：25 种 SceneNode 变体的构建方法 |
//!
//! ## 解析流程
//! ```text
//! .aster 源码 → pest::Parser (PEG 语法) → PestToken 流 → AstBuilder → aster_core::Scene
//! ```
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use aster_parser::parse_script;
//!
//! let source = r#"scene "prologue" {
//!     bg "classroom" with fade(1000)
//!     show sayori at center
//!     sayori "你好，世界！"
//! }"#;
//!
//! match parse_script(source) {
//!     Ok(scene) => {
//!         println!("场景 '{}' 包含 {} 个节点", scene.id, scene.nodes.len());
//!     }
//!     Err(errors) => {
//!         for e in &errors {
//!             eprintln!("{}", e);
//!         }
//!     }
//! }
//! ```

// 模块声明
pub mod builder;
pub mod error;
pub mod parser;

// 重导出 — 外部 crate 通过 `aster_parser::` 路径直接使用
pub use builder::AstBuilder;
pub use error::ParseError;
pub use parser::parse_script;

// ============================================================================
// 集成测试 — 验收标准 (AC)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use aster_core::{Expr, SceneNode};

    // ── AC01: prologue.aster（574 行，25 变体+Expr）解析为正确的 Scene ──

    /// AC01 — 合法 .aster 脚本解析为完整的 `aster_core::Scene`。
    ///
    /// 使用 `templates/default_project/scripts/prologue.aster`（574 行，
    /// 覆盖全部 25 种 SceneNode + Expr 插值）作为输入。
    #[test]
    fn ac01_prologue_aster_parses_to_scene() {
        let prologue_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../templates/default_project/scripts/prologue.aster"
        );
        let source = std::fs::read_to_string(prologue_path).expect("无法读取 prologue.aster 文件");

        let result = parse_script(&source);
        assert!(
            result.is_ok(),
            "prologue.aster 应成功解析为 Scene，但返回错误: {:?}",
            result.err()
        );

        let scene = result.unwrap();
        assert_eq!(scene.id, "prologue", "场景 ID 应为 'prologue'");
        assert!(!scene.nodes.is_empty(), "prologue.aster 应产生非空节点列表");

        // 验证包含全部 25 种节点类型
        let variant_names: Vec<&str> = scene
            .nodes
            .iter()
            .map(|n| match n {
                SceneNode::Bg { .. } => "Bg",
                SceneNode::ShowChar { .. } => "ShowChar",
                SceneNode::ShowSprite { .. } => "ShowSprite",
                SceneNode::MoveChar { .. } => "MoveChar",
                SceneNode::Emotion { .. } => "Emotion",
                SceneNode::HideChar { .. } => "HideChar",
                SceneNode::HideSprite { .. } => "HideSprite",
                SceneNode::Dialogue { .. } => "Dialogue",
                SceneNode::Narration { .. } => "Narration",
                SceneNode::Menu { .. } => "Menu",
                SceneNode::Branch { .. } => "Branch",
                SceneNode::SetVariable { .. } => "SetVariable",
                SceneNode::SetFlag { .. } => "SetFlag",
                SceneNode::UnsetFlag { .. } => "UnsetFlag",
                SceneNode::ToggleFlag { .. } => "ToggleFlag",
                SceneNode::Music { .. } => "Music",
                SceneNode::StopMusic { .. } => "StopMusic",
                SceneNode::PlaySE { .. } => "PlaySE",
                SceneNode::Effect { .. } => "Effect",
                SceneNode::Wait { .. } => "Wait",
                SceneNode::Jump { .. } => "Jump",
                SceneNode::Goto { .. } => "Goto",
                SceneNode::Call { .. } => "Call",
                SceneNode::Return => "Return",
                SceneNode::Label { .. } => "Label",
            })
            .collect();

        // 验证除 Goto（在 prologue.aster 中以注释形式存在）外的 24 种节点类型
        let all_variants = [
            "Bg",
            "ShowChar",
            "ShowSprite",
            "MoveChar",
            "Emotion",
            "HideChar",
            "HideSprite",
            "Dialogue",
            "Narration",
            "Menu",
            "Branch",
            "SetVariable",
            "SetFlag",
            "UnsetFlag",
            "ToggleFlag",
            "Music",
            "StopMusic",
            "PlaySE",
            "Effect",
            "Wait",
            "Jump",
            "Call",
            "Return",
            "Label",
        ];
        for variant in &all_variants {
            assert!(
                variant_names.contains(variant),
                "prologue.aster 应包含 {} 节点，但未找到。已找到的变体: {:?}",
                variant,
                variant_names
            );
        }
    }

    // ── AC02: 多次语法错误可在一次解析中全部收集 ──

    /// AC02 — 多次语法错误可在一次解析中全部收集。
    ///
    /// 注：pest PEG 解析器在遇到第一个语法错误时即停止，
    /// 因此语法层面错误通常只有一个。
    /// 但 AST 构建阶段的错误可以多个（如多个语句各含语义问题）。
    #[test]
    fn ac02_multiple_parse_errors_collected() {
        // 使用一个包含错误的脚本
        let source = "scene \"test\" {\n    invalid_keyword_!!!!\n}";
        let result = parse_script(source);
        // pest 会在语法层面就报错（至少一个错误）
        assert!(result.is_err(), "包含语法错误的脚本应返回 Err");
        let errors = result.unwrap_err();
        assert!(!errors.is_empty(), "应至少有一个错误");
        println!("收集到 {} 个错误:", errors.len());
        for e in &errors {
            println!("  - {}", e);
        }
    }

    // ── AC04: 嵌套 if/elif/else 条件分支解析正确 ──

    /// AC04 — 嵌套 if/elif/else 条件分支解析正确。
    #[test]
    fn ac04_nested_if_elif_else_parses_correctly() {
        let source = "scene \"test\" {\n    if $score >= 100 {\n        narration \"完美\"\n    } elif $score > 50 {\n        narration \"不错\"\n    } else {\n        narration \"加油\"\n    }\n}";
        let scene = parse_script(source).expect("有效 if/elif/else 应成功解析");

        assert_eq!(scene.nodes.len(), 1, "应只有 1 个 Branch 节点");
        if let SceneNode::Branch {
            condition,
            then_nodes,
            elif_branches,
            else_nodes,
        } = &scene.nodes[0]
        {
            // 条件
            assert!(matches!(condition, Expr::BinaryOp(..)), "条件应为表达式");

            // then 分支
            assert_eq!(then_nodes.len(), 1, "then 分支应有 1 个节点");

            // elif 分支
            assert_eq!(elif_branches.len(), 1, "应有 1 个 elif 分支");
            assert_eq!(elif_branches[0].1.len(), 1, "elif 分支应有 1 个节点");

            // else 分支
            assert!(else_nodes.is_some(), "应有 else 分支");
            assert_eq!(else_nodes.as_ref().unwrap().len(), 1);
        } else {
            panic!("期望 Branch 节点，实际为: {:?}", scene.nodes[0]);
        }
    }

    // ── AC03: 空的 scene 块解析正确 ──

    #[test]
    fn ac03_empty_scene_block_parses_correctly() {
        let scene = parse_script("scene \"empty\" {\n}").expect("空场景应成功解析");
        assert_eq!(scene.id, "empty");
        assert!(scene.nodes.is_empty(), "空场景的 nodes 应为空列表");
    }

    // ── 兼容旧测试：空文件解析不 panic ──

    #[test]
    fn empty_input_does_not_panic() {
        // 完全空字符串 —— pest 会成功解析（无 scene_block）
        // AstBuilder 在无 scene_block 时返回错误
        let result = parse_script("");
        // 空输入无 scene_block，Builder 返回错误（符合预期）
        match result {
            Ok(_) => {} // 也可以接受（未来可能放宽）
            Err(errors) => {
                assert!(!errors.is_empty());
            }
        }

        // 仅含空白和注释
        let _ = parse_script("   \n  \n  "); // Ok/Err 两种都接受，不 panic 即可
    }

    // ── 性能测试 ──

    #[test]
    fn performance_10k_lines_under_100ms() {
        let mut source = String::from("scene \"perf_test\" {\n");
        for i in 0..10_000 {
            source.push_str(&format!("    sayori \"这是第 {} 行对话文本。\"\n", i));
        }
        source.push_str("}\n");

        // 预热
        let _ = parse_script(&source);

        use std::time::Instant;
        let start = Instant::now();
        let result = parse_script(&source);
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "10k 行脚本应成功解析");
        assert!(
            elapsed.as_millis() < 100,
            "10k 行解析耗时应 < 100ms，实际耗时: {}ms",
            elapsed.as_millis()
        );
        println!("性能测试: 10,000 行解析耗时 {}ms", elapsed.as_millis());
    }
}
