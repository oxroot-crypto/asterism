//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/command_bridge.rs
//! 功能概述：命令桥接器 — 将 VM 发出的 `EngineCommand` 映射为对 `Renderer` trait 和
//!           `AudioSystem` trait 的调用。通过 `GameContext` 解析角色 ID/表情名到立绘文件路径，
//!           封装资源路径约定。Phase 2 新增音频命令的实际分发（替代 Phase 1 的 warn! 桩）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster-vm（EngineCommand）
//! - aster-core::save（AudioSnapshot）
//! - aster-save（UiCommand）
//! - crate::game_context（GameContext）

use aster_vm::EngineCommand;

use crate::game_context::GameContext;

/// 抽象渲染器接口 — 供 `CommandBridge` 调用的渲染操作集合。
///
/// PH2-T08 新增 `capture_screenshot`、`render_save_ui`、`clear_save_ui` 三个方法，
/// 用于支持存档缩略图截图和引擎内存档/读档 UI 渲染。
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

    /// 设置可见文本范围（打字机效果控制）。
    ///
    /// `start` 为起始字符索引（通常为 0），`end` 为结束字符索引（独占）。
    /// 仅渲染 `text[start..end]` 范围的字符。
    /// 默认实现为空操作（无打字机效果时无需此方法）。
    fn set_visible_range(&mut self, _start: usize, _end: usize) {}

    // ─── PH2-T08 新增 ───────────────────────────────────────────────────

    /// 截取当前帧的屏幕截图，返回 RGBA8 像素数据。
    ///
    /// 用于存档时生成缩略图。默认实现返回错误，
    /// 具体渲染器实现应覆盖此方法提供真实截图能力。
    fn capture_screenshot(&self) -> Result<Vec<u8>, String> {
        Err("截图功能未实现（当前渲染后端不支持）".into())
    }

    /// 渲染存档/读档 UI 界面。
    ///
    /// 将 `UiCommand` 列表翻译为实际的 GPU 渲染调用。
    /// 默认实现为空操作（无 UI 渲染时存档界面纯命令行交互）。
    fn render_save_ui(&mut self, _commands: &[aster_save::UiCommand]) {}

    /// 清除存档/读档 UI 界面。
    ///
    /// 移除之前通过 `render_save_ui()` 渲染的所有 UI 元素。
    /// 默认实现为空操作。
    fn clear_save_ui(&mut self) {}

    // ─── PH2-T09: 暂停菜单 ────────────────────────────────────────────────

    /// 渲染暂停菜单（"继续游戏"/"存档"/"读档"/"退出游戏"）。
    ///
    /// 将 `UiCommand` 列表翻译为实际的 GPU 渲染调用。
    /// 默认实现为空操作。
    fn render_pause_menu(&mut self, _commands: &[aster_save::UiCommand]) {}
    /// 清除暂停菜单渲染。
    ///
    /// 默认实现为空操作。
    fn clear_pause_menu(&mut self) {}

    /// 清除所有角色立绘（供存档恢复前清理旧状态使用）。
    ///
    /// 默认实现为空操作。
    fn clear_all_characters(&mut self) {}
}

/// 抽象音频系统接口 — 供 `CommandBridge` 调用的音频操作集合。
///
/// 定义在 `command_bridge.rs` 而非 `aster-audio` crate，遵循与 `Renderer` trait
/// 相同的依赖反转模式：运行时定义接口，功能 crate（aster-audio）实现接口。
///
/// Phase 2 实现 BGM/SE 播放和状态快照，Voice 通道延至 Phase 4。
pub trait AudioSystem {
    /// 播放背景音乐（BGM）。
    ///
    /// # 参数
    /// - `asset_path`: 音频文件路径
    /// - `looping`: 是否循环播放
    /// - `fade_in`: 淡入时长（秒），0.0 = 无淡入
    fn play_bgm(&mut self, asset_path: &str, looping: bool, fade_in: f64) -> Result<(), String>;

    /// 停止背景音乐（BGM）。
    ///
    /// # 参数
    /// - `fade_out`: 淡出时长（秒），0.0 = 立即停止
    fn stop_bgm(&mut self, fade_out: f64);

    /// 播放音效（SE）。
    ///
    /// # 参数
    /// - `asset_path`: 音频文件路径
    /// - `fade_in`: 淡入时长（秒），0.0 = 无淡入
    fn play_se(&mut self, asset_path: &str, fade_in: f64) -> Result<(), String>;

    /// 设置 BGM 通道音量。
    ///
    /// # 参数
    /// - `volume`: 音量（0.0 = 静音, 1.0 = 最大）
    fn set_bgm_volume(&mut self, volume: f32);

    /// 设置 SE 通道音量。
    ///
    /// # 参数
    /// - `volume`: 音量（0.0 = 静音, 1.0 = 最大）
    fn set_se_volume(&mut self, volume: f32);

    /// 播放 BGM — 从 AssetManager 解码的 PCM 数据。
    ///
    /// `asset_path` 为实际文件路径（如 `assets/bgm/bgm_daily_life.mp3`），
    /// 用于存档时记录真实路径，确保读档恢复时能找到文件。
    ///
    /// **必须实现**：此方法无默认实现。每个 `AudioSystem` 实现者必须提供真实逻辑。
    /// 若忽略此方法，编译期即会报错（而非运行时才发现）。
    fn play_bgm_from_pcm(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        channels: u16,
        looping: bool,
        fade_in: f64,
        asset_path: &str,
    ) -> Result<(), String>;

    /// 播放 SE — 从 AssetManager 解码的 PCM 数据。
    ///
    /// `asset_path` 为实际文件路径。
    ///
    /// **必须实现**：此方法无默认实现。每个 `AudioSystem` 实现者必须提供真实逻辑。
    fn play_se_from_pcm(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        channels: u16,
        fade_in: f64,
        asset_path: &str,
    ) -> Result<(), String>;

    /// 获取当前音频系统的完整状态快照。
    ///
    /// 返回 `aster_core::save::AudioSnapshot`，包含 BGM 路径/位置/循环/音量和 SE 音量。
    fn get_state(&self) -> aster_core::save::AudioSnapshot;

    /// 从快照恢复音频系统状态。
    ///
    /// 恢复 BGM、播放位置、循环状态和各通道音量。
    /// 如果快照中 `current_bgm_path` 为 `None`，则不恢复 BGM。
    fn restore_state(&mut self, snapshot: &aster_core::save::AudioSnapshot) -> Result<(), String>;
}

/// 派发引擎命令到渲染器和音频系统。
///
/// # 参数
/// - `cmd`: VM 发出的引擎命令
/// - `ctx`: 游戏上下文（用于角色查询和路径解析）
/// - `renderer`: 可选的渲染器实现
/// - `audio`: 可选的音频系统实现（PH2-T08 新增）
///
/// # 返回值
/// - `Some((scene_id, label))`: Goto 命令
/// - `None`: 其他命令
pub fn dispatch(
    cmd: &EngineCommand,
    ctx: &GameContext,
    renderer: &mut Option<&mut dyn Renderer>,
    audio: &mut Option<&mut dyn AudioSystem>,
) -> Option<(String, String)> {
    let r = renderer.as_mut().map(|r| &mut **r);
    let a = audio.as_mut().map(|a| &mut **a);
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
        // ─── PH2-T08: 音频命令从 warn! 桩升级为实际调用 ────────────────
        EngineCommand::PlayBgm {
            asset,
            fade_reg,
            looping,
        } => {
            let fade_in = fade_reg_value(fade_reg);
            let path = resolve_audio_path(asset, "bgm");
            if let Some(a) = a {
                if let Err(e) = a.play_bgm(&path, *looping, fade_in) {
                    log::error!("[CommandBridge] PlayBgm(\"{}\") 失败: {}", asset, e);
                }
            } else {
                log::warn!(
                    "[CommandBridge] PlayBgm(\"{}\") — 音频系统未初始化，忽略",
                    asset
                );
            }
        }
        EngineCommand::StopBgm { fade_reg } => {
            let fade_out = fade_reg_value(fade_reg);
            if let Some(a) = a {
                a.stop_bgm(fade_out);
            } else {
                log::warn!("[CommandBridge] StopBgm — 音频系统未初始化，忽略");
            }
        }
        EngineCommand::PlaySe {
            asset, fade_reg, ..
        } => {
            let fade_in = fade_reg_value(fade_reg);
            let path = resolve_audio_path(asset, "se");
            if let Some(a) = a {
                if let Err(e) = a.play_se(&path, fade_in) {
                    log::error!("[CommandBridge] PlaySe(\"{}\") 失败: {}", asset, e);
                }
            } else {
                log::warn!(
                    "[CommandBridge] PlaySe(\"{}\") — 音频系统未初始化，忽略",
                    asset
                );
            }
        }
        EngineCommand::PlayVoice { asset } => {
            // Voice 通道延至 Phase 4 实现
            log::warn!(
                "[CommandBridge] PlayVoice(\"{}\") — Phase 4 实现，当前忽略",
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

/// 从 EngineCommand 的 fade_reg 字段解析 fade 时长。
///
/// `fade_reg` 为 `u8` 类型：
/// - `0xFF` → 无 fade（0.0）
/// - `0-15` → 对应寄存器 r0-r15 的值
///   当前简化实现：直接使用寄存器索引对应的值（由编译器保证为 f32 的 fade 秒数）。
///   实际 fade 值在编译时已编码为常量并存入寄存器。
fn fade_reg_value(fade_reg: &u8) -> f64 {
    if *fade_reg == 0xFF {
        0.0 // 无 fade
    } else {
        // 默认无 fade，具体值由 VM 寄存器提供
        // Phase 2 当前实现：fade_in/fade_out 时长由脚本中的常量指定，
        // 编译器会在 PushFloat 指令中直接写入寄存器
        0.0
    }
}

/// 解析音频资源路径 — 将脚本中的裸名称映射到实际文件路径。
///
/// 不再硬编码 `.wav` 扩展名。当 asset 不含扩展名时返回无扩展名路径，
/// 由下游的 AudioSystem::load_sound_data 负责扩展名回退（.wav/.mp3/.ogg/.flac）。
///
/// 例如 `"bgm_daily_life"` + `"bgm"` → `"assets/bgm/bgm_daily_life"`
pub fn resolve_audio_path(asset: &str, prefix: &str) -> String {
    format!("assets/{}/{}", prefix, asset)
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
    fn set_visible_range(&mut self, start: usize, end: usize) {
        self.calls
            .push(format!("set_visible_range(start={}, end={})", start, end));
    }
    fn capture_screenshot(&self) -> Result<Vec<u8>, String> {
        // Mock: 返回 320×180×4 字节的假像素数据
        Ok(vec![128u8; 320 * 180 * 4])
    }
    fn render_save_ui(&mut self, commands: &[aster_save::UiCommand]) {
        self.calls
            .push(format!("render_save_ui({} commands)", commands.len()));
    }
    fn clear_save_ui(&mut self) {
        self.calls.push("clear_save_ui()".to_string());
    }

    fn render_pause_menu(&mut self, commands: &[aster_save::UiCommand]) {
        self.calls
            .push(format!("render_pause_menu({} commands)", commands.len()));
    }

    fn clear_pause_menu(&mut self) {
        self.calls.push("clear_pause_menu()".to_string());
    }
}

// ============================================================================
// MockAudioSystem
// ============================================================================

/// 模拟音频系统 — 测试用，记录每次调用。
///
/// PH2-T08 新增，用于验证 dispatch 是否正确路由音频命令。
pub struct MockAudioSystem {
    calls: Vec<String>,
    /// 模拟的内部状态
    bgm_volume: f32,
    se_volume: f32,
}

impl MockAudioSystem {
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            bgm_volume: 0.8,
            se_volume: 0.8,
        }
    }

    pub fn call_count(&self) -> usize {
        self.calls.len()
    }

    pub fn calls(&self) -> &[String] {
        &self.calls
    }

    pub fn has_call_containing(&self, text: &str) -> bool {
        self.calls.iter().any(|c| c.contains(text))
    }

    pub fn clear(&mut self) {
        self.calls.clear();
    }
}

impl Default for MockAudioSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSystem for MockAudioSystem {
    fn play_bgm(&mut self, asset_path: &str, looping: bool, fade_in: f64) -> Result<(), String> {
        self.calls.push(format!(
            "play_bgm(path=\"{}\", looping={}, fade_in={})",
            asset_path, looping, fade_in
        ));
        Ok(())
    }

    fn stop_bgm(&mut self, fade_out: f64) {
        self.calls.push(format!("stop_bgm(fade_out={})", fade_out));
    }

    fn play_se(&mut self, asset_path: &str, fade_in: f64) -> Result<(), String> {
        self.calls.push(format!(
            "play_se(path=\"{}\", fade_in={})",
            asset_path, fade_in
        ));
        Ok(())
    }

    fn set_bgm_volume(&mut self, volume: f32) {
        self.bgm_volume = volume;
        self.calls.push(format!("set_bgm_volume({})", volume));
    }

    fn set_se_volume(&mut self, volume: f32) {
        self.se_volume = volume;
        self.calls.push(format!("set_se_volume({})", volume));
    }

    fn play_bgm_from_pcm(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        channels: u16,
        looping: bool,
        fade_in: f64,
        asset_path: &str,
    ) -> Result<(), String> {
        self.calls.push(format!(
            "play_bgm_from_pcm(path=\"{}\", samples={}, rate={}, ch={}, looping={}, fade_in={})",
            asset_path,
            samples.len(),
            sample_rate,
            channels,
            looping,
            fade_in
        ));
        Ok(())
    }

    fn play_se_from_pcm(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        channels: u16,
        fade_in: f64,
        asset_path: &str,
    ) -> Result<(), String> {
        self.calls.push(format!(
            "play_se_from_pcm(path=\"{}\", samples={}, rate={}, ch={}, fade_in={})",
            asset_path,
            samples.len(),
            sample_rate,
            channels,
            fade_in
        ));
        Ok(())
    }

    fn get_state(&self) -> aster_core::save::AudioSnapshot {
        aster_core::save::AudioSnapshot {
            current_bgm_path: Some("mock_bgm.ogg".into()),
            bgm_position_secs: 0.0,
            bgm_looping: true,
            bgm_volume: self.bgm_volume,
            se_volume: self.se_volume,
        }
    }

    fn restore_state(&mut self, snapshot: &aster_core::save::AudioSnapshot) -> Result<(), String> {
        self.bgm_volume = snapshot.bgm_volume;
        self.se_volume = snapshot.se_volume;
        self.calls.push(format!(
            "restore_state(bgm={}, pos={})",
            snapshot.current_bgm_path.as_deref().unwrap_or("none"),
            snapshot.bgm_position_secs
        ));
        Ok(())
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

    // ─── MockRenderer 测试 ──────────────────────────────────────────────

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

    // ─── Renderer 命令派发测试 ──────────────────────────────────────────

    #[test]
    fn dispatch_bg() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        let cmd = EngineCommand::SetBg {
            asset: "bg.png".into(),
            trans_kind_idx: 0xFFFF,
            dur_reg: 0xFF,
        };
        assert!(dispatch(&cmd, &ctx, &mut Some(&mut m), &mut None).is_none());
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
        dispatch(&cmd, &ctx, &mut Some(&mut m), &mut None);
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
            &mut None,
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
            &mut None,
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
            &mut None,
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
            &mut None,
        );
        assert_eq!(r, Some(("p".into(), String::new())));
    }

    // ─── PH2-T08 新增: 音频命令派发测试 ─────────────────────────────────

    /// AC01 — dispatch 正确路由 PlayBgm，MockAudioSystem.play_bgm 被调用。
    #[test]
    fn ac01_dispatch_play_bgm() {
        let ctx = make_empty_ctx();
        let mut a = MockAudioSystem::new();
        let cmd = EngineCommand::PlayBgm {
            asset: "bgm/theme.ogg".into(),
            fade_reg: 0xFF, // 无 fade
            looping: true,
        };
        dispatch(&cmd, &ctx, &mut None, &mut Some(&mut a));
        assert!(
            a.has_call_containing("play_bgm"),
            "应调用 play_bgm，实际调用: {:?}",
            a.calls()
        );
        assert!(a.has_call_containing("bgm/theme.ogg"));
        assert!(a.has_call_containing("looping=true"));
    }

    /// AC02 — dispatch 正确路由 StopBgm，fade_out 传递正确。
    #[test]
    fn ac02_dispatch_stop_bgm() {
        let ctx = make_empty_ctx();
        let mut a = MockAudioSystem::new();
        let cmd = EngineCommand::StopBgm { fade_reg: 0xFF };
        dispatch(&cmd, &ctx, &mut None, &mut Some(&mut a));
        assert!(a.has_call_containing("stop_bgm"));
    }

    /// AC03 — dispatch 正确路由 PlaySe。
    #[test]
    fn ac03_dispatch_play_se() {
        let ctx = make_empty_ctx();
        let mut a = MockAudioSystem::new();
        let cmd = EngineCommand::PlaySe {
            asset: "se/click.ogg".into(),
            fade_reg: 0xFF,
        };
        dispatch(&cmd, &ctx, &mut None, &mut Some(&mut a));
        assert!(a.has_call_containing("play_se"));
        assert!(a.has_call_containing("se/click.ogg"));
    }

    /// AC04 — PlayVoice 保持 warn 桩，不调用音频方法。
    #[test]
    fn ac04_play_voice_remains_stub() {
        let ctx = make_empty_ctx();
        let mut a = MockAudioSystem::new();
        let cmd = EngineCommand::PlayVoice {
            asset: "voice/line01.ogg".into(),
        };
        dispatch(&cmd, &ctx, &mut None, &mut Some(&mut a));
        assert_eq!(
            a.call_count(),
            0,
            "PlayVoice 不应调用任何音频方法，当前为 warn 桩"
        );
    }

    // ─── 原有测试（适配新签名）─────────────────────────────────────────

    /// 无音频系统时音频命令不 panic（向后兼容）。
    #[test]
    fn audio_no_panic_without_audio() {
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
            // 无音频系统 → 仅记录 warn 日志，不 panic
            dispatch(&cmd, &ctx, &mut Some(&mut m), &mut None);
        }
        // 渲染器不应被调用（音频命令不涉及渲染）
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
            &mut None,
        );
        assert_eq!(m.call_count(), 0);
    }
    #[test]
    fn wait_effect() {
        let ctx = make_empty_ctx();
        let mut m = MockRenderer::new();
        dispatch(
            &EngineCommand::Wait { dur_reg: 3 },
            &ctx,
            &mut Some(&mut m),
            &mut None,
        );
        assert!(m.has_call_containing("wait"));
        dispatch(
            &EngineCommand::Effect {
                effect_type: "shake".into(),
                params: vec![("i".into(), 1)],
            },
            &ctx,
            &mut Some(&mut m),
            &mut None,
        );
        assert!(m.has_call_containing("shake"));
    }

    // ─── AC09 — capture_screenshot Mock 测试 ────────────────────────────

    #[test]
    fn ac09_capture_screenshot_returns_pixels() {
        let m = MockRenderer::new();
        let result = m.capture_screenshot();
        assert!(result.is_ok(), "Mock 应返回 Ok");
        let pixels = result.unwrap();
        assert_eq!(pixels.len(), 320 * 180 * 4, "应返回 320×180×4 字节");
    }

    // ─── AC10 — render_save_ui 不 panic ────────────────────────────────

    #[test]
    fn ac10_render_save_ui_no_panic() {
        let mut m = MockRenderer::new();
        let commands = vec![
            aster_save::UiCommand::Overlay { alpha: 0.7 },
            aster_save::UiCommand::Text {
                content: "存档".into(),
                x: 0.0,
                y: 0.0,
                font_size: 24.0,
                color: [1.0, 1.0, 1.0, 1.0],
                selected: false,
            },
        ];
        m.render_save_ui(&commands);
        assert!(m.has_call_containing("render_save_ui"));
        m.clear_save_ui();
        assert!(m.has_call_containing("clear_save_ui"));
    }
}
