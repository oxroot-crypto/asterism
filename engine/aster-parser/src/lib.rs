//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-parser/src/lib.rs
//! 功能概述：.aster DSL 解析器 — 将 `.aster` 脚本源码解析为抽象语法树（AST）。
//!           基于 PEG（Parsing Expression Grammar）语法文件 `aster.pest`，
//!           使用 pest 解析器生成器。输出结构化的 `ParsedScene`。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-12
//!
//! 依赖模块：
//! - aster_core（待 Phase 1 添加）：SceneNode、Choice 等 AST 节点类型
//! - pest（待 Phase 1 添加）：PEG 解析器
//!
//! 架构位置：aster-core ← aster-parser ← aster-compiler

/// .aster DSL 解析器 — 待 Phase 1 实现
///
/// 将定义：
/// - `parse(source: &str) -> Result<ParsedScene, Vec<ParseError>>`：完整场景解析
/// - `parse_expression(source: &str) -> Result<Expr, ParseError>`：单个表达式解析
/// - `ParseError`：携带行号/列号的结构化错误
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // Phase 0 占位测试，Phase 1 实际开发时替换为 AST 解析测试
        assert_eq!(2 + 2, 4);
    }
}
