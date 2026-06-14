//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/src/action.rs
//! 功能概述：VM 对外动作枚举 — 定义 VM 在执行字节码过程中需要外部（SceneManager）
//!           处理的"意图"：等待输入、显示菜单、引擎命令、场景结束等。
//!           纯内部指令（数据传送、算术运算、跳转）不产生 VmAction，
//!           在 `step()` 内部循环中静默执行。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - crate::engine_command::EngineCommand

use std::fmt;

use crate::engine_command::EngineCommand;

/// 选择支数据 — 菜单中单个选项的运行时表示。
///
/// 所有字段为常量池索引或字节偏移，由 SceneManager 结合 `CompiledScene.constant_pool`
/// 解析为可显示文本。
#[derive(Debug, Clone, PartialEq)]
pub struct MenuChoiceData {
    /// 选项显示文本的常量池索引
    pub text_idx: u16,
    /// 选中后跳转的目标字节偏移（指向标签位置）
    pub target_offset: u16,
    /// 条件选项关联的旗标名常量池索引（`0xFFFF` = 无条件选项）
    pub condition_flag_idx: u16,
}

/// VM 对外动作枚举 — step() 方法的返回值。
///
/// VM 执行指令时，遇到需要外部系统（SceneManager/渲染器/音频系统）
/// 处理的操作时，返回对应的 `VmAction`。调用方处理后再次调用 `step()`
/// 继续执行。
///
/// **内部指令**（PUSH_STR、ADD、JUMP 等）在 `step()` 循环内静默执行，
/// 不产生 VmAction。
///
/// # 变体说明
///
/// | 变体 | 触发指令 | 调用方行为 |
/// |------|----------|-----------|
/// | `WaitForInput` | DIALOGUE / NARRATE | 渲染对话框后等待用户点击，然后调用 `step()` |
/// | `ShowMenu` | MENU | 渲染选项列表，等待用户选择，将选择结果告知 VM |
/// | `SceneEnd` | END | 场景结束，切换场景或返回标题 |
/// | `Command` | BG / SHOW / PLAY_BGM 等 | 执行对应的渲染/音频操作后继续调用 `step()` |
#[derive(Debug, Clone, PartialEq)]
pub enum VmAction {
    /// 等待用户输入（点击继续）。
    ///
    /// 由 DIALOGUE 或 NARRATE 指令触发。
    /// 调用方应先通过 VM 的相关方法获取对话/旁白详情，
    /// 渲染完成后等待用户点击，然后继续调用 `step()`。
    WaitForInput,

    /// 显示选择支菜单，等待用户选择。
    ///
    /// 由 MENU 指令触发。
    /// `prompt_idx` 是提示文本的常量池索引。
    /// `choices` 包含所有可选项。
    /// 调用方渲染菜单，在用户选择后通过 VM 的接口设置选择结果并继续。
    ShowMenu {
        /// 提示文本的常量池索引
        prompt_idx: u16,
        /// 选项列表
        choices: Vec<MenuChoiceData>,
    },

    /// 场景执行完毕。
    ///
    /// 由 END 指令触发。
    /// 调用方应执行场景结束逻辑（切换场景、返回标题等）。
    SceneEnd,

    /// 需要 SceneManager 处理的引擎命令。
    ///
    /// 由 BG、SHOW_CHAR、PLAY_BGM 等渲染/音频指令触发。
    /// 调用方执行命令后继续调用 `step()` 推进 VM。
    Command(EngineCommand),
}

impl fmt::Display for VmAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmAction::WaitForInput => write!(f, "WaitForInput"),
            VmAction::ShowMenu {
                prompt_idx,
                choices,
            } => {
                write!(
                    f,
                    "ShowMenu(prompt_pool[{}], {} choices)",
                    prompt_idx,
                    choices.len()
                )
            }
            VmAction::SceneEnd => write!(f, "SceneEnd"),
            VmAction::Command(cmd) => write!(f, "Command({})", cmd),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 VmAction 所有变体可构造。
    #[test]
    fn vm_action_all_variants_constructible() {
        let _ = VmAction::WaitForInput;
        let _ = VmAction::ShowMenu {
            prompt_idx: 0,
            choices: vec![MenuChoiceData {
                text_idx: 1,
                target_offset: 10,
                condition_flag_idx: 0xFFFF,
            }],
        };
        let _ = VmAction::SceneEnd;
        let _ = VmAction::Command(EngineCommand::Error {
            message: "test".into(),
        });
    }

    /// 验证 VmAction::ShowMenu 的 choices 内容正确存储。
    #[test]
    fn show_menu_choices_preserved() {
        let choices = vec![
            MenuChoiceData {
                text_idx: 0,
                target_offset: 100,
                condition_flag_idx: 0xFFFF,
            },
            MenuChoiceData {
                text_idx: 1,
                target_offset: 200,
                condition_flag_idx: 5,
            },
        ];

        let action = VmAction::ShowMenu {
            prompt_idx: 42,
            choices: choices.clone(),
        };

        if let VmAction::ShowMenu {
            prompt_idx,
            choices: stored,
        } = &action
        {
            assert_eq!(*prompt_idx, 42);
            assert_eq!(stored.len(), 2);
            assert_eq!(stored[0].text_idx, 0);
            assert_eq!(stored[0].target_offset, 100);
            assert_eq!(stored[1].condition_flag_idx, 5);
        } else {
            panic!("应为 ShowMenu 变体");
        }
    }

    /// 验证 Display 实现不 panic。
    #[test]
    fn vm_action_display_does_not_panic() {
        let actions = [
            VmAction::WaitForInput,
            VmAction::SceneEnd,
            VmAction::Command(EngineCommand::SetBg {
                asset_idx: 0,
                trans_kind_idx: 0xFFFF,
                dur_reg: 0xFF,
            }),
            VmAction::ShowMenu {
                prompt_idx: 0,
                choices: vec![],
            },
        ];

        for action in &actions {
            let s = format!("{}", action);
            assert!(!s.is_empty());
        }
    }
}
