//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-parser/src/builder/statements.rs
//! 功能概述：25 种 SceneNode 变体对应的语句构建方法。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13

use std::collections::HashMap;

use pest::iterators::Pair;

use aster_core::{Choice, Expr, SceneNode};

use super::expr::build_expr;
use super::position::{build_custom_position, build_position, build_transition};
use super::{err_at, extract_string_content};
use crate::error::ParseError;
use crate::parser::Rule;

/// 语句分发器 — 根据 pair 规则类型调用对应构建函数。
/// 被 `AstBuilder::build_statement` 委托，同时被自身递归调用（Branch/Menu/Choice 内的语句解析）。
///
/// 返回 `Vec<SceneNode>` 而非单个 `SceneNode`，以支持 Menu 等语句展开为多个节点
/// （Menu 自身 + 每个选项 body 对应的 auto-labeled 节点序列）。
pub fn build_statement(pair: &Pair<Rule>, source: &str) -> Result<Vec<SceneNode>, ParseError> {
    match pair.as_rule() {
        Rule::bg_stmt => build_bg(pair, source).map(|n| vec![n]),
        Rule::show_char_stmt => build_show_char(pair, source).map(|n| vec![n]),
        Rule::emotion_stmt => build_emotion(pair, source).map(|n| vec![n]),
        Rule::sprite_stmt => build_sprite(pair, source).map(|n| vec![n]),
        Rule::move_char_stmt => build_move_char(pair, source).map(|n| vec![n]),
        Rule::hide_sprite_stmt => build_hide_sprite(pair, source).map(|n| vec![n]),
        Rule::hide_char_stmt => build_hide_char(pair, source).map(|n| vec![n]),
        Rule::narration_stmt => build_narration(pair, source).map(|n| vec![n]),
        Rule::dialogue_stmt => build_dialogue(pair, source).map(|n| vec![n]),
        Rule::menu_stmt => build_menu(pair, source),
        Rule::branch_stmt => build_branch(pair, source).map(|n| vec![n]),
        Rule::jump_stmt => build_jump(pair, source).map(|n| vec![n]),
        Rule::goto_stmt => build_goto(pair, source).map(|n| vec![n]),
        Rule::call_stmt => build_call(pair, source).map(|n| vec![n]),
        Rule::return_stmt => Ok(vec![SceneNode::Return]),
        Rule::label_stmt => build_label(pair, source).map(|n| vec![n]),
        Rule::sub_def => build_sub(pair, source).map(|n| vec![n]),
        Rule::assignment_stmt => build_assignment(pair, source).map(|n| vec![n]),
        Rule::set_flag_stmt => build_set_flag(pair).map(|n| vec![n]),
        Rule::unset_flag_stmt => build_unset_flag(pair).map(|n| vec![n]),
        Rule::toggle_flag_stmt => build_toggle_flag(pair).map(|n| vec![n]),
        Rule::music_stmt => build_music(pair, source).map(|n| vec![n]),
        Rule::stop_music_stmt => build_stop_music(pair, source).map(|n| vec![n]),
        Rule::se_stmt => build_se(pair, source).map(|n| vec![n]),
        Rule::effect_stmt => build_effect(pair, source).map(|n| vec![n]),
        Rule::wait_stmt => build_wait(pair, source).map(|n| vec![n]),
        other => Err(err_at(pair, format!("意外的语句规则：{:?}", other))),
    }
}

// ========================================================================
// §4.1 背景切换 — SceneNode::Bg
// ========================================================================

/// `bg_stmt = { "bg" ~ expr ~ ("with" ~ transition)? }`
pub fn build_bg(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let asset_path = build_expr(&inner.next().unwrap(), source)?;
    let transition = inner
        .next()
        .map(|p| build_transition(&p, source))
        .transpose()?;
    Ok(SceneNode::Bg {
        asset_path,
        transition,
    })
}

// ========================================================================
// §4.2 音乐 — SceneNode::Music / StopMusic
// ========================================================================

/// `music_stmt = { "music" ~ expr ~ ("fade_in" ~ ":" ~ expr)? ~ ("looping" ~ ":" ~ bool_literal)? }`
pub fn build_music(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let inner: Vec<Pair<Rule>> = pair.clone().into_inner().collect();
    if inner.is_empty() {
        return Err(err_at(pair, "music 语句缺少资源路径"));
    }
    let asset_path = build_expr(&inner[0], source)?;
    let mut fade_in: Option<Expr> = None;
    let mut looping: bool = true;
    for item in &inner[1..] {
        match item.as_rule() {
            Rule::expr if fade_in.is_none() => {
                fade_in = Some(build_expr(item, source)?);
            }
            Rule::expr => {}
            Rule::bool_literal => looping = item.as_str() == "true",
            _ => {}
        }
    }
    Ok(SceneNode::Music {
        asset_path,
        fade_in,
        looping,
    })
}

/// `stop_music_stmt = { "stop_music" ~ ("fade_out" ~ ":" ~ expr)? }`
pub fn build_stop_music(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let fade_out = inner.next().map(|p| build_expr(&p, source)).transpose()?;
    Ok(SceneNode::StopMusic { fade_out })
}

// ========================================================================
// §4.3 音效 — SceneNode::PlaySE
// ========================================================================

/// `se_stmt = { "se" ~ expr ~ ("fade_in" ~ ":" ~ expr)? }`
pub fn build_se(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let asset_id = build_expr(&inner.next().unwrap(), source)?;
    let fade_in = inner.next().map(|p| build_expr(&p, source)).transpose()?;
    Ok(SceneNode::PlaySE { asset_id, fade_in })
}

// ========================================================================
// §4.4 角色显示/表情/移动/隐藏 — ShowChar / Emotion / MoveChar / HideChar
// ========================================================================

/// `show_char_stmt = { "show" ~ expr ~ "at" ~ position ~ ("emotion" ~ ":" ~ expr)? ~ ("with" ~ transition)? }`
pub fn build_show_char(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let char_id = build_expr(&inner.next().unwrap(), source)?;
    let pos_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "show 语句缺少位置"))?;
    let position = build_position(&pos_pair, source)?;
    let mut emotion: Option<Expr> = None;
    let mut transition = None;
    for child in inner {
        match child.as_rule() {
            Rule::expr if emotion.is_none() => {
                emotion = Some(build_expr(&child, source)?);
            }
            Rule::expr => {}
            Rule::transition
            | Rule::fade_transition
            | Rule::dissolve_transition
            | Rule::slide_transition => {
                transition = Some(build_transition(&child, source)?);
            }
            _ => {}
        }
    }
    Ok(SceneNode::ShowChar {
        char_id,
        position,
        emotion,
        transition,
    })
}

/// `emotion_stmt = { "emotion" ~ expr ~ expr ~ ("with" ~ transition)? }`
pub fn build_emotion(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let char_id = build_expr(&inner.next().unwrap(), source)?;
    let emotion = build_expr(
        &inner
            .next()
            .ok_or_else(|| err_at(pair, "emotion 语句缺少表情名"))?,
        source,
    )?;
    let transition = inner
        .next()
        .map(|p| build_transition(&p, source))
        .transpose()?;
    Ok(SceneNode::Emotion {
        char_id,
        emotion,
        transition,
    })
}

/// `move_char_stmt = { "move" ~ expr ~ "to" ~ position ~ ("emotion" ~ ":" ~ expr)? ~ "with" ~ transition }`
pub fn build_move_char(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let char_id = build_expr(&inner.next().unwrap(), source)?;
    let pos_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "move 语句缺少目标位置"))?;
    let position = build_position(&pos_pair, source)?;
    let mut emotion: Option<Expr> = None;
    let mut transition: Option<aster_core::TransitionSpec> = None;
    for child in inner {
        match child.as_rule() {
            Rule::expr if emotion.is_none() => {
                emotion = Some(build_expr(&child, source)?);
            }
            Rule::expr => {}
            Rule::transition
            | Rule::fade_transition
            | Rule::dissolve_transition
            | Rule::slide_transition => {
                transition = Some(build_transition(&child, source)?);
            }
            _ => {}
        }
    }
    let transition = transition.ok_or_else(|| err_at(pair, "move 语句缺少转场效果（with ...）"))?;
    Ok(SceneNode::MoveChar {
        char_id,
        position,
        emotion,
        transition,
    })
}

/// `hide_char_stmt = { "hide" ~ expr ~ ("with" ~ transition)? }`
pub fn build_hide_char(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let char_id = build_expr(&inner.next().unwrap(), source)?;
    let transition = inner
        .next()
        .map(|p| build_transition(&p, source))
        .transpose()?;
    Ok(SceneNode::HideChar {
        char_id,
        transition,
    })
}

// ========================================================================
// §4.5 独立精灵 — ShowSprite / HideSprite
// ========================================================================

/// `sprite_stmt = { "sprite" ~ expr ~ "at" ~ custom_position ~ ("scale" ~ ":" ~ expr)? ~ ("alpha" ~ ":" ~ expr)? ~ ("with" ~ transition)? }`
pub fn build_sprite(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let inner: Vec<Pair<Rule>> = pair.clone().into_inner().collect();
    if inner.len() < 2 {
        return Err(err_at(pair, "sprite 语句不完整"));
    }
    let asset_path = build_expr(&inner[0], source)?;
    let (x, y) = build_custom_position(&inner[1], source)?;
    let mut scale = Expr::float_literal(1.0);
    let mut alpha = Expr::float_literal(1.0);
    let mut transition = None;
    let mut next_is_scale = true;
    for item in &inner[2..] {
        match item.as_rule() {
            Rule::expr => {
                if next_is_scale {
                    scale = build_expr(item, source)?;
                    next_is_scale = false;
                } else {
                    alpha = build_expr(item, source)?;
                }
            }
            Rule::transition
            | Rule::fade_transition
            | Rule::dissolve_transition
            | Rule::slide_transition => {
                transition = Some(build_transition(item, source)?);
            }
            _ => {}
        }
    }
    Ok(SceneNode::ShowSprite {
        asset_path,
        x,
        y,
        scale,
        alpha,
        transition,
    })
}

/// `hide_sprite_stmt = { "usprite" ~ expr ~ ("with" ~ transition)? }`
pub fn build_hide_sprite(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let asset_path = build_expr(&inner.next().unwrap(), source)?;
    let transition = inner
        .next()
        .map(|p| build_transition(&p, source))
        .transpose()?;
    Ok(SceneNode::HideSprite {
        asset_path,
        transition,
    })
}

// ========================================================================
// §4.6 对话/旁白 — Dialogue / Narration
// ========================================================================

/// `dialogue_stmt = { identifier ~ expr ~ ("voice" ~ ":" ~ expr)? }`
pub fn build_dialogue(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let speaker_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "对话语句缺少说话者"))?;
    let speaker = Expr::string_literal(speaker_pair.as_str());
    let text = build_expr(
        &inner
            .next()
            .ok_or_else(|| err_at(pair, "对话语句缺少文本内容"))?,
        source,
    )?;
    let voice_id = inner.next().map(|p| build_expr(&p, source)).transpose()?;
    Ok(SceneNode::Dialogue {
        speaker,
        text,
        voice_id,
    })
}

/// `narration_stmt = { "narration" ~ expr }`
pub fn build_narration(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let text = build_expr(&inner.next().unwrap(), source)?;
    Ok(SceneNode::Narration { text })
}

// ========================================================================
// §4.7 菜单 — Menu
// ========================================================================

/// `menu_stmt = { "menu" ~ expr ~ "{" ~ choice_block* ~ "}" }`
///
/// 返回多个 SceneNode：[Menu, Label("@menu_choice_0"), <body_0...>, Label("@menu_choice_1"), <body_1...>, ...]
/// 每个选项的 body 语句被保留为 auto-labeled 节点序列，Choice.target 指向对应的 auto-label。
pub fn build_menu(pair: &Pair<Rule>, source: &str) -> Result<Vec<SceneNode>, ParseError> {
    let mut inner = pair.clone().into_inner();
    let prompt = build_expr(&inner.next().unwrap(), source)?;
    let mut choices: Vec<Choice> = Vec::new();
    let mut body_sections: Vec<SceneNode> = Vec::new();

    // 全局唯一计数器 — 避免场景内多个 Menu 的 @menu_choice_N 标签名冲突
    use std::sync::atomic::{AtomicUsize, Ordering};
    static MENU_COUNTER: AtomicUsize = AtomicUsize::new(0);

    let menu_end_id = MENU_COUNTER.fetch_add(1, Ordering::Relaxed);
    let end_label = format!("@menu_end_{}", menu_end_id);

    for child in inner {
        if child.as_rule() == Rule::choice_block {
            let choice_idx = MENU_COUNTER.fetch_add(1, Ordering::Relaxed);
            let (choice, body_nodes) = build_choice_with_body(&child, choice_idx, source)?;
            let auto_label = format!("@menu_choice_{}", choice_idx);
            // 前一个 body 结束后插入 Jump 到 @menu_end，防止 fall-through
            if !body_sections.is_empty() {
                body_sections.push(SceneNode::Jump {
                    target: aster_core::Expr::string_literal(end_label.clone()),
                });
            }
            body_sections.push(SceneNode::Label {
                name: auto_label.clone(),
            });
            body_sections.extend(body_nodes);
            choices.push(choice);
        }
    }
    // 最后一个 body 结束后也加 Jump
    if !body_sections.is_empty() {
        body_sections.push(SceneNode::Jump {
            target: aster_core::Expr::string_literal(end_label.clone()),
        });
    }

    let mut result = vec![SceneNode::Menu { prompt, choices }];
    result.extend(body_sections);
    result.push(SceneNode::Label { name: end_label });
    Ok(result)
}

/// `choice_block = { expr ~ ("if" ~ expr)? ~ "{" ~ statement* ~ "}" }`
///
/// 返回 (Choice, body_nodes)：
/// - Choice.target 设为 `@menu_choice_{idx}` auto-label
/// - body_nodes 包含选项被选中后执行的所有语句
fn build_choice_with_body(
    pair: &Pair<Rule>,
    choice_idx: usize,
    source: &str,
) -> Result<(Choice, Vec<SceneNode>), ParseError> {
    let mut inner = pair.clone().into_inner();
    let text = build_expr(
        &inner
            .next()
            .ok_or_else(|| err_at(pair, "选项缺少显示文本"))?,
        source,
    )?;
    let mut condition: Option<Expr> = None;
    let mut body_nodes: Vec<SceneNode> = Vec::new();
    for child in inner {
        match child.as_rule() {
            Rule::expr => {
                if condition.is_none() {
                    condition = Some(build_expr(&child, source)?);
                }
            }
            _ => {
                // statement 子节点（statement 为静默规则，子规则直接出现）
                match build_statement(&child, source) {
                    Ok(new_nodes) => body_nodes.extend(new_nodes),
                    Err(e) => return Err(e),
                }
            }
        }
    }
    // Choice.target 指向 auto-label，VM 选中后跳转到对应 body
    let target = Expr::string_literal(format!("@menu_choice_{}", choice_idx));
    Ok((
        Choice {
            text,
            target,
            condition,
        },
        body_nodes,
    ))
}

// ========================================================================
// §4.8 条件分支 — Branch
// ========================================================================

/// `branch_stmt = { "if" ~ expr ~ "{" ~ statement* ~ "}" ~ elif_branch* ~ else_branch? }`
pub fn build_branch(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let inner: Vec<Pair<Rule>> = pair.clone().into_inner().collect();
    if inner.is_empty() {
        return Err(err_at(pair, "if 分支缺少条件表达式"));
    }
    let condition = build_expr(&inner[0], source)?;
    let mut idx = 1;
    // 收集 if block 内的语句
    let mut then_nodes: Vec<SceneNode> = Vec::new();
    while idx < inner.len()
        && !matches!(inner[idx].as_rule(), Rule::elif_branch | Rule::else_branch)
    {
        match build_statement(&inner[idx], source) {
            Ok(new_nodes) => then_nodes.extend(new_nodes),
            Err(e) => return Err(e),
        }
        idx += 1;
    }
    // elif 分支
    let mut elif_branches: Vec<(Expr, Vec<SceneNode>)> = Vec::new();
    while idx < inner.len() && inner[idx].as_rule() == Rule::elif_branch {
        let elif_inner: Vec<Pair<Rule>> = inner[idx].clone().into_inner().collect();
        if !elif_inner.is_empty() {
            let elif_cond = build_expr(&elif_inner[0], source)?;
            let mut elif_nodes: Vec<SceneNode> = Vec::new();
            for node_pair in &elif_inner[1..] {
                match build_statement(node_pair, source) {
                    Ok(new_nodes) => elif_nodes.extend(new_nodes),
                    Err(e) => return Err(e),
                }
            }
            elif_branches.push((elif_cond, elif_nodes));
        }
        idx += 1;
    }
    // else 分支
    let mut else_nodes: Option<Vec<SceneNode>> = None;
    if idx < inner.len() && inner[idx].as_rule() == Rule::else_branch {
        let mut nodes: Vec<SceneNode> = Vec::new();
        for node_pair in inner[idx].clone().into_inner() {
            match build_statement(&node_pair, source) {
                Ok(new_nodes) => nodes.extend(new_nodes),
                Err(e) => return Err(e),
            }
        }
        else_nodes = Some(nodes);
    }
    Ok(SceneNode::Branch {
        condition,
        then_nodes,
        elif_branches,
        else_nodes,
    })
}

// ========================================================================
// §4.9 控制流 — Jump / Goto / Call / Label
// ========================================================================

pub fn build_jump(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let target = build_expr(&inner.next().unwrap(), source)?;
    Ok(SceneNode::Jump { target })
}

pub fn build_goto(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let scene_id = build_expr(&inner.next().unwrap(), source)?;
    let label = inner.next().map(|p| build_expr(&p, source)).transpose()?;
    Ok(SceneNode::Goto { scene_id, label })
}

/// 构建子例程调用节点。
///
/// 语法：`name()` 或 `name(arg1, arg2, ...)`（函数式调用）
/// 产出 `SceneNode::Call { name, args }`。
/// `name` 为子例程标识符，`args` 为参数表达式列表（预留）。
pub fn build_call(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let name_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "call 语句缺少子例程名"))?;
    let name = name_pair.as_str().to_string();
    let mut args: Vec<Expr> = Vec::new();
    for arg_pair in inner {
        args.push(build_expr(&arg_pair, source)?);
    }
    Ok(SceneNode::Call { name, args })
}

pub fn build_label(pair: &Pair<Rule>, _source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let name_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "label 语句缺少标签名"))?;
    let name = match name_pair.as_rule() {
        Rule::string_literal => extract_string_content(&name_pair),
        Rule::identifier => name_pair.as_str().to_string(),
        _ => name_pair.as_str().to_string(),
    };
    Ok(SceneNode::Label { name })
}

/// 构建子例程定义节点。
///
/// 语法：`sub "name" { statement* }`
/// 产出 `SceneNode::Subroutine { name, body }`。
/// 子例程仅在被 `call "name"` 时执行，主流程自动跳过。
pub fn build_sub(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let name_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "sub 语句缺少子例程名"))?;
    let name = match name_pair.as_rule() {
        Rule::string_literal => extract_string_content(&name_pair),
        Rule::identifier => name_pair.as_str().to_string(),
        _ => name_pair.as_str().to_string(),
    };
    let mut body: Vec<SceneNode> = Vec::new();
    for child in inner {
        match build_statement(&child, source) {
            Ok(nodes) => body.extend(nodes),
            Err(e) => return Err(e),
        }
    }
    Ok(SceneNode::Subroutine { name, body })
}

// ========================================================================
// §4.10 变量/旗标 — SetVariable / SetFlag / UnsetFlag / ToggleFlag
// ========================================================================

pub fn build_assignment(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let var_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "赋值语句缺少变量名"))?;
    let name = var_pair
        .as_str()
        .strip_prefix('$')
        .unwrap_or(var_pair.as_str())
        .to_string();
    let value = build_expr(
        &inner
            .next()
            .ok_or_else(|| err_at(pair, "赋值语句缺少值表达式"))?,
        source,
    )?;
    Ok(SceneNode::SetVariable { name, value })
}

pub fn build_set_flag(pair: &Pair<Rule>) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let flag_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "set 语句缺少旗标名"))?;
    let name = flag_pair
        .as_str()
        .strip_prefix('%')
        .unwrap_or(flag_pair.as_str())
        .to_string();
    Ok(SceneNode::SetFlag { name })
}

pub fn build_unset_flag(pair: &Pair<Rule>) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let flag_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "unset 语句缺少旗标名"))?;
    let name = flag_pair
        .as_str()
        .strip_prefix('%')
        .unwrap_or(flag_pair.as_str())
        .to_string();
    Ok(SceneNode::UnsetFlag { name })
}

pub fn build_toggle_flag(pair: &Pair<Rule>) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let flag_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "toggle 语句缺少旗标名"))?;
    let name = flag_pair
        .as_str()
        .strip_prefix('%')
        .unwrap_or(flag_pair.as_str())
        .to_string();
    Ok(SceneNode::ToggleFlag { name })
}

// ========================================================================
// §4.11 特效/等待 — Effect / Wait
// ========================================================================

pub fn build_effect(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let type_pair = inner
        .next()
        .ok_or_else(|| err_at(pair, "effect 语句缺少特效类型"))?;
    let effect_type = extract_string_content(&type_pair);
    let mut params: HashMap<String, Expr> = HashMap::new();
    for child in inner {
        if child.as_rule() == Rule::effect_param {
            let mut param_inner = child.into_inner();
            let key_pair = param_inner
                .next()
                .ok_or_else(|| err_at(pair, "effect 参数缺少名称"))?;
            let key = key_pair.as_str().to_string();
            if let Some(val_pair) = param_inner.next() {
                params.insert(key, build_expr(&val_pair, source)?);
            }
        }
    }
    Ok(SceneNode::Effect {
        effect_type,
        params,
    })
}

pub fn build_wait(pair: &Pair<Rule>, source: &str) -> Result<SceneNode, ParseError> {
    let mut inner = pair.clone().into_inner();
    let duration_ms = build_expr(&inner.next().unwrap(), source)?;
    Ok(SceneNode::Wait { duration_ms })
}
