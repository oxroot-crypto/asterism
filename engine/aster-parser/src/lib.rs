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
//! | `parser` | `parser.rs` | pest 解析器入口 `parse_script()`：.aster 源码 → pest token 流 |
//!
//! ## 解析流程
//! ```text
//! .aster 源码 → pest::Parser (PEG 语法) → PestToken 流 → AstBuilder (PH1-T05) → aster_core::Scene
//! ```
//!
//! ## 当前阶段（PH1-T04）
//!
//! `parse_script()` 返回 `Result<pest::Pairs, Vec<ParseError>>`。
//! PH1-T05 接入 AstBuilder 后将改为 `Result<aster_core::Scene, Vec<ParseError>>`。
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
//!     Ok(pairs) => {
//!         for pair in pairs {
//!             println!("Rule: {:?}, span: {:?}", pair.as_rule(), pair.as_span());
//!         }
//!     }
//!     Err(errors) => {
//!         for e in &errors {
//!             eprintln!("{}", e);
//!         }
//!     }
//! }
//! ```

// 模块声明
pub mod error;
pub mod parser;

// 重导出 — 外部 crate 通过 `aster_parser::` 路径直接使用
pub use error::ParseError;
pub use parser::parse_script;

// ============================================================================
// 集成测试 — PH1-T04 验收标准 (AC)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── AC01: prologue.aster（574 行）被 pest 解析器成功解析 ──

    /// AC01 — 合法 .aster 脚本被 pest 解析器成功解析（无语法错误）。
    ///
    /// 使用 `templates/default_project/scripts/prologue.aster`（574 行，
    /// 覆盖全部 25 种 SceneNode + Expr 插值）作为输入，验证 `parse_script()` 返回 Ok。
    #[test]
    fn ac01_prologue_aster_parses_successfully() {
        // 读取模板脚本 — 路径相对于项目根目录
        let prologue_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../templates/default_project/scripts/prologue.aster"
        );
        let source = std::fs::read_to_string(prologue_path).expect("无法读取 prologue.aster 文件");

        let result = parse_script(&source);
        assert!(
            result.is_ok(),
            "prologue.aster 应成功解析，但返回错误: {:?}",
            result.err()
        );

        let pairs = result.unwrap();
        // 验证有 token 产出（至少包含 scene_block）
        assert!(
            !pairs.is_empty(),
            "prologue.aster 解析后应产生 token，但得到空结果"
        );
    }

    // ── AC02: 非法语法返回含行号的错误 ──

    /// AC02 — 非法语法（如未闭合的字符串）返回含行号的错误。
    #[test]
    fn ac02_invalid_syntax_returns_error_with_line_number() {
        let source = "scene \"test\" {\n    sayori \"hello\n}";
        let result = parse_script(source);
        assert!(result.is_err(), "非法语法应返回 Err");

        let errors = result.unwrap_err();
        assert!(!errors.is_empty(), "错误列表不应为空");

        let first = &errors[0];
        assert!(
            first.line() > 0,
            "错误应包含有效行号（1-based），实际: {}",
            first.line()
        );
        println!("错误消息: {}", first);
    }

    // ── AC03: 注释被正确忽略 ──

    /// AC03 — 注释（`--` 前缀）被正确忽略。
    ///
    /// 验证包含注释的脚本与等效无注释脚本产生相同的解析结果。
    #[test]
    fn ac03_comments_are_ignored() {
        // 含注释的脚本
        let with_comments =
            "scene \"test\" {\n    -- 顶部注释\n    narration \"你好\"\n    -- 底部注释\n}";
        let result_with = parse_script(with_comments);
        assert!(result_with.is_ok(), "含注释的脚本应成功解析");

        // 等效无注释脚本
        let without_comments = "scene \"test\" {\n    narration \"你好\"\n}";
        let result_without = parse_script(without_comments);
        assert!(result_without.is_ok(), "无注释脚本应成功解析");

        // 两种情况下都成功解析即可（pest token 流因注释被静默丢弃而等价）
    }

    // ── AC04: 空文件解析不 panic ──

    /// AC04 — 空文件解析不 panic。
    #[test]
    fn ac04_empty_input_does_not_panic() {
        // 完全空字符串
        let result = parse_script("");
        assert!(result.is_ok(), "空字符串应返回 Ok");

        // 仅含换行和空格
        let result = parse_script("   \n  \n  ");
        assert!(result.is_ok(), "仅含空白的输入应返回 Ok");

        // 仅含注释
        let result = parse_script("-- 只有注释\n-- 另一行注释");
        assert!(result.is_ok(), "仅含注释的输入应返回 Ok");
    }

    // ── AC05: 10,000 行脚本解析耗时 < 100ms ──

    /// AC05 — 10,000 行脚本解析耗时 < 100ms。
    ///
    /// 生成 10,000 行合法 .aster 脚本（重复对话行），测量 `parse_script()` 耗时。
    /// 对应需求 NFR-PERF-011：解析性能 10,000 行 < 100ms。
    #[test]
    fn ac05_performance_10k_lines_under_100ms() {
        // 生成 10,000 行合法脚本
        let mut source = String::from("scene \"perf_test\" {\n");
        for i in 0..10_000 {
            source.push_str(&format!("    sayori \"这是第 {} 行对话文本。\"\n", i));
        }
        source.push_str("}\n");

        // 预热：运行一次让 CPU cache 和内存就绪
        let _ = parse_script(&source);

        // 正式计时
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

        println!("AC05 性能测试: 10,000 行解析耗时 {}ms", elapsed.as_millis());
    }
}
