//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/scene_manager.rs
//! 功能概述：场景管理器 — 管理场景加载→执行→结束生命周期，
//!           协调 VM 和 Renderer 的交互。renderer 以参数形式传入 update/select_choice。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster_core（TextSpeed 速度类型）
//! - aster_renderer（TypewriterSpeed 打字机速度）
//! - aster_vm（Vm / VmAction / EngineCommand / MenuChoiceData）
//! - crate::command_bridge（Renderer trait + dispatch）
//! - crate::dialogue_controller（DialogueController / DialogueAction / DialogueLine）
//! - crate::game_context::GameContext
//! - crate::error::RuntimeError
//!
//! 对应任务：PH1-T18 — 实现 SceneManager + Renderer trait 的真实实现

use aster_renderer::TypewriterSpeed;
use aster_vm::{EngineCommand, MenuChoiceData, Vm, VmAction};

use crate::command_bridge::{self, Renderer};
use crate::dialogue_controller::{DialogueAction, DialogueController, DialogueLine};
use crate::error::RuntimeError;
use crate::game_context::GameContext;

/// 将配置中的文字速度转换为打字机运行时速度。
fn to_typewriter_speed(speed: &aster_core::TextSpeed) -> TypewriterSpeed {
    match speed {
        aster_core::TextSpeed::Instant => TypewriterSpeed::Instant,
        aster_core::TextSpeed::Slow => TypewriterSpeed::Slow,
        aster_core::TextSpeed::Normal => TypewriterSpeed::Normal,
        aster_core::TextSpeed::Fast => TypewriterSpeed::Fast,
        aster_core::TextSpeed::Custom(ms) => TypewriterSpeed::Custom(*ms),
    }
}

/// 场景状态枚举 — 表示场景管理器的当前生命周期阶段。
///
/// SceneManager 通过此枚举跟踪场景执行的各个阶段，
/// 控制 `update()` / `on_click()` / `select_choice()` 等方法的合法调用时机。
///
/// # 状态流转
///
/// ```text
/// Idle → Loading → Playing ⇄ AtMenu
///                   Playing → Paused → Playing
///                   Playing → Transitioning → Playing
///                   Playing → Ended
/// ```
///
/// # 变体说明
///
/// | 变体 | 说明 |
/// |------|------|
/// | `Idle` | 初始状态，尚未加载任何场景 |
/// | `Loading` | 正在加载场景资源（Phase 2 起使用） |
/// | `Playing` | 正在执行场景指令（包括等待用户点击） |
/// | `AtMenu` | 正在显示选择支菜单，等待玩家选择 |
/// | `Paused` | 游戏暂停（用户打开设置面板等） |
/// | `Transitioning` | 正在执行场景转场动画 |
/// | `Ended` | 当前场景已执行完毕 |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneState {
    /// 初始状态 — 尚未加载任何场景
    Idle,
    /// 正在加载场景资源（Phase 2 起使用）
    Loading,
    /// 正在执行场景指令
    Playing,
    /// 正在显示选择支菜单
    AtMenu,
    /// 游戏已暂停
    Paused,
    /// 正在执行场景转场动画
    Transitioning,
    /// 当前场景已结束
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

/// 场景管理器 — 协调 VM 执行与 Renderer 渲染的核心控制器。
///
/// 负责场景生命周期的完整管理：
/// - 场景加载/卸载（`load_scene`）
/// - 驱动 VM 逐指令执行（`update`）
/// - 处理用户输入（`on_click` / `on_key_press` / `select_choice`）
/// - 对话流管理（通过 `DialogueController` 控制打字机动画和点击推进）
/// - 跨场景跳转（Goto 指令处理）
///
/// # 字段说明
/// - `ctx`：游戏上下文（持有编译后的场景字节码 + 角色表）
/// - `vm`：字节码虚拟机（执行 CompiledScene 指令）
/// - `state`：当前场景状态（Playing / AtMenu / Ended 等）
/// - `current_scene_id`：当前活动的场景 ID
/// - `current_menu`：当前显示的选择支菜单状态（仅在 AtMenu 状态下为 Some）
/// - `command_log`：已执行的渲染/音频命令历史（用于调试和重放）
/// - `steps_this_update`：当前 update() 调用中已执行的 VM 步数（防无限循环）
/// - `dialogue_controller`：对话流管理器（打字机动画 + 文本缓冲队列）
pub struct SceneManager {
    ctx: GameContext,
    vm: Vm,
    state: SceneState,
    current_scene_id: Option<String>,
    current_menu: Option<MenuState>,
    command_log: Vec<EngineCommand>,
    steps_this_update: usize,
    dialogue_controller: DialogueController,
}

impl SceneManager {
    pub fn new(ctx: GameContext) -> Self {
        // 从项目配置中读取默认文字显示速度
        let text_speed = to_typewriter_speed(&ctx.default_text_speed);
        Self {
            ctx,
            vm: Vm::new(),
            state: SceneState::Idle,
            current_scene_id: None,
            current_menu: None,
            command_log: Vec::new(),
            steps_this_update: 0,
            dialogue_controller: DialogueController::new(text_speed),
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
        self.dialogue_controller.reset();
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

    /// 鼠标点击：委托给 DialogueController。
    /// - 打字机进行中 → 跳过动画
    /// - 打字机完成 → 推进 VM 到下一句
    pub fn on_click(
        &mut self,
        mut renderer: Option<&mut dyn Renderer>,
    ) -> Result<(), RuntimeError> {
        match self.state {
            SceneState::Playing => {
                match self.dialogue_controller.on_click() {
                    DialogueAction::Advance => {
                        // 对话已完成且队列为空，推进 VM 以产生下一句
                        self.update(renderer)
                    }
                    DialogueAction::None => {
                        // 打字机被跳过（Typewriting→WaitingForAdvance）
                        // 或队列中有下一句已自动开始显示
                        // 同步可见范围到渲染器
                        self.sync_dialogue_to_renderer(&mut renderer);
                        Ok(())
                    }
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

    /// 每帧更新打字机动画并同步可见范围到渲染器。
    ///
    /// 在事件循环中每帧调用一次。推进 DialogueController 内部的 Typewriter，
    /// 然后将当前可见字符数同步到渲染器（`set_visible_range`）。
    ///
    /// # 参数
    /// - `delta`: 自上一帧以来的时间增量
    /// - `renderer`: 渲染器实现（用于 `set_visible_range` 调用）
    pub fn update_dialogue(
        &mut self,
        delta: std::time::Duration,
        renderer: &mut Option<&mut dyn Renderer>,
    ) {
        if self.state != SceneState::Playing {
            return;
        }
        self.dialogue_controller.update(delta);
        self.sync_dialogue_to_renderer(renderer);
    }

    /// 获取对 DialogueController 的不可变引用。
    #[inline]
    pub fn dialogue_controller(&self) -> &DialogueController {
        &self.dialogue_controller
    }

    // ─── 内部方法 ──────────────────────────────────────────────────

    /// 将 DialogueController 的当前可见字符数同步到渲染器。
    fn sync_dialogue_to_renderer(&self, renderer: &mut Option<&mut dyn Renderer>) {
        if let Some(r) = renderer.as_mut() {
            r.set_visible_range(0, self.dialogue_controller.current_visible_chars());
        }
    }

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

                // 将对话命令推入 DialogueController（在 cmd 被 move 之前提取文本）
                // command_bridge::dispatch 已调用 renderer.set_dialogue() 设置完整文本，
                // DialogueController 负责管理打字机动画和点击推进逻辑
                match &cmd {
                    EngineCommand::SetDialogue { speaker, text, .. } => {
                        self.dialogue_controller.push(DialogueLine {
                            speaker: speaker.clone(),
                            text: text.clone(),
                            voice_id: None,
                        });
                    }
                    EngineCommand::SetNarration { text } => {
                        self.dialogue_controller.push(DialogueLine {
                            speaker: String::new(),
                            text: text.clone(),
                            voice_id: None,
                        });
                    }
                    _ => {}
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
        // 推进打字机到完成（DialogueController 管理 Typewriter）
        m.update_dialogue(std::time::Duration::from_secs(1), &mut Some(&mut mock));
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
        // 第一句：推进打字机完成 → 点击推进到第二句
        m.update_dialogue(std::time::Duration::from_secs(1), &mut Some(&mut mock));
        m.on_click(Some(&mut mock)).unwrap();
        assert_eq!(*m.state(), SceneState::Playing);
        // 第二句：推进打字机完成 → 点击推进到场景结束
        m.update_dialogue(std::time::Duration::from_secs(1), &mut Some(&mut mock));
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
        // 推进打字机到完成，然后 Enter 键推进
        m.update_dialogue(std::time::Duration::from_secs(1), &mut Some(&mut mock));
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
        // A 键不推进，但需要有推进过打字机（否则会进入 skip 逻辑）
        m.update_dialogue(std::time::Duration::from_secs(1), &mut Some(&mut mock));
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
