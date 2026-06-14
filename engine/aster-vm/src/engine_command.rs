//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/src/engine_command.rs
//! 功能概述：引擎命令枚举 — 定义 VM 向上层（SceneManager/渲染器/音频系统）
//!           发出的所有渲染和音频操作命令。所有字段使用常量池索引（u16）
//!           或寄存器索引（u8），由上层解析为具体资源路径或值。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - （无外部模块依赖，纯数据结构）

use std::fmt;

/// 引擎命令枚举 — VM 执行渲染/音频指令时发出的操作命令。
///
/// 所有资源标识符（asset_id、char_id、emotion 等）使用常量池索引（u16），
/// 由 SceneManager 结合 `CompiledScene.constant_pool` 解析为具体字符串。
/// 寄存器索引（dur_reg、fade_reg 等）指向 VM 的 `registers` 数组，
/// SceneManager 通过 VM 的只读接口获取实际值。
///
/// # 变体分类
///
/// | 分类 | 变体 | 说明 |
/// |------|------|------|
/// | 渲染 | `SetBg` | 切换背景图片 |
/// | 渲染 | `ShowChar` | 显示角色立绘 |
/// | 渲染 | `HideChar` | 隐藏角色立绘 |
/// | 渲染 | `ShowSprite` | 显示独立精灵 |
/// | 渲染 | `HideSprite` | 隐藏独立精灵 |
/// | 渲染 | `MoveChar` | 移动角色立绘 |
/// | 渲染 | `Emotion` | 切换角色表情 |
/// | 渲染 | `SetDialogue` | 显示对话文本 |
/// | 渲染 | `SetNarration` | 显示旁白文本 |
/// | 媒体 | `PlayBgm` | 播放背景音乐 |
/// | 媒体 | `StopBgm` | 停止背景音乐 |
/// | 媒体 | `PlaySe` | 播放音效 |
/// | 媒体 | `PlayVoice` | 播放语音 |
/// | 时序 | `Wait` | 等待指定时长 |
/// | 特效 | `Effect` | 触发画面特效 |
/// | 错误 | `Error` | VM 运行时错误 |
#[derive(Debug, Clone, PartialEq)]
pub enum EngineCommand {
    /// 切换背景图片
    ///
    /// - `asset_idx`：背景资源路径的常量池索引
    /// - `trans_kind_idx`：转场类型名的常量池索引（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认时长）
    SetBg {
        asset_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示/重新出场角色立绘
    ///
    /// - `char_idx`：角色 ID 的常量池索引
    /// - `pos_byte`：立绘位置编码（0=Left, 1=Center, 2=Right, 3=Custom）
    /// - `x_reg`：Custom 位置的 X 坐标寄存器（`0xFF` = 非 Custom 位置）
    /// - `y_reg`：Custom 位置的 Y 坐标寄存器（`0xFF` = 非 Custom 位置）
    /// - `emotion_idx`：表情名的常量池索引（`0xFFFF` = 默认表情）
    /// - `trans_kind_idx`：入场转场类型（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认）
    ShowChar {
        char_idx: u16,
        pos_byte: u8,
        x_reg: u8,
        y_reg: u8,
        emotion_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示独立精灵图片
    ///
    /// - `asset_idx`：图片资源路径的常量池索引
    /// - `x_reg` / `y_reg`：归一化坐标的寄存器
    /// - `scale_reg` / `alpha_reg`：缩放/透明度的寄存器
    /// - `trans_kind_idx`：入场转场类型（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认）
    ShowSprite {
        asset_idx: u16,
        x_reg: u8,
        y_reg: u8,
        scale_reg: u8,
        alpha_reg: u8,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 平滑移动角色立绘
    ///
    /// - `char_idx`：角色 ID 的常量池索引
    /// - `pos_byte`：目标位置编码
    /// - `x_reg` / `y_reg`：Custom 位置的坐标寄存器
    /// - `emotion_idx`：可选的新表情（`0xFFFF` = 保持现有）
    /// - `trans_kind_idx`：移动动画类型
    /// - `dur_reg`：移动时长的寄存器
    MoveChar {
        char_idx: u16,
        pos_byte: u8,
        x_reg: u8,
        y_reg: u8,
        emotion_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 原地切换角色立绘表情
    ///
    /// - `char_idx`：角色 ID 的常量池索引
    /// - `emotion_idx`：新表情名的常量池索引
    /// - `trans_kind_idx`：切换动画类型（`0xFFFF` = 无动画）
    /// - `dur_reg`：切换时长的寄存器（`0xFF` = 默认）
    Emotion {
        char_idx: u16,
        emotion_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 隐藏角色立绘
    ///
    /// - `char_idx`：角色 ID 的常量池索引
    /// - `trans_kind_idx`：退场转场类型（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认）
    HideChar {
        char_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 隐藏独立精灵
    ///
    /// - `asset_idx`：图片资源路径的常量池索引
    /// - `trans_kind_idx`：退场转场类型（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认）
    HideSprite {
        asset_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示角色对话（含可选语音）
    ///
    /// - `speaker_idx`：说话者名称的常量池索引
    /// - `text_idx`：对话文本的常量池索引
    /// - `voice_idx`：语音文件 ID 的常量池索引（`0xFFFF` = 无语音）
    SetDialogue {
        speaker_idx: u16,
        text_idx: u16,
        voice_idx: u16,
    },

    /// 显示旁白文本（无说话者）
    ///
    /// - `text_idx`：旁白文本的常量池索引
    SetNarration { text_idx: u16 },

    /// 播放/切换背景音乐
    ///
    /// - `asset_idx`：BGM 资源路径的常量池索引
    /// - `fade_reg`：淡入时长的寄存器（`0xFF` = 无淡入/默认）
    /// - `looping`：是否循环播放
    PlayBgm {
        asset_idx: u16,
        fade_reg: u8,
        looping: bool,
    },

    /// 停止背景音乐
    ///
    /// - `fade_reg`：淡出时长的寄存器（`0xFF` = 立即停止）
    StopBgm { fade_reg: u8 },

    /// 播放音效（不阻断 VM 执行）
    ///
    /// - `asset_idx`：音效资源路径的常量池索引
    /// - `fade_reg`：淡入时长的寄存器（`0xFF` = 无淡入）
    PlaySe { asset_idx: u16, fade_reg: u8 },

    /// 播放语音（通常伴随 Dialogue）
    ///
    /// - `asset_idx`：语音资源路径的常量池索引
    PlayVoice { asset_idx: u16 },

    /// 暂停指定时长
    ///
    /// - `dur_reg`：等待时长（毫秒）的寄存器
    Wait { dur_reg: u8 },

    /// 触发画面特效
    ///
    /// - `type_idx`：特效类型标识的常量池索引
    /// - `params`：特效参数键值对（常量池索引, 寄存器引用）
    Effect {
        type_idx: u16,
        params: Vec<(u16, u16)>,
    },

    /// VM 运行时错误
    ///
    /// 用于汇报不可恢复的 VM 执行错误（无效操作码、栈溢出等）。
    /// SceneManager 应终止当前场景并显示错误信息。
    Error { message: String },
}

impl fmt::Display for EngineCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineCommand::SetBg { asset_idx, .. } => {
                write!(f, "SetBg(asset_pool[{}])", asset_idx)
            }
            EngineCommand::ShowChar {
                char_idx,
                pos_byte,
                emotion_idx,
                ..
            } => {
                write!(
                    f,
                    "ShowChar(char_pool[{}], pos={}, emo_pool[{}])",
                    char_idx, pos_byte, emotion_idx
                )
            }
            EngineCommand::ShowSprite { asset_idx, .. } => {
                write!(f, "ShowSprite(asset_pool[{}])", asset_idx)
            }
            EngineCommand::MoveChar {
                char_idx, pos_byte, ..
            } => {
                write!(f, "MoveChar(char_pool[{}], pos={})", char_idx, pos_byte)
            }
            EngineCommand::Emotion {
                char_idx,
                emotion_idx,
                ..
            } => {
                write!(
                    f,
                    "Emotion(char_pool[{}], emo_pool[{}])",
                    char_idx, emotion_idx
                )
            }
            EngineCommand::HideChar { char_idx, .. } => {
                write!(f, "HideChar(char_pool[{}])", char_idx)
            }
            EngineCommand::HideSprite { asset_idx, .. } => {
                write!(f, "HideSprite(asset_pool[{}])", asset_idx)
            }
            EngineCommand::SetDialogue {
                speaker_idx,
                text_idx,
                ..
            } => {
                write!(
                    f,
                    "SetDialogue(speaker_pool[{}], text_pool[{}])",
                    speaker_idx, text_idx
                )
            }
            EngineCommand::SetNarration { text_idx } => {
                write!(f, "SetNarration(text_pool[{}])", text_idx)
            }
            EngineCommand::PlayBgm {
                asset_idx, looping, ..
            } => {
                write!(f, "PlayBgm(asset_pool[{}], loop={})", asset_idx, looping)
            }
            EngineCommand::StopBgm { .. } => {
                write!(f, "StopBgm")
            }
            EngineCommand::PlaySe { asset_idx, .. } => {
                write!(f, "PlaySe(asset_pool[{}])", asset_idx)
            }
            EngineCommand::PlayVoice { asset_idx } => {
                write!(f, "PlayVoice(asset_pool[{}])", asset_idx)
            }
            EngineCommand::Wait { dur_reg } => {
                write!(f, "Wait(r{})", dur_reg)
            }
            EngineCommand::Effect { type_idx, params } => {
                write!(f, "Effect(pool[{}], {} params)", type_idx, params.len())
            }
            EngineCommand::Error { message } => {
                write!(f, "Error({})", message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 EngineCommand 所有变体可构造。
    #[test]
    fn engine_command_all_variants_constructible() {
        let _ = EngineCommand::SetBg {
            asset_idx: 0,
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::ShowChar {
            char_idx: 0,
            pos_byte: 1,
            x_reg: 0xFF,
            y_reg: 0xFF,
            emotion_idx: 1,
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::ShowSprite {
            asset_idx: 0,
            x_reg: 1,
            y_reg: 2,
            scale_reg: 3,
            alpha_reg: 4,
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::MoveChar {
            char_idx: 0,
            pos_byte: 0,
            x_reg: 0xFF,
            y_reg: 0xFF,
            emotion_idx: 0xFFFF,
            trans_kind_idx: 1,
            dur_reg: 2,
        };
        let _ = EngineCommand::Emotion {
            char_idx: 0,
            emotion_idx: 1,
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::HideChar {
            char_idx: 0,
            trans_kind_idx: 1,
            dur_reg: 2,
        };
        let _ = EngineCommand::HideSprite {
            asset_idx: 0,
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::SetDialogue {
            speaker_idx: 0,
            text_idx: 1,
            voice_idx: 0xFFFF,
        };
        let _ = EngineCommand::SetNarration { text_idx: 0 };
        let _ = EngineCommand::PlayBgm {
            asset_idx: 0,
            fade_reg: 0xFF,
            looping: true,
        };
        let _ = EngineCommand::StopBgm { fade_reg: 0xFF };
        let _ = EngineCommand::PlaySe {
            asset_idx: 0,
            fade_reg: 0xFF,
        };
        let _ = EngineCommand::PlayVoice { asset_idx: 0 };
        let _ = EngineCommand::Wait { dur_reg: 0 };
        let _ = EngineCommand::Effect {
            type_idx: 0,
            params: vec![(1, 2)],
        };
        let _ = EngineCommand::Error {
            message: "test error".into(),
        };
    }

    /// 验证 Display 实现不 panic。
    #[test]
    fn engine_command_display_does_not_panic() {
        let commands = [
            EngineCommand::SetBg {
                asset_idx: 0,
                trans_kind_idx: 0xFFFF,
                dur_reg: 0xFF,
            },
            EngineCommand::SetDialogue {
                speaker_idx: 0,
                text_idx: 1,
                voice_idx: 0xFFFF,
            },
            EngineCommand::Error {
                message: "测试错误".into(),
            },
        ];

        for cmd in &commands {
            let s = format!("{}", cmd);
            assert!(!s.is_empty());
        }
    }
}
