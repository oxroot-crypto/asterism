//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/scene_manager.rs
//! 功能概述：场景管理器 — 管理场景加载→执行→结束生命周期，
//!           协调 VM 和 Renderer 的交互。renderer 以参数形式传入 update/select_choice。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15

use aster_vm::{EngineCommand, MenuChoiceData, Vm, VmAction};

use crate::command_bridge::{self, Renderer};
use crate::error::RuntimeError;
use crate::game_context::GameContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneState {
    Idle,
    Loading,
    Playing,
    AtMenu,
    Paused,
    Transitioning,
    Ended,
}

impl std::fmt::Display for SceneState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Idle => "Idle",
            Self::Loading => "Loading",
            Self::Playing => "Playing",
            Self::AtMenu => "AtMenu",
            Self::Paused => "Paused",
            Self::Transitioning => "Transitioning",
            Self::Ended => "Ended",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
struct MenuState {
    prompt: String,
    choices: Vec<MenuChoiceData>,
}

const MAX_STEPS_PER_UPDATE: usize = 50_000;

/// 场景管理器。
pub struct SceneManager {
    ctx: GameContext,
    vm: Vm,
    state: SceneState,
    current_scene_id: Option<String>,
    current_menu: Option<MenuState>,
    command_log: Vec<EngineCommand>,
    steps_this_update: usize,
}

impl SceneManager {
    pub fn new(ctx: GameContext) -> Self {
        Self {
            ctx,
            vm: Vm::new(),
            state: SceneState::Idle,
            current_scene_id: None,
            current_menu: None,
            command_log: Vec::new(),
            steps_this_update: 0,
        }
    }

    #[inline]
    pub fn state(&self) -> &SceneState {
        &self.state
    }
    #[inline]
    pub fn current_scene_id(&self) -> Option<&str> {
        self.current_scene_id.as_deref()
    }
    #[inline]
    pub fn vm(&self) -> &Vm {
        &self.vm
    }
    #[inline]
    pub fn vm_mut(&mut self) -> &mut Vm {
        &mut self.vm
    }
    #[inline]
    pub fn command_log(&self) -> &[EngineCommand] {
        &self.command_log
    }
    #[inline]
    pub fn menu_choice_count(&self) -> Option<usize> {
        self.current_menu.as_ref().map(|m| m.choices.len())
    }
    #[inline]
    pub fn menu_prompt(&self) -> Option<&str> {
        self.current_menu.as_ref().map(|m| m.prompt.as_str())
    }

    /// 加载场景。
    pub fn load_scene(&mut self, scene_id: &str) -> Result<(), RuntimeError> {
        if !self.ctx.is_scene_loaded(scene_id) {
            return Err(RuntimeError::SceneNotFound {
                scene_id: scene_id.to_string(),
            });
        }
        self.vm.set_pc(0);
        self.current_scene_id = Some(scene_id.to_string());
        self.current_menu = None;
        self.state = SceneState::Playing;
        self.command_log.clear();
        self.steps_this_update = 0;
        Ok(())
    }

    /// 驱动场景执行。传入可选的 renderer 用于派发渲染命令。
    pub fn update(&mut self, mut renderer: Option<&mut dyn Renderer>) -> Result<(), RuntimeError> {
        if self.state != SceneState::Playing {
            return Ok(());
        }
        self.steps_this_update = 0;

        loop {
            self.steps_this_update += 1;
            if self.steps_this_update > MAX_STEPS_PER_UPDATE {
                return Err(RuntimeError::VmError {
                    message: format!(
                        "场景 '{}' 单次 update() 中 VM 执行超过 {} 条指令",
                        self.current_scene_id.as_deref().unwrap_or("?"),
                        MAX_STEPS_PER_UPDATE
                    ),
                });
            }

            let scene_id = self
                .current_scene_id
                .as_deref()
                .ok_or(RuntimeError::SceneNotLoaded)?;
            let scene =
                self.ctx
                    .get_scene(scene_id)
                    .ok_or_else(|| RuntimeError::SceneNotFound {
                        scene_id: scene_id.to_string(),
                    })?;
            let action = self.vm.step(scene);

            let should_pause = self.process_action(action, &mut renderer)?;
            if should_pause {
                break;
            }
        }
        Ok(())
    }

    /// 鼠标点击：打字机进行中→skip；打字机完成→推进对话。
    pub fn on_click(
        &mut self,
        mut renderer: Option<&mut dyn Renderer>,
    ) -> Result<(), RuntimeError> {
        match self.state {
            SceneState::Playing => {
                // 检查打字机状态：进行中则跳过动画，完成则推进对话
                let complete = renderer.as_ref().is_none_or(|r| r.is_typewriter_complete());
                if complete {
                    self.update(renderer)
                } else {
                    if let Some(ref mut r) = renderer {
                        r.skip_typewriter();
                    }
                    Ok(())
                }
            }
            SceneState::Paused => {
                self.state = SceneState::Playing;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// 键盘按键。Enter/Space → 推进。
    pub fn on_key_press(
        &mut self,
        key: &str,
        renderer: Option<&mut dyn Renderer>,
    ) -> Result<(), RuntimeError> {
        match key {
            "Enter" | "Space" => self.on_click(renderer),
            _ => Ok(()),
        }
    }

    /// 选择菜单选项。
    pub fn select_choice(
        &mut self,
        index: usize,
        mut renderer: Option<&mut dyn Renderer>,
    ) -> Result<(), RuntimeError> {
        if self.state != SceneState::AtMenu {
            return Err(RuntimeError::InvalidState {
                expected: "AtMenu".into(),
                actual: self.state.to_string(),
            });
        }
        let menu = self
            .current_menu
            .as_ref()
            .ok_or_else(|| RuntimeError::InvalidState {
                expected: "AtMenu（含数据）".into(),
                actual: "AtMenu（数据缺失）".into(),
            })?;
        if index >= menu.choices.len() {
            return Err(RuntimeError::InvalidChoiceIndex {
                index,
                max: menu.choices.len(),
            });
        }
        let target_offset = menu.choices[index].target_offset as usize;
        self.vm.set_pc(target_offset);
        if let Some(r) = renderer.as_mut() {
            r.clear_menu();
        }
        self.current_menu = None;
        self.state = SceneState::Playing;
        self.steps_this_update = 0;
        self.update(renderer)
    }

    // ─── 内部方法 ──────────────────────────────────────────────────

    fn process_action(
        &mut self,
        action: VmAction,
        renderer: &mut Option<&mut dyn Renderer>,
    ) -> Result<bool, RuntimeError> {
        match action {
            VmAction::WaitForInput => Ok(true),

            VmAction::ShowMenu { prompt, choices } => {
                let scene_id = self
                    .current_scene_id
                    .as_deref()
                    .ok_or(RuntimeError::SceneNotLoaded)?;
                let scene =
                    self.ctx
                        .get_scene(scene_id)
                        .ok_or_else(|| RuntimeError::SceneNotFound {
                            scene_id: scene_id.to_string(),
                        })?;
                let choice_texts: Vec<String> = choices
                    .iter()
                    .map(|c| {
                        if (c.text_idx as usize) < scene.constant_pool.len() {
                            scene.constant_pool[c.text_idx as usize].clone()
                        } else {
                            format!("<无效选项 {}>", c.text_idx)
                        }
                    })
                    .collect();
                if let Some(r) = renderer.as_mut() {
                    r.show_menu(&prompt, &choice_texts);
                }
                self.current_menu = Some(MenuState { prompt, choices });
                self.state = SceneState::AtMenu;
                Ok(true)
            }

            VmAction::SceneEnd => {
                self.state = SceneState::Ended;
                Ok(true)
            }

            VmAction::Command(cmd) => {
                let is_pause = matches!(
                    &cmd,
                    EngineCommand::SetDialogue { .. } | EngineCommand::SetNarration { .. }
                );
                if matches!(&cmd, EngineCommand::Error { .. }) {
                    log::error!("[SceneManager] VM 错误：{}", cmd);
                }

                let goto_target = command_bridge::dispatch(&cmd, &self.ctx, renderer);

                if let Some((scene_id, label)) = goto_target {
                    self.load_scene(&scene_id)?;
                    if !label.is_empty() {
                        let new_id = self
                            .current_scene_id
                            .as_deref()
                            .ok_or(RuntimeError::SceneNotLoaded)?;
                        let new_scene = self.ctx.get_scene(new_id).ok_or_else(|| {
                            RuntimeError::SceneNotFound {
                                scene_id: new_id.to_string(),
                            }
                        })?;
                        if let Some(&offset) = new_scene.label_table.get(&label) {
                            self.vm.set_pc(offset);
                        }
                    }
                    self.command_log.push(cmd);
                    return Ok(false);
                }

                self.command_log.push(cmd);
                Ok(is_pause)
            }
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_bridge::MockRenderer;
    use crate::game_manifest::GameManifest;
    use aster_compiler::{BuildInfo, CompiledGame, CompiledScene, Opcode};
    use aster_core::{Game, GameSettings, Resolution};
    use std::collections::HashMap;

    fn make_ctx(scene_id: &str, scene: CompiledScene) -> GameContext {
        let mut scenes = HashMap::new();
        scenes.insert(scene_id.to_string(), scene);
        GameContext::new(
            GameManifest {
                project: Game {
                    name: "t".into(),
                    version: "0.1".into(),
                    entry_scene: scene_id.into(),
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
                entry_scene_id: scene_id.into(),
                scenes,
                characters: HashMap::new(),
                build_info: BuildInfo {
                    source_file_count: 1,
                    total_instructions: 0,
                    optimization_level: "none".into(),
                    build_timestamp: "".into(),
                },
            },
        )
    }

    fn encode_dialogue(s: u16, t: u16, v: u16) -> Vec<u8> {
        let mut b = vec![Opcode::Dialogue as u8];
        b.extend_from_slice(&s.to_le_bytes());
        b.extend_from_slice(&t.to_le_bytes());
        b.extend_from_slice(&v.to_le_bytes());
        b
    }
    fn encode_menu(p: u16, choices: &[(u16, u16, u16)]) -> Vec<u8> {
        let mut b = vec![Opcode::Menu as u8];
        b.extend_from_slice(&p.to_le_bytes());
        b.push(choices.len() as u8);
        for &(ti, to, cf) in choices {
            b.extend_from_slice(&ti.to_le_bytes());
            b.extend_from_slice(&to.to_le_bytes());
            b.extend_from_slice(&cf.to_le_bytes());
        }
        b
    }

    fn scene_end() -> CompiledScene {
        CompiledScene {
            version: 1,
            instructions: vec![Opcode::End as u8],
            constant_pool: vec![],
            label_table: HashMap::new(),
        }
    }

    #[test]
    fn ac01_empty_scene() {
        let mut m = SceneManager::new(make_ctx("t", scene_end()));
        m.load_scene("t").unwrap();
        m.update(None).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
    }
    #[test]
    fn ac01_push_int() {
        let mut ins = vec![Opcode::PushInt as u8, 0];
        ins.extend_from_slice(&42i64.to_le_bytes());
        ins.push(Opcode::End as u8);
        let s = CompiledScene {
            version: 1,
            instructions: ins,
            constant_pool: vec![],
            label_table: HashMap::new(),
        };
        let mut m = SceneManager::new(make_ctx("t", s));
        m.load_scene("t").unwrap();
        m.update(None).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
        assert_eq!(m.vm().registers()[0], aster_core::Value::Int(42));
    }
    #[test]
    fn ac01_missing_scene() {
        let mut m = SceneManager::new(make_ctx("t", scene_end()));
        assert!(m.load_scene("x").is_err());
    }

    #[test]
    fn ac02_dialogue_pause() {
        let pool = vec!["S".into(), "hi".into()];
        let mut ins = encode_dialogue(0, 1, 0xFFFF);
        ins.push(Opcode::End as u8);
        let s = CompiledScene {
            version: 1,
            instructions: ins,
            constant_pool: pool,
            label_table: HashMap::new(),
        };
        let mut mock = MockRenderer::new();
        let mut m = SceneManager::new(make_ctx("t", s));
        m.load_scene("t").unwrap();
        m.update(Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Playing);
        assert!(mock.has_call_containing("set_dialogue"));
        assert!(
            m.command_log()
                .iter()
                .any(|c| matches!(c, EngineCommand::SetDialogue { .. }))
        );
        m.on_click(Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
    }
    #[test]
    fn ac02_no_scene() {
        let mut m = SceneManager::new(make_ctx("t", scene_end()));
        m.update(None).unwrap();
        assert_eq!(*m.state(), SceneState::Idle);
    }

    #[test]
    fn ac03_menu() {
        let pool = vec!["Q".into(), "A".into(), "B".into()];
        let ch = vec![(1u16, 100u16, 0xFFFFu16), (2u16, 200u16, 0xFFFFu16)];
        let mut ins = encode_menu(0, &ch);
        ins.push(Opcode::End as u8);
        let s = CompiledScene {
            version: 1,
            instructions: ins,
            constant_pool: pool,
            label_table: HashMap::new(),
        };
        let mut mock = MockRenderer::new();
        let mut m = SceneManager::new(make_ctx("t", s));
        m.load_scene("t").unwrap();
        m.update(Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::AtMenu);
        assert_eq!(m.menu_choice_count(), Some(2));
        assert_eq!(m.menu_prompt(), Some("Q"));
        assert!(mock.has_call_containing("show_menu"));
    }

    #[test]
    fn ac04_choice() {
        let pool = vec!["?".into(), "go".into()];
        let ch = vec![(1u16, 10u16, 0xFFFFu16)];
        let mut ins = encode_menu(0, &ch);
        ins.push(Opcode::End as u8);
        let s = CompiledScene {
            version: 1,
            instructions: ins,
            constant_pool: pool,
            label_table: HashMap::new(),
        };
        let mut mock = MockRenderer::new();
        let mut m = SceneManager::new(make_ctx("t", s));
        m.load_scene("t").unwrap();
        m.update(Some(&mut mock)).unwrap();
        m.select_choice(0, Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
    }
    #[test]
    fn ac04_bad_index() {
        let mut mock = MockRenderer::new();
        let mut m = SceneManager::new(make_ctx("t", scene_end()));
        m.load_scene("t").unwrap();
        assert!(m.select_choice(0, Some(&mut mock)).is_err());
    }

    #[test]
    fn ac05_audio() {
        let pool = vec!["b.ogg".into()];
        let mut ins = vec![Opcode::PlayBgm as u8];
        ins.extend_from_slice(&0u16.to_le_bytes());
        ins.push(0xFF);
        ins.push(1);
        ins.push(Opcode::End as u8);
        let s = CompiledScene {
            version: 1,
            instructions: ins,
            constant_pool: pool,
            label_table: HashMap::new(),
        };
        let mut mock = MockRenderer::new();
        let mut m = SceneManager::new(make_ctx("t", s));
        m.load_scene("t").unwrap();
        m.update(Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
        assert!(
            m.command_log()
                .iter()
                .any(|c| matches!(c, EngineCommand::PlayBgm { .. }))
        );
        assert!(!mock.has_call_containing("PlayBgm"));
    }

    #[test]
    fn goto_cross_scene() {
        let mut ins_a = vec![Opcode::Goto as u8];
        ins_a.extend_from_slice(&0u16.to_le_bytes());
        ins_a.extend_from_slice(&0xFFFFu16.to_le_bytes());
        ins_a.push(Opcode::End as u8);
        let sa = CompiledScene {
            version: 1,
            instructions: ins_a,
            constant_pool: vec!["b".into()],
            label_table: HashMap::new(),
        };
        let sb = CompiledScene {
            version: 1,
            instructions: vec![Opcode::End as u8],
            constant_pool: vec![],
            label_table: HashMap::new(),
        };
        let mut scenes = HashMap::new();
        scenes.insert("a".into(), sa);
        scenes.insert("b".into(), sb);
        let compiled = CompiledGame {
            game_name: "t".into(),
            game_version: "0.1".into(),
            entry_scene_id: "a".into(),
            scenes,
            characters: HashMap::new(),
            build_info: BuildInfo {
                source_file_count: 2,
                total_instructions: 0,
                optimization_level: "none".into(),
                build_timestamp: "".into(),
            },
        };
        let manifest = GameManifest {
            project: Game {
                name: "t".into(),
                version: "0.1".into(),
                entry_scene: "a".into(),
                resolution: Resolution::default(),
                settings: GameSettings::default(),
            },
            characters: HashMap::new(),
            scenes: vec![],
            build_config: aster_core::BuildConfig::default(),
        };
        let ctx = GameContext::new(manifest, compiled);
        let mut m = SceneManager::new(ctx);
        m.load_scene("a").unwrap();
        m.update(None).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
        assert_eq!(m.current_scene_id(), Some("b"));
        assert!(
            m.command_log()
                .iter()
                .any(|c| matches!(c, EngineCommand::Goto { .. }))
        );
    }

    #[test]
    fn multi_dialogue() {
        let pool = vec!["S".into(), "1".into(), "2".into()];
        let mut ins = Vec::new();
        ins.extend(encode_dialogue(0, 1, 0xFFFF));
        ins.extend(encode_dialogue(0, 2, 0xFFFF));
        ins.push(Opcode::End as u8);
        let s = CompiledScene {
            version: 1,
            instructions: ins,
            constant_pool: pool,
            label_table: HashMap::new(),
        };
        let mut mock = MockRenderer::new();
        let mut m = SceneManager::new(make_ctx("t", s));
        m.load_scene("t").unwrap();
        m.update(Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Playing);
        m.on_click(Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Playing);
        m.on_click(Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
        assert_eq!(
            m.command_log()
                .iter()
                .filter(|c| matches!(c, EngineCommand::SetDialogue { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn key_enter() {
        let pool = vec!["S".into(), "t".into()];
        let mut ins = encode_dialogue(0, 1, 0xFFFF);
        ins.push(Opcode::End as u8);
        let s = CompiledScene {
            version: 1,
            instructions: ins,
            constant_pool: pool,
            label_table: HashMap::new(),
        };
        let mut mock = MockRenderer::new();
        let mut m = SceneManager::new(make_ctx("t", s));
        m.load_scene("t").unwrap();
        m.update(Some(&mut mock)).unwrap();
        m.on_key_press("Enter", Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
    }
    #[test]
    fn key_ignored() {
        let pool = vec!["S".into(), "t".into()];
        let mut ins = encode_dialogue(0, 1, 0xFFFF);
        ins.push(Opcode::End as u8);
        let s = CompiledScene {
            version: 1,
            instructions: ins,
            constant_pool: pool,
            label_table: HashMap::new(),
        };
        let mut mock = MockRenderer::new();
        let mut m = SceneManager::new(make_ctx("t", s));
        m.load_scene("t").unwrap();
        m.update(Some(&mut mock)).unwrap();
        m.on_key_press("A", Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Playing);
    }
    #[test]
    fn init_state() {
        let m = SceneManager::new(make_ctx("t", scene_end()));
        assert_eq!(*m.state(), SceneState::Idle);
    }
    #[test]
    fn ended_noop() {
        let mut m = SceneManager::new(make_ctx("t", scene_end()));
        m.load_scene("t").unwrap();
        m.update(None).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
        m.update(None).unwrap();
        assert_eq!(*m.state(), SceneState::Ended);
    }
}
