//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/src/engine_command.rs
//! 功能概述：引擎命令枚举 — 定义 VM 向上层（SceneManager/渲染器/音频系统）
//!           发出的所有渲染和音频操作命令。
//!           文本/名称/路径字段为已解析的 `String`（VM 内部已从常量池或寄存器取值），
//!           寄存器索引字段（`dur_reg`、`fade_reg` 等）由上层通过 VM 接口获取实际值。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - （无外部模块依赖，纯数据结构）

use std::fmt;

/// 引擎命令枚举 — VM 执行渲染/音频指令时发出的操作命令。
///
/// 所有文本/名称/路径字段已由 VM 在指令执行时解析为 `String`
///（支持常量池索引和寄存器两种来源，通过 `REG_MARKER` 位区分）。
/// 寄存器索引字段（`dur_reg`、`fade_reg` 等）指向 VM 的 `registers` 数组，
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
/// | 跳转 | `Goto` | 跨场景跳转 |
/// | 错误 | `Error` | VM 运行时错误 |
#[derive(Debug, Clone, PartialEq)]
pub enum EngineCommand {
    /// 切换背景图片
    ///
    /// - `asset`：已解析的背景资源路径
    /// - `trans_kind_idx`：转场类型名的常量池索引（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认时长）
    SetBg {
        asset: String,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示/重新出场角色立绘
    ///
    /// - `char`：已解析的角色 ID
    /// - `pos_byte`：立绘位置编码（0=Left, 1=Center, 2=Right, 3=Custom）
    /// - `x_reg` / `y_reg`：Custom 位置的坐标寄存器（`0xFF` = 非 Custom）
    /// - `emotion`：已解析的表情名（空字符串 = 默认表情）
    /// - `trans_kind_idx`：入场转场类型（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认）
    ShowChar {
        char: String,
        pos_byte: u8,
        x_reg: u8,
        y_reg: u8,
        emotion: String,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示独立精灵图片
    ///
    /// - `asset`：已解析的图片资源路径
    /// - `x_reg` / `y_reg`：归一化坐标的寄存器
    /// - `scale_reg` / `alpha_reg`：缩放/透明度的寄存器
    /// - `trans_kind_idx`：入场转场类型（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认）
    ShowSprite {
        asset: String,
        x_reg: u8,
        y_reg: u8,
        scale_reg: u8,
        alpha_reg: u8,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 平滑移动角色立绘
    ///
    /// - `char`：已解析的角色 ID
    /// - `pos_byte`：目标位置编码
    /// - `x_reg` / `y_reg`：Custom 位置的坐标寄存器
    /// - `emotion`：已解析的新表情（空字符串 = 保持现有）
    /// - `trans_kind_idx`：移动动画类型
    /// - `dur_reg`：移动时长的寄存器
    MoveChar {
        char: String,
        pos_byte: u8,
        x_reg: u8,
        y_reg: u8,
        emotion: String,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 原地切换角色立绘表情
    ///
    /// - `char`：已解析的角色 ID
    /// - `emotion`：已解析的新表情名
    /// - `trans_kind_idx`：切换动画类型（`0xFFFF` = 无动画）
    /// - `dur_reg`：切换时长的寄存器（`0xFF` = 默认）
    Emotion {
        char: String,
        emotion: String,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 隐藏角色立绘
    ///
    /// - `char`：已解析的角色 ID
    /// - `trans_kind_idx`：退场转场类型（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认）
    HideChar {
        char: String,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 隐藏独立精灵
    ///
    /// - `asset`：已解析的图片资源路径
    /// - `trans_kind_idx`：退场转场类型（`0xFFFF` = 无转场）
    /// - `dur_reg`：转场持续时长的寄存器（`0xFF` = 默认）
    HideSprite {
        asset: String,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示角色对话（含可选语音）
    ///
    /// - `speaker`：已解析的说话者名称
    /// - `text`：已解析的对话文本
    /// - `voice`：已解析的语音文件路径（空字符串 = 无语音）
    SetDialogue {
        speaker: String,
        text: String,
        voice: String,
    },

    /// 显示旁白文本（无说话者）
    ///
    /// - `text`：已解析的旁白文本
    SetNarration { text: String },

    /// 播放/切换背景音乐
    ///
    /// - `asset`：已解析的 BGM 资源路径
    /// - `fade_reg`：淡入时长的寄存器（`0xFF` = 无淡入/默认）
    /// - `looping`：是否循环播放
    PlayBgm {
        asset: String,
        fade_reg: u8,
        looping: bool,
    },

    /// 停止背景音乐
    ///
    /// - `fade_reg`：淡出时长的寄存器（`0xFF` = 立即停止）
    StopBgm { fade_reg: u8 },

    /// 播放音效（不阻断 VM 执行）
    ///
    /// - `asset`：已解析的音效资源路径
    /// - `fade_reg`：淡入时长的寄存器（`0xFF` = 无淡入）
    PlaySe { asset: String, fade_reg: u8 },

    /// 播放语音（通常伴随 Dialogue）
    ///
    /// - `asset`：已解析的语音资源路径
    PlayVoice { asset: String },

    /// 暂停指定时长
    ///
    /// - `dur_reg`：等待时长（毫秒）的寄存器
    Wait { dur_reg: u8 },

    /// 触发画面特效
    ///
    /// - `effect_type`：已解析的特效类型标识
    /// - `params`：特效参数（已解析的键, 寄存器引用）
    Effect {
        effect_type: String,
        params: Vec<(String, u16)>,
    },

    /// 跨场景跳转
    ///
    /// - `scene`：已解析的目标场景 ID
    /// - `label`：已解析的目标标签名（空字符串 = 场景入口）
    Goto { scene: String, label: String },

    /// VM 运行时错误
    ///
    /// 用于汇报不可恢复的 VM 执行错误（无效操作码、栈溢出等）。
    /// SceneManager 应终止当前场景并显示错误信息。
    Error { message: String },
}

impl fmt::Display for EngineCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineCommand::SetBg { asset, .. } => {
                write!(f, "SetBg(\"{}\")", asset)
            }
            EngineCommand::ShowChar {
                char,
                pos_byte,
                emotion,
                ..
            } => {
                write!(
                    f,
                    "ShowChar(\"{}\", pos={}, emo=\"{}\")",
                    char, pos_byte, emotion
                )
            }
            EngineCommand::ShowSprite { asset, .. } => {
                write!(f, "ShowSprite(\"{}\")", asset)
            }
            EngineCommand::MoveChar { char, pos_byte, .. } => {
                write!(f, "MoveChar(\"{}\", pos={})", char, pos_byte)
            }
            EngineCommand::Emotion { char, emotion, .. } => {
                write!(f, "Emotion(\"{}\", emo=\"{}\")", char, emotion)
            }
            EngineCommand::HideChar { char, .. } => {
                write!(f, "HideChar(\"{}\")", char)
            }
            EngineCommand::HideSprite { asset, .. } => {
                write!(f, "HideSprite(\"{}\")", asset)
            }
            EngineCommand::SetDialogue { speaker, text, .. } => {
                write!(f, "SetDialogue(\"{}\" → \"{}\")", speaker, text)
            }
            EngineCommand::SetNarration { text } => {
                let display = if text.len() > 60 {
                    format!("{}...", &text[..57])
                } else {
                    text.clone()
                };
                write!(f, "SetNarration(\"{}\")", display)
            }
            EngineCommand::PlayBgm { asset, looping, .. } => {
                write!(f, "PlayBgm(\"{}\", loop={})", asset, looping)
            }
            EngineCommand::StopBgm { .. } => {
                write!(f, "StopBgm")
            }
            EngineCommand::PlaySe { asset, .. } => {
                write!(f, "PlaySe(\"{}\")", asset)
            }
            EngineCommand::PlayVoice { asset } => {
                write!(f, "PlayVoice(\"{}\")", asset)
            }
            EngineCommand::Wait { dur_reg } => {
                write!(f, "Wait(r{})", dur_reg)
            }
            EngineCommand::Effect {
                effect_type,
                params,
            } => {
                write!(f, "Effect(\"{}\", {} params)", effect_type, params.len())
            }
            EngineCommand::Goto { scene, label } => {
                write!(f, "Goto(scene=\"{}\", label=\"{}\")", scene, label)
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
            asset: "bg.png".into(),
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::ShowChar {
            char: "sayori".into(),
            pos_byte: 1,
            x_reg: 0xFF,
            y_reg: 0xFF,
            emotion: "smile".into(),
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::ShowSprite {
            asset: "icon.png".into(),
            x_reg: 1,
            y_reg: 2,
            scale_reg: 3,
            alpha_reg: 4,
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::MoveChar {
            char: "akane".into(),
            pos_byte: 0,
            x_reg: 0xFF,
            y_reg: 0xFF,
            emotion: String::new(),
            trans_kind_idx: 1,
            dur_reg: 2,
        };
        let _ = EngineCommand::Emotion {
            char: "sayori".into(),
            emotion: "happy".into(),
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::HideChar {
            char: "sayori".into(),
            trans_kind_idx: 1,
            dur_reg: 2,
        };
        let _ = EngineCommand::HideSprite {
            asset: "icon.png".into(),
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        let _ = EngineCommand::SetDialogue {
            speaker: "小百合".into(),
            text: "你好！".into(),
            voice: String::new(),
        };
        let _ = EngineCommand::SetNarration {
            text: "旁白文本".into(),
        };
        let _ = EngineCommand::PlayBgm {
            asset: "bgm.ogg".into(),
            fade_reg: 0xFF,
            looping: true,
        };
        let _ = EngineCommand::StopBgm { fade_reg: 0xFF };
        let _ = EngineCommand::PlaySe {
            asset: "se.ogg".into(),
            fade_reg: 0xFF,
        };
        let _ = EngineCommand::PlayVoice {
            asset: "voice.ogg".into(),
        };
        let _ = EngineCommand::Wait { dur_reg: 0 };
        let _ = EngineCommand::Effect {
            effect_type: "shake".into(),
            params: vec![("intensity".into(), 2)],
        };
        let _ = EngineCommand::Goto {
            scene: "scene_b".into(),
            label: String::new(),
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
                asset: "bg.png".into(),
                trans_kind_idx: 0xFFFF,
                dur_reg: 0xFF,
            },
            EngineCommand::SetDialogue {
                speaker: "sayori".into(),
                text: "你好".into(),
                voice: String::new(),
            },
            EngineCommand::Goto {
                scene: "scene_b".into(),
                label: String::new(),
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
