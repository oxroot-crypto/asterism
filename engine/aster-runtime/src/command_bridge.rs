//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/command_bridge.rs
//! 功能概述：命令桥接器 — 将 VM 发出的 `EngineCommand` 映射为对 `Renderer` trait 的调用。
//!           通过 `GameContext` 解析角色 ID/表情名到立绘文件路径，
//!           封装资源路径约定。Phase 1 不支持的命令（音频等）记录 `warn!` 日志并忽略。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15

use aster_vm::EngineCommand;

use crate::game_context::GameContext;

/// 抽象渲染器接口 — 供 `CommandBridge` 调用的渲染操作集合。
pub trait Renderer {
    fn set_background(&mut self, path: &str);
    fn show_character(&mut self, char_id: &str, sprite_path: &str, position: u8);
    fn hide_character(&mut self, char_id: &str);
    fn move_character(&mut self, char_id: &str, position: u8);
    fn set_emotion(&mut self, char_id: &str, sprite_path: &str);
    fn show_sprite(&mut self, path: &str, x: f32, y: f32, scale: f32, alpha: f32);
    fn hide_sprite(&mut self, path: &str);
    fn set_dialogue(&mut self, speaker: &str, text: &str);
    fn set_narration(&mut self, text: &str);
    fn show_menu(&mut self, prompt: &str, choice_texts: &[String]);
    fn clear_menu(&mut self);
    fn wait(&mut self, duration_ms: u64);
    fn effect(&mut self, effect_type: &str, params: &[(String, u16)]);

    /// 打字机动画是否已完成（true=文本全部显示）。
    /// SceneManager 用此判断 on_click 应该 skip 还是 advance。
    fn is_typewriter_complete(&self) -> bool {
        true
    }

    /// 跳过打字机动画，立即显示全部文本。
    fn skip_typewriter(&mut self) {}
}

/// 派发引擎命令到渲染器。
///
/// # 参数
/// - `cmd`: VM 发出的引擎命令
/// - `ctx`: 游戏上下文（用于角色查询和路径解析）
/// - `renderer`: 可选的渲染器实现
///
/// # 返回值
/// - `Some((scene_id, label))`: Goto 命令
/// - `None`: 其他命令
pub fn dispatch(
    cmd: &EngineCommand,
    ctx: &GameContext,
    renderer: &mut Option<&mut dyn Renderer>,
) -> Option<(String, String)> {
    let r = renderer.as_mut().map(|r| &mut **r);
    match cmd {
        EngineCommand::SetBg { asset, .. } => {
            if let Some(r) = r {
                // 背景路径约定: assets/bg/{name}.png
                let bg_path = format!("assets/bg/{}.png", asset);
                r.set_background(&bg_path);
            }
        }
        EngineCommand::ShowChar {
            char: char_id,
            pos_byte,
            emotion,
            ..
        } => {
            let sprite_path = resolve_emotion_path(ctx, char_id, emotion);
            if let Some(r) = r {
                r.show_character(char_id, &sprite_path, *pos_byte);
            }
        }
        EngineCommand::HideChar { char: char_id, .. } => {
            if let Some(r) = r {
                r.hide_character(char_id);
            }
        }
        EngineCommand::MoveChar {
            char: char_id,
            pos_byte,
            ..
        } => {
            if let Some(r) = r {
                r.move_character(char_id, *pos_byte);
            }
        }
        EngineCommand::Emotion {
            char: char_id,
            emotion,
            ..
        } => {
            let sprite_path = resolve_emotion_path(ctx, char_id, emotion);
            if let Some(r) = r {
                r.set_emotion(char_id, &sprite_path);
            }
        }
        EngineCommand::ShowSprite { asset, .. } => {
            if let Some(r) = r {
                r.show_sprite(asset, 0.5, 0.5, 1.0, 1.0);
            }
        }
        EngineCommand::HideSprite { asset, .. } => {
            if let Some(r) = r {
                r.hide_sprite(asset);
            }
        }
        EngineCommand::SetDialogue { speaker, text, .. } => {
            if let Some(r) = r {
                r.set_dialogue(speaker, text);
            }
        }
        EngineCommand::SetNarration { text } => {
            if let Some(r) = r {
                r.set_narration(text);
            }
        }
        EngineCommand::PlayBgm { asset, .. } => {
            log::warn!(
                "[CommandBridge] PlayBgm(\"{}\") — Phase 1 不支持音频，忽略",
                asset
            );
        }
        EngineCommand::StopBgm { .. } => {
            log::warn!("[CommandBridge] StopBgm — Phase 1 不支持音频，忽略");
        }
        EngineCommand::PlaySe { asset, .. } => {
            log::warn!(
                "[CommandBridge] PlaySe(\"{}\") — Phase 1 不支持音频，忽略",
                asset
            );
        }
        EngineCommand::PlayVoice { asset } => {
            log::warn!(
                "[CommandBridge] PlayVoice(\"{}\") — Phase 1 不支持音频，忽略",
                asset
            );
        }
        EngineCommand::Wait { .. } => {
            if let Some(r) = r {
                r.wait(0);
            }
        }
        EngineCommand::Effect {
            effect_type,
            params,
        } => {
            if let Some(r) = r {
                r.effect(effect_type, params);
            }
        }
        EngineCommand::Goto { scene, label } => {
            return Some((scene.clone(), label.clone()));
        }
        EngineCommand::Error { message } => {
            log::error!("[CommandBridge] VM 运行时错误：{}", message);
        }
    }
    None
}

fn resolve_emotion_path(ctx: &GameContext, char_id: &str, emotion: &str) -> String {
    let effective = if emotion.is_empty() {
        "default"
    } else {
        emotion
    };
    ctx.resolve_sprite_path(char_id, effective)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            log::warn!(
                "[CommandBridge] 角色 '{}' 的表情 '{}' 无对应立绘资源",
                char_id,
                effective
            );
            String::new()
        })
}

// ============================================================================
// MockRenderer
// ============================================================================

pub struct MockRenderer {
    calls: Vec<String>,
}

impl MockRenderer {
    pub fn new() -> Self {
        Self { calls: Vec::new() }
    }
    pub fn call_count(&self) -> usize {
        self.calls.len()
    }
    pub fn last_call(&self) -> &str {
        self.calls.last().map(|s| s.as_str()).unwrap_or("")
    }
    pub fn calls(&self) -> &[String] {
        &self.calls
    }
    pub fn clear(&mut self) {
        self.calls.clear();
    }
    pub fn has_call_containing(&self, text: &str) -> bool {
        self.calls.iter().any(|c| c.contains(text))
    }
}

impl Default for MockRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for MockRenderer {
    fn set_background(&mut self, path: &str) {
        self.calls.push(format!("set_background(\"{}\")", path));
    }
    fn show_character(&mut self, c: &str, p: &str, pos: u8) {
        self.calls
            .push(format!("show_character(\"{}\", \"{}\", pos={})", c, p, pos));
    }
    fn hide_character(&mut self, c: &str) {
        self.calls.push(format!("hide_character(\"{}\")", c));
    }
    fn move_character(&mut self, c: &str, pos: u8) {
        self.calls
            .push(format!("move_character(\"{}\", pos={})", c, pos));
    }
    fn set_emotion(&mut self, c: &str, p: &str) {
        self.calls
            .push(format!("set_emotion(\"{}\", \"{}\")", c, p));
    }
    fn show_sprite(&mut self, p: &str, x: f32, y: f32, s: f32, a: f32) {
        self.calls.push(format!(
            "show_sprite(\"{}\", x={}, y={}, s={}, a={})",
            p, x, y, s, a
        ));
    }
    fn hide_sprite(&mut self, p: &str) {
        self.calls.push(format!("hide_sprite(\"{}\")", p));
    }
    fn set_dialogue(&mut self, s: &str, t: &str) {
        self.calls
            .push(format!("set_dialogue(speaker=\"{}\", text=\"{}\")", s, t));
    }
    fn set_narration(&mut self, t: &str) {
        self.calls.push(format!("set_narration(text=\"{}\")", t));
    }
    fn show_menu(&mut self, p: &str, choices: &[String]) {
        self.calls.push(format!(
            "show_menu(prompt=\"{}\", choices={:?})",
            p, choices
        ));
    }
    fn clear_menu(&mut self) {
        self.calls.push("clear_menu()".to_string());
    }
    fn wait(&mut self, d: u64) {
        self.calls.push(format!("wait({}ms)", d));
    }
    fn effect(&mut self, t: &str, p: &[(String, u16)]) {
        self.calls
            .push(format!("effect(type=\"{}\", params={:?})", t, p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_manifest::GameManifest;
    use aster_compiler::{BuildInfo, CompiledGame};
    use aster_core::{Game, GameSettings, Resolution};
    use std::collections::HashMap;

    fn make_empty_ctx() -> GameContext {
        GameContext::new(
            GameManifest {
                project: Game {
                    name: "t".into(),
                    version: "0.1".into(),
                    entry_scene: "p".into(),
                    resolution: Resolution::default(),
                    settings: GameSettings::default(),
                },
                characters: HashMap::new(),
                scenes: vec![],
                build_config: aster_core::BuildConfig::default(),
            },
            CompiledGame {
                game_name: "t".into(),
                game_version: "0.1".into(),
                entry_scene_id: "p".into(),
                scenes: HashMap::new(),
                characters: HashMap::new(),
                build_info: BuildInfo {
                    source_file_count: 0,
                    total_instructions: 0,
                    optimization_level: "none".into(),
                    build_timestamp: "".into(),
                },
            },
        )
    }

    #[test]
    fn mock_records_calls() {
        let mut m = MockRenderer::new();
        m.set_background("b");
        m.set_dialogue("s", "t");
        assert_eq!(m.call_count(), 2);
    }
    #[test]
    fn mock_clear() {
        let mut m = MockRenderer::new();
        m.set_background("b");
        m.clear();
        assert_eq!(m.call_count(), 0);
    }

    #[test]
    fn dispatch_bg() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        let cmd = EngineCommand::SetBg {
            asset: "bg.png".into(),
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        assert!(dispatch(&cmd, &ctx, &mut Some(&mut m)).is_none());
        assert!(m.has_call_containing("bg.png"));
    }
    #[test]
    fn dispatch_dialogue() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        let cmd = EngineCommand::SetDialogue {
            speaker: "S".into(),
            text: "hi".into(),
            voice: String::new(),
        };
        dispatch(&cmd, &ctx, &mut Some(&mut m));
        assert!(m.has_call_containing("speaker=\"S\""));
    }
    #[test]
    fn dispatch_narration() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        dispatch(
            &EngineCommand::SetNarration { text: "春".into() },
            &ctx,
            &mut Some(&mut m),
        );
        assert!(m.has_call_containing("set_narration"));
    }
    #[test]
    fn dispatch_hide() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        dispatch(
            &EngineCommand::HideChar {
                char: "s".into(),
                trans_kind_idx: 0,
                dur_reg: 0,
            },
            &ctx,
            &mut Some(&mut m),
        );
        assert!(m.has_call_containing("hide_character"));
    }
    #[test]
    fn dispatch_goto() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        let r = dispatch(
            &EngineCommand::Goto {
                scene: "sc".into(),
                label: "lb".into(),
            },
            &ctx,
            &mut Some(&mut m),
        );
        assert_eq!(r, Some(("sc".into(), "lb".into())));
    }
    #[test]
    fn dispatch_goto_empty_label() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        let r = dispatch(
            &EngineCommand::Goto {
                scene: "p".into(),
                label: String::new(),
            },
            &ctx,
            &mut Some(&mut m),
        );
        assert_eq!(r, Some(("p".into(), String::new())));
    }
    #[test]
    fn audio_no_panic() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        for cmd in [
            EngineCommand::PlayBgm {
                asset: "b.ogg".into(),
                fade_reg: 0,
                looping: true,
            },
            EngineCommand::StopBgm { fade_reg: 0 },
            EngineCommand::PlaySe {
                asset: "s.ogg".into(),
                fade_reg: 0,
            },
            EngineCommand::PlayVoice {
                asset: "v.ogg".into(),
            },
        ] {
            dispatch(&cmd, &ctx, &mut Some(&mut m));
        }
        assert_eq!(m.call_count(), 0);
    }
    #[test]
    fn without_renderer() {
        let ctx = make_empty_ctx();
        dispatch(
            &EngineCommand::SetBg {
                asset: "b.png".into(),
                trans_kind_idx: 0,
                dur_reg: 0,
            },
            &ctx,
            &mut None,
        );
    }
    #[test]
    fn error_no_panic() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        dispatch(
            &EngineCommand::Error {
                message: "e".into(),
            },
            &ctx,
            &mut Some(&mut m),
        );
        assert_eq!(m.call_count(), 0);
    }
    #[test]
    fn wait_effect() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        dispatch(&EngineCommand::Wait { dur_reg: 3 }, &ctx, &mut Some(&mut m));
        assert!(m.has_call_containing("wait"));
        dispatch(
            &EngineCommand::Effect {
                effect_type: "shake".into(),
                params: vec![("i".into(), 1)],
            },
            &ctx,
            &mut Some(&mut m),
        );
        assert!(m.has_call_containing("shake"));
    }
}
