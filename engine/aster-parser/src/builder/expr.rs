//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-parser/src/builder/expr.rs
//! 功能概述：表达式 AST 构建 — 优先级链递归下降，支持全部 7 种 Expr 变体 + 14 种运算符。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13

use pest::iterators::Pair;

use aster_core::{BinaryOp, Expr, UnaryOp};

use super::{err_at, extract_string_content};
use crate::error::ParseError;
use crate::parser::Rule;

/// 表达式解析入口 — 支持全部 7 种 Expr 变体 + 14 种运算符的完整表达式树构建。
///
/// 优先级链（从低到高）：
/// ```text
/// expr → expr_or → expr_and → expr_compare → expr_add → expr_mul → unary → primary
/// or   <   and   <   ==/!=/</>/<=/>=  <  +/-  <  *//  <  not/-  <  字面量/变量/括号
/// ```
/// 每层通过 `(~ operator ~ operand)*` 实现左结合。
pub fn build_expr(pair: &Pair<Rule>, source: &str) -> Result<Expr, ParseError> {
    match pair.as_rule() {
        // 入口：expr → 解包 expr_or
        Rule::expr => {
            let inner = pair.clone().into_inner().next();
            match inner {
                Some(p) => build_expr(&p, source),
                None => Err(err_at(pair, "表达式为空")),
            }
        }

        // 逻辑或：左结合折叠 "or"
        Rule::expr_or => {
            let mut inner = pair.clone().into_inner();
            let mut left = build_expr(
                &inner
                    .next()
                    .ok_or_else(|| err_at(pair, "表达式缺少左操作数"))?,
                source,
            )?;
            for right_pair in inner {
                let right = build_expr(&right_pair, source)?;
                left = Expr::BinaryOp(Box::new(left), BinaryOp::Or, Box::new(right));
            }
            Ok(left)
        }

        // 逻辑与：左结合折叠 "and"
        Rule::expr_and => {
            let mut inner = pair.clone().into_inner();
            let mut left = build_expr(
                &inner
                    .next()
                    .ok_or_else(|| err_at(pair, "表达式缺少左操作数"))?,
                source,
            )?;
            for right_pair in inner {
                let right = build_expr(&right_pair, source)?;
                left = Expr::BinaryOp(Box::new(left), BinaryOp::And, Box::new(right));
            }
            Ok(left)
        }

        // 比较表达式：== != < > <= >=
        Rule::expr_compare => {
            let children: Vec<Pair<Rule>> = pair.clone().into_inner().collect();
            let mut left = build_expr(
                children
                    .first()
                    .ok_or_else(|| err_at(pair, "比较表达式缺少左操作数"))?,
                source,
            )?;
            let mut i = 1;
            while i < children.len() {
                if children[i].as_rule() == Rule::compare_op {
                    let op = build_compare_op(&children[i])?;
                    i += 1;
                    if i < children.len() {
                        let right = build_expr(&children[i], source)?;
                        left = Expr::BinaryOp(Box::new(left), op, Box::new(right));
                    }
                }
                i += 1;
            }
            Ok(left)
        }

        // 加法表达式：+/-
        Rule::expr_add => {
            let children: Vec<Pair<Rule>> = pair.clone().into_inner().collect();
            let mut left = build_expr(
                children
                    .first()
                    .ok_or_else(|| err_at(pair, "加法表达式缺少左操作数"))?,
                source,
            )?;
            let mut i = 1;
            while i < children.len() {
                if children[i].as_rule() == Rule::add_op {
                    let op = build_add_op(&children[i])?;
                    i += 1;
                    if i < children.len() {
                        let right = build_expr(&children[i], source)?;
                        left = Expr::BinaryOp(Box::new(left), op, Box::new(right));
                    }
                }
                i += 1;
            }
            Ok(left)
        }

        // 乘法表达式：*//
        Rule::expr_mul => {
            let children: Vec<Pair<Rule>> = pair.clone().into_inner().collect();
            let mut left = build_expr(
                children
                    .first()
                    .ok_or_else(|| err_at(pair, "乘法表达式缺少左操作数"))?,
                source,
            )?;
            let mut i = 1;
            while i < children.len() {
                if children[i].as_rule() == Rule::mul_op {
                    let op = build_mul_op(&children[i])?;
                    i += 1;
                    if i < children.len() {
                        let right = build_expr(&children[i], source)?;
                        left = Expr::BinaryOp(Box::new(left), op, Box::new(right));
                    }
                }
                i += 1;
            }
            Ok(left)
        }

        // 一元表达式：unary_op* ~ primary
        Rule::unary => {
            let children: Vec<Pair<Rule>> = pair.clone().into_inner().collect();
            if children.is_empty() {
                return Err(err_at(pair, "一元表达式缺少操作数"));
            }
            // 收集一元操作符（not / -）
            let mut unary_ops: Vec<UnaryOp> = Vec::new();
            let mut primary_idx = 0;
            for (i, child) in children.iter().enumerate() {
                if child.as_rule() == Rule::unary_op {
                    unary_ops.push(match child.as_str() {
                        "not" => UnaryOp::Not,
                        "-" => UnaryOp::Neg,
                        _ => {
                            return Err(err_at(
                                child,
                                format!("未知一元运算符：{}", child.as_str()),
                            ));
                        }
                    });
                } else {
                    primary_idx = i;
                    break;
                }
            }
            let mut expr = build_primary(&children[primary_idx], source)?;
            // 从内向外包裹（最后收集的运算符最外层）
            for op in unary_ops.into_iter().rev() {
                expr = Expr::UnaryOp(op, Box::new(expr));
            }
            Ok(expr)
        }

        // 其他规则 → 尝试作为 primary 处理（兼容直接出现的字面量/引用）
        _ => build_primary(pair, source),
    }
}

/// 构建基本表达式（字面量/变量引用/旗标引用/括号表达式/隐式字符串）。
///
/// `primary` 在语法中是静默规则（`_{}`），其替代规则直接出现在 unary 的 inner 中。
pub fn build_primary(pair: &Pair<Rule>, source: &str) -> Result<Expr, ParseError> {
    match pair.as_rule() {
        Rule::string_literal => Ok(Expr::string_literal(extract_string_content(pair))),
        Rule::int_literal => {
            let val: i64 = pair
                .as_str()
                .parse()
                .map_err(|_| err_at(pair, format!("无效整数：{}", pair.as_str())))?;
            Ok(Expr::int_literal(val))
        }
        Rule::float_literal => {
            let val: f64 = pair
                .as_str()
                .parse()
                .map_err(|_| err_at(pair, format!("无效浮点数：{}", pair.as_str())))?;
            Ok(Expr::float_literal(val))
        }
        Rule::bool_literal => Ok(Expr::bool_literal(pair.as_str() == "true")),
        Rule::variable_ref => {
            let name = pair.as_str().strip_prefix('$').unwrap_or(pair.as_str());
            Ok(Expr::variable(name))
        }
        Rule::flag_ref => {
            let name = pair.as_str().strip_prefix('%').unwrap_or(pair.as_str());
            Ok(Expr::variable(name)) // 旗标在 Expr 中映射为 Variable
        }
        Rule::identifier => Ok(Expr::string_literal(pair.as_str())), // 裸标识符 → 隐式字符串
        // 括号表达式："(" ~ expr ~ ")" —— primary 为静默规则，括号被消费，留下 expr
        Rule::expr => build_expr(pair, source),
        _ => Err(err_at(
            pair,
            format!(
                "意外的表达式标记：{:?} ('{}')",
                pair.as_rule(),
                pair.as_str()
            ),
        )),
    }
}

/// 构建比较运算符：== != < > <= >=
fn build_compare_op(pair: &Pair<Rule>) -> Result<BinaryOp, ParseError> {
    match pair.as_str() {
        "==" => Ok(BinaryOp::Eq),
        "!=" => Ok(BinaryOp::Neq),
        "<" => Ok(BinaryOp::Lt),
        ">" => Ok(BinaryOp::Gt),
        "<=" => Ok(BinaryOp::Le),
        ">=" => Ok(BinaryOp::Ge),
        other => Err(err_at(pair, format!("未知比较运算符：{}", other))),
    }
}

/// 构建加减运算符：+ -
fn build_add_op(pair: &Pair<Rule>) -> Result<BinaryOp, ParseError> {
    match pair.as_str() {
        "+" => Ok(BinaryOp::Add),
        "-" => Ok(BinaryOp::Sub),
        other => Err(err_at(pair, format!("未知加减运算符：{}", other))),
    }
}

/// 构建乘除运算符：* /
fn build_mul_op(pair: &Pair<Rule>) -> Result<BinaryOp, ParseError> {
    match pair.as_str() {
        "*" => Ok(BinaryOp::Mul),
        "/" => Ok(BinaryOp::Div),
        other => Err(err_at(pair, format!("未知乘除运算符：{}", other))),
    }
}
