//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-parser/src/builder/position.rs
//! 功能概述：位置与转场构建 — `build_position()` / `build_transition()` + 自定义坐标解析。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13

use pest::iterators::Pair;

use aster_core::{Expr, Position, TransitionSpec};

use super::err_at;
use super::expr::build_expr;
use crate::error::ParseError;
use crate::parser::Rule;

/// 构建立绘位置 — 预设位置（left/center/right）或自定义坐标 (x, y)。
///
/// ```text
/// position = { preset_position | custom_position }
/// preset_position = { "left" | "center" | "right" }
/// custom_position = { "(" ~ expr ~ "," ~ expr ~ ")" }
/// ```
pub fn build_position(pair: &Pair<Rule>, source: &str) -> Result<Position, ParseError> {
    // position 规则是命名规则（非静默），需要先解包到内部变体
    let actual = if pair.as_rule() == Rule::position {
        pair.clone()
            .into_inner()
            .next()
            .ok_or_else(|| err_at(pair, "位置规则为空"))?
    } else {
        pair.clone()
    };
    match actual.as_rule() {
        Rule::preset_position => match actual.as_str() {
            "left" => Ok(Position::Left),
            "center" => Ok(Position::Center),
            "right" => Ok(Position::Right),
            other => Err(err_at(&actual, format!("未知的预设位置：{}", other))),
        },
        Rule::custom_position => {
            let (x, y) = build_custom_position(&actual, source)?;
            Ok(Position::Custom(x, y))
        }
        _ => Err(err_at(
            &actual,
            format!("意外的位置规则：{:?}", actual.as_rule()),
        )),
    }
}

/// 构建自定义坐标 (x, y) — x 和 y 均为 Expr。
///
/// 用于 ShowSprite 的 at 子句和 Position::Custom 变体。
pub fn build_custom_position(pair: &Pair<Rule>, source: &str) -> Result<(Expr, Expr), ParseError> {
    let mut inner = pair.clone().into_inner();
    let x = build_expr(
        &inner
            .next()
            .ok_or_else(|| err_at(pair, "自定义坐标缺少 X 值"))?,
        source,
    )?;
    let y = build_expr(
        &inner
            .next()
            .ok_or_else(|| err_at(pair, "自定义坐标缺少 Y 值"))?,
        source,
    )?;
    Ok((x, y))
}

/// 构建转场效果规格 — fade / dissolve / slide。
///
/// ```text
/// transition = { fade_transition | dissolve_transition | slide_transition }
/// fade_transition      = { "fade" ~ "(" ~ expr ~ ")" }
/// dissolve_transition  = { "dissolve" ~ "(" ~ expr ~ ")" }
/// slide_transition     = { "slide" ~ "(" ~ slide_direction ~ "," ~ expr ~ ")" }
/// ```
pub fn build_transition(pair: &Pair<Rule>, source: &str) -> Result<TransitionSpec, ParseError> {
    // transition 规则是命名规则（非静默），需要先解包到内部的具体变体
    let actual = if pair.as_rule() == Rule::transition {
        pair.clone()
            .into_inner()
            .next()
            .ok_or_else(|| err_at(pair, "转场规则为空"))?
    } else {
        pair.clone()
    };
    match actual.as_rule() {
        Rule::fade_transition => {
            let mut inner = actual.clone().into_inner();
            let duration = build_expr(&inner.next().unwrap(), source)?;
            Ok(TransitionSpec {
                kind: "fade".into(),
                duration_ms: duration,
            })
        }
        Rule::dissolve_transition => {
            let mut inner = actual.clone().into_inner();
            let duration = build_expr(&inner.next().unwrap(), source)?;
            Ok(TransitionSpec {
                kind: "dissolve".into(),
                duration_ms: duration,
            })
        }
        Rule::slide_transition => {
            let mut inner = actual.clone().into_inner();
            let dir_pair = inner
                .next()
                .ok_or_else(|| err_at(&actual, "slide 转场缺少方向"))?;
            let direction = dir_pair.as_str();
            let dur_pair = inner
                .next()
                .ok_or_else(|| err_at(&actual, "slide 转场缺少时长"))?;
            let duration = build_expr(&dur_pair, source)?;
            Ok(TransitionSpec {
                kind: format!("slide_{}", direction),
                duration_ms: duration,
            })
        }
        _ => Err(err_at(
            &actual,
            format!("意外的转场规则：{:?}", actual.as_rule()),
        )),
    }
}
