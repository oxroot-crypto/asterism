//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/scene_manager.rs
//! 功能概述：场景管理器 — 管理场景加载→执行→结束生命周期，
//!           协调 VM 和 Renderer 的交互。renderer 以参数形式传入 update/select_choice。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core（TextSpeed / SaveData / VmSnapshot / AudioSnapshot / RenderState 等存档类型）
//! - aster_save（SaveManager / SaveUi / SaveUiMode / UiAction / SaveUiResult / SaveSlotInfo）
//! - aster_asset（AssetManager）
//! - aster_vm（Vm / VmAction / EngineCommand / MenuChoiceData）
//! - crate::command_bridge（Renderer trait + AudioSystem trait + dispatch）
//! - crate::dialogue_controller（DialogueController / DialogueAction / DialogueLine）
//! - crate::game_context::GameContext
//! - crate::error::RuntimeError
//!
//! 对应任务：PH1-T18 — 实现 SceneManager + Renderer trait 的真实实现；
//!          PH2-T08 — 运行时集成（音频/资源/存档子系统接入）

use std::sync::{Arc, Mutex};

use aster_core::save::SaveData;
use aster_renderer::TypewriterSpeed;
use aster_save::{SaveManager, SaveUi, SaveUiMode, SaveUiResult, UiAction};
use aster_vm::{EngineCommand, MenuChoiceData, Vm, VmAction};

use crate::command_bridge::{self, AudioSystem, Renderer, resolve_audio_path};
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
    // PH2-T08 新增: 子系统引用和存档状态
    /// 音频系统（可选 — 通过 trait object 持有）
    audio_system: Option<Box<dyn AudioSystem>>,
    /// 资源管理器（可选 — Arc<Mutex> 用于跨子系统共享）
    asset_manager: Option<Arc<Mutex<aster_asset::AssetManager>>>,
    /// 存档管理器（通过 Arc 共享，SaveUi 也需要访问）
    save_manager: Option<Arc<SaveManager>>,
    /// 存档/读档 UI 状态机
    save_ui: SaveUi,
    /// 当前渲染状态（手动跟踪，用于存档时构造 RenderState）
    render_state: aster_core::save::RenderState,
    /// PH2-T08: 存档用的 PC（暂停时的指令位置，比 VM 当前 PC 早一条或多条指令）
    /// 恢复时从此 PC 重放，让 VM 重新发出所有渲染命令
    save_pc: usize,
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
            // PH2-T08: 子系统默认未初始化，由 App 注入
            audio_system: None,
            asset_manager: None,
            save_manager: None,
            save_ui: SaveUi::new(),
            render_state: aster_core::save::RenderState::default(),
            save_pc: 0,
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

    // ─── PH2-T08: 子系统注入 ──────────────────────────────────────────

    /// 注入音频系统。
    pub fn set_audio_system(&mut self, audio: Box<dyn AudioSystem>) {
        self.audio_system = Some(audio);
    }

    /// 注入资源管理器。
    pub fn set_asset_manager(&mut self, asset: Arc<Mutex<aster_asset::AssetManager>>) {
        self.asset_manager = Some(asset);
    }

    /// 注入存档管理器。
    pub fn set_save_manager(&mut self, save: Arc<SaveManager>) {
        self.save_manager = Some(save);
    }

    /// 返回存档 UI 的不可变引用（供 App 判断是否打开）。
    #[inline]
    pub fn save_ui(&self) -> &SaveUi {
        &self.save_ui
    }

    /// 通过 AssetManager 加载音频资源，返回 (samples, sample_rate, channels)。
    ///
    /// AssetManager 不可用或资源未找到时返回 None。
    fn load_audio_through_asset_manager(&self, path: &str) -> Option<(Vec<f32>, u32, u16)> {
        let mgr = self.asset_manager.as_ref()?;
        // Mutex poison 恢复：若其他线程 panic 导致锁中毒，仍可安全取回内部数据。
        // AssetManager 不变量由单线程（事件循环）保证，中毒不会破坏数据一致性。
        let mut mgr = mgr.lock().unwrap_or_else(|poison| poison.into_inner());
        let base_path = std::path::Path::new(path);

        // 尝试在 AssetManager 中查找音频资源。
        // resolve_audio_path 固定追加 .wav，但实际文件可能是 .mp3/.ogg/.flac。
        // 因此按优先级尝试多种扩展名，适配创作者可能使用的任意格式。
        let audio_extensions = ["wav", "mp3", "ogg", "flac"];
        let mut asset_id = mgr.find_by_path(base_path);

        if asset_id.is_none() {
            // 原路径没找到，尝试替换扩展名
            let stem = base_path.with_extension(""); // 去掉扩展名
            for ext in &audio_extensions {
                let try_path = stem.with_extension(ext);
                if let Some(id) = mgr.find_by_path(&try_path) {
                    asset_id = Some(id);
                    break;
                }
            }
        }

        let asset_id = asset_id?;
        let cached = match mgr.load(asset_id) {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "[SceneManager] AssetManager load({}) 失败: {}",
                    asset_id.0,
                    e
                );
                return None;
            }
        };
        if let aster_asset::LoadedAsset::AudioData {
            ref samples,
            sample_rate,
            channels,
        } = cached.data
        {
            Some((samples.clone(), sample_rate, channels))
        } else {
            None
        }
    }

    // ─── PH2-T08: 存档/读档菜单 ───────────────────────────────────────

    /// 打���存档菜单。
    ///
    /// 从 `save_manager` 获取槽位列表，构建 `SlotDisplayInfo` 并打开 SaveUi。
    pub fn enter_save_menu(&mut self) -> Result<(), RuntimeError> {
        let save_mgr = self
            .save_manager
            .as_ref()
            .ok_or_else(|| RuntimeError::SaveError("存档管理器未初始化".into()))?;
        let slot_infos = save_mgr
            .list_saves()
            .map_err(|e| RuntimeError::SaveError(format!("获取存档列表失败: {}", e)))?;
        self.save_ui.open(SaveUiMode::Save, &slot_infos);
        self.state = SceneState::Paused;
        Ok(())
    }

    /// 打开读档菜单。
    pub fn enter_load_menu(&mut self) -> Result<(), RuntimeError> {
        let save_mgr = self
            .save_manager
            .as_ref()
            .ok_or_else(|| RuntimeError::SaveError("存档管理器未初始化".into()))?;
        let slot_infos = save_mgr
            .list_saves()
            .map_err(|e| RuntimeError::SaveError(format!("获取存档列表失败: {}", e)))?;
        self.save_ui.open(SaveUiMode::Load, &slot_infos);
        self.state = SceneState::Paused;
        Ok(())
    }

    /// 关闭存档/读档 UI。
    pub fn close_save_ui(&mut self) {
        self.save_ui.close();
        if self.state == SceneState::Paused {
            self.state = SceneState::Playing;
        }
    }

    /// 设置存档/读档 UI 为错误提示状态。
    ///
    /// 在错误状态下，SaveUi 渲染红色错误文本 + "按任意键返回"提示。
    /// 用户按任意键后 UI 返回槽位列表（而非退出整个 UI）。
    ///
    /// # 参数
    /// - `message`：向用户展示的错误描述（中文）
    /// - `mode`：当前操作模式（存档/读档），用于从错误状态返回列表
    pub fn set_save_ui_error(&mut self, message: &str, mode: SaveUiMode) {
        // 从 save_manager 获取最新的槽位列表，确保返回列表时数据是最新的
        let slots = self
            .save_manager
            .as_ref()
            .and_then(|mgr| mgr.list_saves().ok())
            .map(|infos| SaveUi::build_slot_list(&infos))
            .unwrap_or_default();
        self.save_ui.set_error(message.to_string(), mode, slots);
    }

    /// 检查存档/读档 UI 是否可见。
    #[inline]
    pub fn is_save_ui_open(&self) -> bool {
        self.save_ui.is_open()
    }

    /// 处理存档/读档 UI 中的用户输入。
    ///
    /// 驱动 SaveUi 状态机，根据返回的 `SaveUiResult` 执行实际的
    /// 存档/读档/删除操作。
    ///
    /// # 参数
    /// - `action`: 用户输入（Up/Down/Confirm/Cancel/Delete）
    ///
    /// # 返回值
    /// - `Ok(true)`: 执行了读档（App 需要处理状态恢复和 UI 关闭）
    /// - `Ok(false)`: 执行了存档/删除/取消（UI 可能仍然打开）
    /// - `Err`: 操作失败
    pub fn handle_save_ui_input(
        &mut self,
        action: UiAction,
        mode: SaveUiMode,
        renderer: &mut Option<&mut dyn Renderer>,
    ) -> Result<bool, RuntimeError> {
        let result = self.save_ui.handle_input(action);
        match result {
            SaveUiResult::None => {}
            SaveUiResult::SaveRequested { slot } => {
                // 收集游戏状态 → 保存
                let save_data = self.collect_game_state(slot);
                let save_mgr = self
                    .save_manager
                    .as_ref()
                    .ok_or_else(|| RuntimeError::SaveError("存档管理器未初始化".into()))?;
                save_mgr
                    .save(slot, &save_data)
                    .map_err(|e| RuntimeError::SaveError(format!("存档失败: {}", e)))?;
                // 保存后刷新槽位列表（新建/覆盖后槽位状态改变）
                self.refresh_save_ui_slots(mode)?;
            }
            SaveUiResult::LoadRequested { slot } => {
                let save_mgr = self
                    .save_manager
                    .as_ref()
                    .ok_or_else(|| RuntimeError::SaveError("存档管理器未初始化".into()))?;
                let save_data = save_mgr
                    .load(slot)
                    .map_err(|e| RuntimeError::SaveError(format!("读档失败: {}", e)))?;
                self.restore_game_state(&save_data, renderer)?;
                self.close_save_ui();
                return Ok(true); // 读档成功
            }
            SaveUiResult::DeleteRequested { slot } => {
                let save_mgr = self
                    .save_manager
                    .as_ref()
                    .ok_or_else(|| RuntimeError::SaveError("存档管理器未初始化".into()))?;
                save_mgr
                    .delete_save(slot)
                    .map_err(|e| RuntimeError::SaveError(format!("删除存档失败: {}", e)))?;
                // 删除后刷新槽位列表
                self.refresh_save_ui_slots(mode)?;
            }
            SaveUiResult::Closed => {
                self.close_save_ui();
            }
        }
        Ok(false)
    }

    /// 从 save_manager 重新获取槽位列表并刷新 SaveUi。
    ///
    /// SaveRequested / DeleteRequested 操作成功后调用此方法，
    /// 确保 UI 展示的槽位信息是最新的（消除各操作分支中的重复刷新逻辑）。
    fn refresh_save_ui_slots(&mut self, mode: SaveUiMode) -> Result<(), RuntimeError> {
        let save_mgr = self
            .save_manager
            .as_ref()
            .ok_or_else(|| RuntimeError::SaveError("存档管理器未初始化".into()))?;
        let slot_infos = save_mgr
            .list_saves()
            .map_err(|e| RuntimeError::SaveError(format!("获取存档列表失败: {}", e)))?;
        self.save_ui.open(mode, &slot_infos);
        Ok(())
    }

    // ─── PH2-T08: 游戏状态收集与恢复 ──────────────────────────────────

    /// 收集当前游戏完整状态，构造 `SaveData`。
    ///
    /// 从 VM、AudioSystem、RenderState 收集所有运行时状态。
    ///
    /// # 参数
    /// - `slot`: 目标槽位编号
    pub fn collect_game_state(&self, slot: u8) -> SaveData {
        let scene_id = self.current_scene_id.clone().unwrap_or_else(|| {
            // 防御性日志：正常流程中 collect_game_state 不应在场景加载前被调用。
            // 若通过快速存档（F5）等异步路径在异常时机触发，记录警告以便排查。
            log::warn!("[SceneManager] collect_game_state: current_scene_id 为 None，存档 scene_id 将为空字符串");
            String::new()
        });

        let mut save_data = SaveData::new(slot, &scene_id);
        // 使用 save_pc 而非 vm.pc() — save_pc 指向暂停前的指令，
        // 恢复时 VM 从此 PC 重放，会重新发出 SetBg/ShowChar/SetDialogue 等渲染命令
        let mut vm_snap = self.vm.to_snapshot();
        vm_snap.pc = self.save_pc;
        save_data.vm_snapshot = vm_snap;
        save_data.variables = self.vm.variables().clone();
        save_data.flags = self.vm.flags().clone();
        save_data.render_state = self.render_state.clone();

        // 音频状态 — 从 AudioSystem 获取
        if let Some(ref audio) = self.audio_system {
            save_data.audio_state = audio.get_state();
        }

        save_data
    }

    /// 从 `SaveData` 恢复游戏状态。
    ///
    /// 恢复顺序（关键 — 先恢复无副作用的子系统，再修改 VM/场景/渲染状态）：
    /// 1. 先恢复音频状态（失败则整体回滚，不修改 VM/场景/渲染）
    /// 2. 加载场景（`load_scene` 会重置 VM PC 到 0）
    /// 3. 恢复 VM 状态（覆盖 PC/寄存器/栈）
    /// 4. 恢复渲染状态并应用到 renderer（重绘背景和立绘）
    ///
    /// 步骤 1 提前的设计理由：音频恢复可能因文件缺失等原因失败，
    /// 若放在 VM/场景/渲染恢复之后，失败时游戏已处于半恢复状态
    /// （VM 指向存档场景但 BGM 无声 / 错误日志提示恢复失败）。
    ///
    /// # 参数
    /// - `data`: 之前保存的存档数据
    /// - `renderer`: 渲染器（用于恢复画面）
    pub fn restore_game_state(
        &mut self,
        save_data: &SaveData,
        renderer: &mut Option<&mut dyn Renderer>,
    ) -> Result<(), RuntimeError> {
        // 步骤 1：恢复音频状态（提前执行 — 失败时 VM/场景/渲染均未修改）
        // 优先通过 AssetManager 加载 BGM（统一资源管理，享受 LRU 缓存），
        // AssetManager 不可用时回退到 AudioSystem 的直接文件加载。
        // 跳过 "pcm://" 合成路径（旧版本存档兼容）。
        let bgm_path = save_data.audio_state.current_bgm_path.clone();
        let bgm_looping = save_data.audio_state.bgm_looping;
        let bgm_volume = save_data.audio_state.bgm_volume;
        let se_volume = save_data.audio_state.se_volume;

        // 尝试通过 AssetManager 加载（&self 借用，在此语句结束后释放）
        let pcm_data: Option<(Vec<f32>, u32, u16)> = bgm_path
            .as_deref()
            .filter(|p| !p.starts_with("pcm://"))
            .and_then(|path| self.load_audio_through_asset_manager(path));

        // 应用音频状态（&mut self 借用）
        if let Some(ref mut audio) = self.audio_system {
            audio.stop_bgm(0.0);
            audio.set_se_volume(se_volume);

            if let Some((samples, sample_rate, channels)) = pcm_data {
                // AssetManager 路径：播放已解码的 PCM
                let path = bgm_path.as_deref().unwrap_or("bgm");
                if let Err(e) =
                    audio.play_bgm_from_pcm(&samples, sample_rate, channels, bgm_looping, 0.0, path)
                {
                    log::error!("[SceneManager] 存档恢复 BGM PCM 失败 ({}): {}", path, e);
                }
            } else if bgm_path.is_some() {
                // AssetManager 不可用（未初始化或加载失败），回退到直接文件加载。
                // 注意：bgm_path 已过滤 pcm:// 合成路径，此处为真实文件路径。
                audio
                    .restore_state(&save_data.audio_state)
                    .map_err(|e| RuntimeError::SaveError(format!("音频状态恢复失败: {}", e)))?;
            }
            // pcm:// 合成路径 → 跳过 BGM 恢复，仅恢复音量

            audio.set_bgm_volume(bgm_volume);
        }

        // 步骤 2：加载场景（这会重置 VM 状态）
        if !save_data.scene_id.is_empty() {
            self.load_scene(&save_data.scene_id)?;
        }

        // 步骤 3：恢复 VM 执行状态（覆盖 load_scene 的默认值）
        self.vm.restore_from_snapshot(&save_data.vm_snapshot);
        *self.vm.variables_mut() = save_data.variables.clone();
        *self.vm.flags_mut() = save_data.flags.clone();

        // 步骤 4：恢复渲染状态到内部记录
        self.render_state = save_data.render_state.clone();

        // 步骤 5：将渲染状态应用到 renderer（重绘背景和立绘+对话）
        if let Some(r) = renderer.as_mut() {
            // 先清除所有旧立绘（防止恢复前的残留精灵干扰存档画面）
            r.clear_all_characters();
            // 应用背景
            if let Some(ref bg_path) = self.render_state.current_bg {
                r.set_background(bg_path);
            }
            // 应用立绘：从 char_id + emotion 解析实际文件路径，
            // 以 char_id 作为唯一 key 避免多角色互相覆盖。
            for sprite in &self.render_state.displayed_sprites {
                let effective_emotion = sprite.emotion.as_deref().unwrap_or("default");
                let resolved_path = self
                    .ctx
                    .resolve_sprite_path(&sprite.sprite_path, effective_emotion)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !resolved_path.is_empty() {
                    r.show_character(&sprite.sprite_path, &resolved_path, sprite.position);
                }
            }
        }

        Ok(())
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
            // 记录 step 前的 PC（用于存档时回退到渲染命令之前）
            let pre_step_pc = self.vm.pc();
            let action = self.vm.step(scene);

            let should_pause = self.process_action(action, &mut renderer)?;
            if should_pause {
                self.save_pc = pre_step_pc;
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

                // ── PH2-T08: render_state 追踪 ────────────────────────
                // 在执行渲染命令前更新内部渲染状态记录（用于存档时捕获画面）
                match &cmd {
                    EngineCommand::SetBg { asset, .. } => {
                        let bg_path = format!("assets/bg/{}.png", asset);
                        self.render_state.current_bg = Some(bg_path);
                    }
                    EngineCommand::ShowChar {
                        char,
                        emotion,
                        pos_byte,
                        ..
                    } => {
                        // 移除同一角色的旧立绘
                        self.render_state
                            .displayed_sprites
                            .retain(|s| s.sprite_path != *char);
                        // 添加新立绘
                        self.render_state
                            .displayed_sprites
                            .push(aster_core::save::SpriteState {
                                sprite_path: char.clone(),
                                position: *pos_byte,
                                alpha: 1.0,
                                emotion: if emotion.is_empty() {
                                    None
                                } else {
                                    Some(emotion.clone())
                                },
                            });
                    }
                    EngineCommand::HideChar { char, .. } => {
                        self.render_state
                            .displayed_sprites
                            .retain(|s| s.sprite_path != *char);
                    }
                    EngineCommand::MoveChar {
                        char: char_id,
                        pos_byte,
                        ..
                    } => {
                        // 更新角色位置（PH2-T09 修复：MoveChar 之前未被追踪）
                        for sprite in &mut self.render_state.displayed_sprites {
                            if sprite.sprite_path == *char_id {
                                sprite.position = *pos_byte;
                            }
                        }
                    }
                    EngineCommand::Emotion { char, emotion, .. } => {
                        for sprite in &mut self.render_state.displayed_sprites {
                            if sprite.sprite_path == *char {
                                sprite.emotion = if emotion.is_empty() {
                                    None
                                } else {
                                    Some(emotion.clone())
                                };
                            }
                        }
                    }
                    _ => {}
                }

                // PH2-T08/PH2-T09: 音频命令始终优先走 AssetManager（PCM 路径）。
                // AssetManager 支持多扩展名回退（.wav/.mp3/.ogg/.flac），
                // 解决了 resolve_audio_path 硬编码 .wav 导致的"资源不存在"误报。
                // AssetManager 不可用时 fallback 到 dispatch 的文件路径。
                //
                // 注意：先将 PCM 数据提取到局部变量（&self 借用），
                // 再借用 &mut self.audio_system，避免借用冲突。
                match &cmd {
                    EngineCommand::PlayBgm {
                        asset,
                        fade_reg: _,
                        looping,
                    } => {
                        let path = resolve_audio_path(asset, "bgm");
                        // 步骤 1：通过 AssetManager 加载（&self 借用在此结束）
                        let pcm_data = self.load_audio_through_asset_manager(&path);
                        // 步骤 2：播放 PCM 数据（&mut self 借用）
                        if let Some((samples, sample_rate, channels)) = pcm_data
                            && let Some(audio) = &mut self.audio_system
                        {
                            if let Err(e) = audio.play_bgm_from_pcm(
                                &samples,
                                sample_rate,
                                channels,
                                *looping,
                                0.0,
                                &path,
                            ) {
                                log::error!("[SceneManager] BGM PCM 播放失败 ({}): {}", path, e);
                            } else {
                                log::info!("[SceneManager] BGM PCM 播放成功: {}", path);
                            }
                            self.command_log.push(cmd);
                            return Ok(is_pause);
                        }
                    }
                    EngineCommand::PlaySe {
                        asset, fade_reg: _, ..
                    } => {
                        let path = resolve_audio_path(asset, "se");
                        let pcm_data = self.load_audio_through_asset_manager(&path);
                        if let Some((samples, sample_rate, channels)) = pcm_data
                            && let Some(audio) = &mut self.audio_system
                        {
                            if let Err(e) =
                                audio.play_se_from_pcm(&samples, sample_rate, channels, 0.0, &path)
                            {
                                log::error!("[SceneManager] SE PCM 播放失败 ({}): {}", path, e);
                            }
                            self.command_log.push(cmd);
                            return Ok(is_pause);
                        }
                    }
                    _ => {}
                }

                // PH2-T08: dispatch 剩余命令（含 fallback 音频命令）
                let goto_target = {
                    let mut audio_opt: Option<&mut dyn AudioSystem> = match &mut self.audio_system {
                        Some(boxed) => Some(boxed.as_mut()),
                        None => None,
                    };
                    command_bridge::dispatch(&cmd, &self.ctx, renderer, &mut audio_opt)
                };

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
