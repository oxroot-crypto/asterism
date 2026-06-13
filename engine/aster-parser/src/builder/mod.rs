//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-parser/src/builder/mod.rs
//! 功能概述：AST 构建器模块入口 — `AstBuilder` 结构体、场景构建、辅助函数。
//!           表达式构建见 `expr.rs`，位置/转场见 `position.rs`，各语句见 `statements.rs`。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13

pub mod expr;
pub mod position;
pub mod statements;

use pest::iterators::{Pair, Pairs};

use aster_core::{Scene, SceneNode};

use crate::error::ParseError;
use crate::parser::Rule;

// ============================================================================
// 辅助函数（模块内共享）
// ============================================================================

/// 从 pest Pair 提取 1-based 源码位置：(line, column, offset)。
pub(crate) fn pos_from_pair(pair: &Pair<Rule>) -> (usize, usize, usize) {
    let span = pair.as_span();
    let (line, col) = span.start_pos().line_col();
    let offset = span.start() + 1;
    (line, col, offset)
}

/// 创建携带源码位置的 ParseError。
pub(crate) fn err_at(pair: &Pair<Rule>, message: impl Into<String>) -> ParseError {
    let (line, col, offset) = pos_from_pair(pair);
    ParseError::new((line, col, offset), message.into(), None, String::new())
}

/// 创建带修复建议的 ParseError。
#[allow(dead_code)]
pub(crate) fn err_with_hint(
    pair: &Pair<Rule>,
    message: impl Into<String>,
    hint: impl Into<String>,
) -> ParseError {
    let (line, col, offset) = pos_from_pair(pair);
    ParseError::new(
        (line, col, offset),
        message.into(),
        Some(hint.into()),
        String::new(),
    )
}

/// 字符串字面量反转义（`\"` → `"`、`\\` → `\`、`\n` → 换行、`\t` → 制表）。
pub(crate) fn unescape_string(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// 从字符串字面量 pair 提取内容（去首尾双引号 + 反转义）。
pub(crate) fn extract_string_content(pair: &Pair<Rule>) -> String {
    let raw = pair.as_str();
    let inner = if raw.len() >= 2 {
        &raw[1..raw.len() - 1]
    } else {
        raw
    };
    unescape_string(inner)
}

// ============================================================================
// AstBuilder — AST 构建器
// ============================================================================

/// AST 构建器 — 将 pest token 流递归下降转换为 `aster_core::Scene`。
///
/// 所有方法为关联函数（静态方法），不持有状态。
pub struct AstBuilder;

impl AstBuilder {
    // ── 顶层入口 ──────────────────────────────────────────────────────

    /// 从 pest token 流构建 `Scene`。Phase 1 单场景模式：返回第一个 scene_block。
    ///
    /// pest parse 返回的 Pairs 外层为 `Rule::script`，其 inner pairs 为 `scene_block*`。
    pub fn build(pairs: Pairs<Rule>, source: &str) -> Result<Scene, Vec<ParseError>> {
        let mut errors: Vec<ParseError> = Vec::new();
        let mut scenes: Vec<Scene> = Vec::new();

        // 外层为 Rule::script pair，进入其 inner 迭代 scene_block*
        for script_pair in pairs {
            for pair in script_pair.into_inner() {
                match pair.as_rule() {
                    Rule::scene_block => match Self::build_scene(&pair, source) {
                        Ok(scene) => scenes.push(scene),
                        Err(e) => errors.push(e),
                    },
                    Rule::EOI => {}
                    other => {
                        errors.push(err_at(
                            &pair,
                            format!("意外的顶层规则：{:?}，期望 scene 块", other),
                        ));
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        // 无 scene_block 时返回空 Scene（空文件/仅注释的输入）
        Ok(scenes.into_iter().next().unwrap_or(Scene {
            id: String::new(),
            label: None,
            background: None,
            music: None,
            nodes: Vec::new(),
        }))
    }

    // ── 场景结构 ──────────────────────────────────────────────────────

    /// `scene_block = { "scene" ~ string_literal ~ "{" ~ scene_body ~ "}" }`
    fn build_scene(pair: &Pair<Rule>, source: &str) -> Result<Scene, ParseError> {
        let mut inner = pair.clone().into_inner();

        let id_pair = inner
            .next()
            .ok_or_else(|| err_at(pair, "场景块缺少 ID 字符串"))?;
        let scene_id = extract_string_content(&id_pair);

        let body_pair = inner
            .next()
            .ok_or_else(|| err_at(pair, "场景块缺少主体内容"))?;

        let (_description, nodes) = match Self::build_scene_body(&body_pair, source) {
            Ok(result) => result,
            Err(mut errors) => return Err(errors.remove(0)),
        };

        Ok(Scene {
            id: scene_id,
            label: None,
            background: None,
            music: None,
            nodes,
        })
    }

    /// `scene_body = { description_line? ~ statement* }`
    /// 返回 `(description, nodes)`。statement 是静默规则，子规则直接出现在 inner pairs 中。
    fn build_scene_body(
        pair: &Pair<Rule>,
        source: &str,
    ) -> Result<(Option<String>, Vec<SceneNode>), Vec<ParseError>> {
        let mut description: Option<String> = None;
        let mut nodes: Vec<SceneNode> = Vec::new();
        let mut errors: Vec<ParseError> = Vec::new();

        for child in pair.clone().into_inner() {
            match child.as_rule() {
                Rule::description_line => {
                    if let Some(desc_pair) = child.into_inner().next() {
                        description = Some(extract_string_content(&desc_pair));
                    }
                }
                _ => match Self::build_statement(&child, source) {
                    Ok(node) => nodes.push(node),
                    Err(e) => errors.push(e),
                },
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok((description, nodes))
    }

    // ── 语句分发 ──────────────────────────────────────────────────────

    /// 根据 pair 规则类型分发到对应构建方法。覆盖全部 25 种 SceneNode 变体。
    fn build_statement(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
        statements::build_statement(pair, source)
    }
}
