//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/game_context.rs
//! 功能概述：游戏上下文 — 持有编译后的全部场景字节码、角色定义表、项目配置，
//!           提供跨场景导航、角色查询、资源路径解析等核心能力。
//!           SceneManager（PH1-T18）通过 GameContext 获取场景和角色信息。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster_core（Game / Character / AssetId / TextSpeed 等核心类型）
//! - aster_compiler（CompiledGame / CompiledScene — 已编译场景字节码）
//! - crate::game_manifest（GameManifest — 游戏清单，角色表来源）
//! - std::collections::HashMap（场景和角色查询）
//! - std::path::PathBuf（资源路径解析）
//!
//! 对应文档：Phase-1-Tasks.md PH1-T17
//! 架构位置：aster-runtime — 依赖 aster-core + aster-compiler，被 SceneManager（PH1-T18）消费

use std::collections::HashMap;
use std::path::PathBuf;

use aster_compiler::{CompiledGame, CompiledScene};
use aster_core::{AssetId, Character, Game, TextSpeed};

use crate::game_manifest::GameManifest;

/// 游戏上下文 — SceneManager 和渲染器的共享项目状态容器。
///
/// 聚合了编译后的场景字节码、角色定义表和项目配置，
/// 提供跨场景导航、角色查询和资源路径解析的核心能力。
///
/// # 数据来源
///
/// | 字段 | 来源 | 说明 |
/// |------|------|------|
/// | `project` | `GameManifest.project` | 项目元数据（aster.toml） |
/// | `characters` | `GameManifest.characters` | 角色定义表（.asterchar 文件） |
/// | `scenes` | `CompiledGame.scenes` | 已编译场景字节码（GameCompiler 产出） |
/// | `entry_scene_id` | `CompiledGame.entry_scene_id` | 入口场景标识符 |
/// | `resolution` | `Game.settings.resolution`（便捷提取） | 设计分辨率 |
/// | `default_text_speed` | `Game.settings.text_speed`（便捷提取） | 默认文字速度 |
/// | `default_bgm_volume` | `Game.settings`（便捷提取） | 默认 BGM 音量 |
/// | `default_se_volume` | `Game.settings`（便捷提取） | 默认音效音量 |
/// | `default_voice_volume` | `Game.settings`（便捷提取） | 默认语音音量 |
///
/// # 生命周期
///
/// ```text
/// GameLoader::load() → GameManifest ─┐
///                                      ├→ GameContext::new() → SceneManager（PH1-T18）
/// GameCompiler::compile() → CompiledGame ┘
/// ```
///
/// # 示例
///
/// ```no_run
/// use aster_runtime::{GameContext, GameLoader};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // 加载游戏清单（包含角色定义、项目配置）
/// let manifest = GameLoader::load(std::path::Path::new("templates/default_project/"))?;
///
/// // 编译所有场景（由 GameCompiler 完成，此处略）
/// // let compiled = ...;
///
/// // 构建游戏上下文（实际使用中 compiled 由 GameCompiler::compile() 产出）
/// // let ctx = GameContext::new(manifest, compiled);
/// // ctx.get_scene("prologue") ...
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GameContext {
    /// 项目元数据 — 来自 `aster.toml` 的 `[game]` section
    pub project: Game,

    /// 角色定义表 — key 为角色 ID（如 `"sayori"`、`"akane"`），
    /// value 为对应的 `Character` 定义（来自 `characters/*.asterchar`）
    pub characters: HashMap<String, Character>,

    /// 已编译场景集合 — key 为场景 ID（如 `"prologue"`、`"chapter2/opening"`），
    /// value 为 `CompiledScene`（包含字节码、常量池、标签表）
    pub scenes: HashMap<String, CompiledScene>,

    /// 入口场景 ID — 引擎启动时首先加载并执行的场景
    pub entry_scene_id: String,

    /// 设计分辨率（逻辑像素）— 引擎布局和资源缩放的基准
    pub resolution: (u32, u32),

    /// 默认文字显示速度 — 打字机效果的逐字推进速率
    pub default_text_speed: TextSpeed,

    /// 默认 BGM 音量（0.0 ~ 1.0）
    pub default_bgm_volume: f32,

    /// 默认音效音量（0.0 ~ 1.0）
    pub default_se_volume: f32,

    /// 默认语音音量（0.0 ~ 1.0）
    pub default_voice_volume: f32,
}

impl GameContext {
    /// 从游戏清单和编译产物构建游戏上下文。
    ///
    /// 合并 `GameManifest`（项目配置 + 角色表）和 `CompiledGame`（已编译场景），
    /// 提取便捷字段，验证入口场景一致性。
    ///
    /// # 参数
    ///
    /// - `manifest`：`GameLoader::load()` 的产出物，包含项目配置和角色定义
    /// - `compiled`：`GameCompiler::compile()` 的产出物，包含所有已编译场景
    ///
    /// # 验证
    ///
    /// 构造时验证 `manifest.project.entry_scene` 是否存在于 `compiled.scenes` 中。
    /// 如果入口场景缺失，仅记录 `warn!` 日志，不 panic（运行时 SceneManager 会
    /// 在加载入口场景时返回错误）。
    ///
    /// # 示例
    ///
    /// ```
    /// use aster_runtime::{GameContext, GameManifest};
    /// use aster_compiler::CompiledGame;
    /// use aster_core::Game;
    /// use std::collections::HashMap;
    ///
    /// let manifest = GameManifest {
    ///     project: Game {
    ///         name: "测试游戏".into(),
    ///         version: "0.1.0".into(),
    ///         entry_scene: "prologue".into(),
    ///         resolution: aster_core::Resolution { width: 1280, height: 720 },
    ///         settings: aster_core::GameSettings::default(),
    ///     },
    ///     characters: HashMap::new(),
    ///     scenes: vec![],
    ///     build_config: aster_core::BuildConfig::default(),
    /// };
    ///
    /// let compiled = CompiledGame {
    ///     game_name: "测试游戏".into(),
    ///     game_version: "0.1.0".into(),
    ///     entry_scene_id: "prologue".into(),
    ///     scenes: HashMap::new(),
    ///     characters: HashMap::new(),
    ///     build_info: aster_compiler::BuildInfo {
    ///         source_file_count: 0,
    ///         total_instructions: 0,
    ///         optimization_level: "optimized".into(),
    ///         build_timestamp: "2026-06-15T00:00:00Z".into(),
    ///     },
    /// };
    ///
    /// let ctx = GameContext::new(manifest, compiled);
    /// assert_eq!(ctx.entry_scene_id, "prologue");
    /// assert_eq!(ctx.resolution, (1280, 720));
    /// ```
    pub fn new(manifest: GameManifest, compiled: CompiledGame) -> Self {
        // 验证入口场景是否存在于已编译场景集合中
        if !compiled.scenes.contains_key(&manifest.project.entry_scene) {
            eprintln!(
                "[aster-runtime] 警告：入口场景 '{}' 不在已编译场景集合中（scenes 包含 {} 个场景：{:?}），后续加载将失败",
                manifest.project.entry_scene,
                compiled.scenes.len(),
                compiled.scenes.keys().collect::<Vec<_>>()
            );
        }

        // 从项目配置中提前提取便捷字段（必须在移动 manifest.project 之前完成）
        let resolution = (
            manifest.project.resolution.width,
            manifest.project.resolution.height,
        );
        let default_text_speed = manifest.project.settings.text_speed.clone();
        let default_bgm_volume = manifest.project.settings.default_bgm_volume;
        let default_se_volume = manifest.project.settings.default_se_volume;
        let default_voice_volume = manifest.project.settings.default_voice_volume;

        Self {
            project: manifest.project,
            characters: manifest.characters,
            scenes: compiled.scenes,
            entry_scene_id: compiled.entry_scene_id,
            resolution,
            default_text_speed,
            default_bgm_volume,
            default_se_volume,
            default_voice_volume,
        }
    }

    /// 按场景 ID 获取已编译场景。
    ///
    /// # 参数
    ///
    /// - `scene_id`：场景标识符，如 `"prologue"`、`"chapter2/opening"`
    ///
    /// # 返回值
    ///
    /// - `Some(&CompiledScene)`：场景存在，返回包含字节码、常量池、标签表的编译产物
    /// - `None`：场景不存在（未编译或 ID 错误）
    ///
    /// # 示例
    ///
    /// ```
    /// # use aster_runtime::{GameContext, GameManifest};
    /// # use aster_compiler::{CompiledGame, CompiledScene, BuildInfo};
    /// # use aster_core::{Game, Resolution, GameSettings};
    /// # use std::collections::HashMap;
    /// #
    /// # let mut scenes = HashMap::new();
    /// # scenes.insert("prologue".into(), CompiledScene { version: 1, instructions: vec![], constant_pool: vec![], label_table: HashMap::new() });
    /// #
    /// # let ctx = GameContext::new(
    /// #     GameManifest {
    /// #         project: Game {
    /// #             name: "test".into(), version: "0.1.0".into(), entry_scene: "prologue".into(),
    /// #             resolution: Resolution::default(), settings: GameSettings::default(),
    /// #         },
    /// #         characters: HashMap::new(), scenes: vec![],
    /// #         build_config: aster_core::BuildConfig::default(),
    /// #     },
    /// #     CompiledGame {
    /// #         game_name: "test".into(), game_version: "0.1.0".into(),
    /// #         entry_scene_id: "prologue".into(), scenes, characters: HashMap::new(),
    /// #         build_info: BuildInfo { source_file_count: 0, total_instructions: 0, optimization_level: "optimized".into(), build_timestamp: "2026-06-15T00:00:00Z".into() },
    /// #     },
    /// # );
    /// assert!(ctx.get_scene("prologue").is_some());
    /// assert!(ctx.get_scene("nonexistent").is_none());
    /// ```
    pub fn get_scene(&self, scene_id: &str) -> Option<&CompiledScene> {
        self.scenes.get(scene_id)
    }

    /// 按角色 ID 获取角色定义。
    ///
    /// # 参数
    ///
    /// - `char_id`：角色标识符，如 `"sayori"`、`"akane"`
    ///
    /// # 返回值
    ///
    /// - `Some(&Character)`：角色存在，返回完整的角色定义（名称、颜色、立绘映射等）
    /// - `None`：角色不存在
    ///
    /// # 示例
    ///
    /// ```
    /// # use aster_runtime::{GameContext, GameManifest};
    /// # use aster_compiler::{CompiledGame, BuildInfo};
    /// # use aster_core::{Game, Character, AssetId, Resolution, GameSettings, Position};
    /// # use std::collections::HashMap;
    /// #
    /// # let mut characters = HashMap::new();
    /// # let mut sprites = HashMap::new();
    /// # sprites.insert("default".into(), AssetId(1));
    /// # characters.insert("sayori".into(), Character {
    /// #     id: "sayori".into(), name: "小百合".into(),
    /// #     display_color: "#F8BBD0".into(), description: None, birthday: None,
    /// #     default_position: Position::Center, sprites, voice: None,
    /// # });
    /// #
    /// # let ctx = GameContext::new(
    /// #     GameManifest {
    /// #         project: Game {
    /// #             name: "test".into(), version: "0.1.0".into(), entry_scene: "prologue".into(),
    /// #             resolution: Resolution::default(), settings: GameSettings::default(),
    /// #         },
    /// #         characters, scenes: vec![],
    /// #         build_config: aster_core::BuildConfig::default(),
    /// #     },
    /// #     CompiledGame {
    /// #         game_name: "test".into(), game_version: "0.1.0".into(),
    /// #         entry_scene_id: "prologue".into(),
    /// #         scenes: HashMap::new(), characters: HashMap::new(),
    /// #         build_info: BuildInfo { source_file_count: 0, total_instructions: 0, optimization_level: "optimized".into(), build_timestamp: "2026-06-15T00:00:00Z".into() },
    /// #     },
    /// # );
    /// assert!(ctx.get_character("sayori").is_some());
    /// assert!(ctx.get_character("nonexistent").is_none());
    /// ```
    pub fn get_character(&self, char_id: &str) -> Option<&Character> {
        self.characters.get(char_id)
    }

    /// 获取角色特定表情的立绘资源 ID。
    ///
    /// 通过角色 ID 和表情名查找对应的 `AssetId`。
    /// 这是查询 `Character.sprites` 映射表的便捷方法。
    ///
    /// # 参数
    ///
    /// - `char_id`：角色标识符
    /// - `emotion`：表情名，如 `"default"`、`"smile"`、`"angry"`
    ///
    /// # 返回值
    ///
    /// - `Some(AssetId)`：该角色有对应表情的立绘资源
    /// - `None`：角色不存在，或该角色没有此表情的立绘
    ///
    /// # 示例
    ///
    /// ```
    /// # use aster_runtime::{GameContext, GameManifest};
    /// # use aster_compiler::{CompiledGame, BuildInfo};
    /// # use aster_core::{Game, Character, AssetId, Resolution, GameSettings, Position};
    /// # use std::collections::HashMap;
    /// #
    /// # let mut characters = HashMap::new();
    /// # let mut sprites = HashMap::new();
    /// # sprites.insert("default".into(), AssetId(100));
    /// # sprites.insert("smile".into(), AssetId(101));
    /// # characters.insert("sayori".into(), Character {
    /// #     id: "sayori".into(), name: "小百合".into(),
    /// #     display_color: "#F8BBD0".into(), description: None, birthday: None,
    /// #     default_position: Position::Center, sprites, voice: None,
    /// # });
    /// #
    /// # let ctx = GameContext::new(
    /// #     GameManifest {
    /// #         project: Game {
    /// #             name: "test".into(), version: "0.1.0".into(), entry_scene: "prologue".into(),
    /// #             resolution: Resolution::default(), settings: GameSettings::default(),
    /// #         },
    /// #         characters, scenes: vec![],
    /// #         build_config: aster_core::BuildConfig::default(),
    /// #     },
    /// #     CompiledGame {
    /// #         game_name: "test".into(), game_version: "0.1.0".into(),
    /// #         entry_scene_id: "prologue".into(),
    /// #         scenes: HashMap::new(), characters: HashMap::new(),
    /// #         build_info: BuildInfo { source_file_count: 0, total_instructions: 0, optimization_level: "optimized".into(), build_timestamp: "2026-06-15T00:00:00Z".into() },
    /// #     },
    /// # );
    /// assert_eq!(ctx.get_character_sprite("sayori", "smile"), Some(AssetId(101)));
    /// assert_eq!(ctx.get_character_sprite("sayori", "angry"), None);
    /// assert_eq!(ctx.get_character_sprite("unknown", "default"), None);
    /// ```
    pub fn get_character_sprite(&self, char_id: &str, emotion: &str) -> Option<AssetId> {
        self.characters
            .get(char_id)
            .and_then(|character| character.sprites.get(emotion))
            .copied()
    }

    /// 按约定路径解析角色立绘文件路径。
    ///
    /// 路径约定（Architecture.md §5.2）：
    /// `assets/sprites/{char_id}/{emotion}.png`
    ///
    /// 此方法封装了资源目录结构约定，调用方（SceneManager → Renderer）
    /// 无需硬编码路径规则。
    ///
    /// # 参数
    ///
    /// - `char_id`：角色标识符
    /// - `emotion`：表情名
    ///
    /// # 返回值
    ///
    /// - `Some(PathBuf)`：约定路径（使用正斜杠作为路径分隔符，跨平台一致）
    /// - `None`：角色不存在，或该角色没有对应表情的立绘
    ///
    /// # 示例
    ///
    /// ```
    /// # use aster_runtime::{GameContext, GameManifest};
    /// # use aster_compiler::{CompiledGame, BuildInfo};
    /// # use aster_core::{Game, Character, AssetId, Resolution, GameSettings, Position};
    /// # use std::collections::HashMap;
    /// #
    /// # let mut characters = HashMap::new();
    /// # let mut sprites = HashMap::new();
    /// # sprites.insert("default".into(), AssetId(1));
    /// # characters.insert("sayori".into(), Character {
    /// #     id: "sayori".into(), name: "小百合".into(),
    /// #     display_color: "#F8BBD0".into(), description: None, birthday: None,
    /// #     default_position: Position::Center, sprites, voice: None,
    /// # });
    /// #
    /// # let ctx = GameContext::new(
    /// #     GameManifest {
    /// #         project: Game {
    /// #             name: "test".into(), version: "0.1.0".into(), entry_scene: "prologue".into(),
    /// #             resolution: Resolution::default(), settings: GameSettings::default(),
    /// #         },
    /// #         characters, scenes: vec![],
    /// #         build_config: aster_core::BuildConfig::default(),
    /// #     },
    /// #     CompiledGame {
    /// #         game_name: "test".into(), game_version: "0.1.0".into(),
    /// #         entry_scene_id: "prologue".into(),
    /// #         scenes: HashMap::new(), characters: HashMap::new(),
    /// #         build_info: BuildInfo { source_file_count: 0, total_instructions: 0, optimization_level: "optimized".into(), build_timestamp: "2026-06-15T00:00:00Z".into() },
    /// #     },
    /// # );
    /// let path = ctx.resolve_sprite_path("sayori", "default").unwrap();
    /// assert_eq!(path.to_str().unwrap(), "assets/sprites/sayori/default.png");
    /// ```
    pub fn resolve_sprite_path(&self, char_id: &str, emotion: &str) -> Option<PathBuf> {
        // 先验证角色和表情存在，避免为不存在的资源生成路径
        self.get_character_sprite(char_id, emotion)?;
        Some(PathBuf::from(format!(
            "assets/sprites/{}/{}.png",
            char_id, emotion
        )))
    }

    /// 按约定路径解析角色语音文件路径。
    ///
    /// 路径约定（Architecture.md §5.2）：
    /// `assets/voices/{char_id}/{number}.ogg`
    ///
    /// 此方法封装了语音资源目录结构约定，调用方（SceneManager → AudioSystem）
    /// 无需硬编码路径规则。
    ///
    /// # 参数
    ///
    /// - `char_id`：角色标识符
    /// - `number`：语音文件编号（如 `"001"`、`"002"`）
    ///
    /// # 返回值
    ///
    /// - `Some(PathBuf)`：约定路径（使用正斜杠作为路径分隔符，跨平台一致）
    /// - `None`：角色不存在或未启用语音配置
    ///
    /// # 示例
    ///
    /// ```
    /// # use aster_runtime::{GameContext, GameManifest};
    /// # use aster_compiler::{CompiledGame, BuildInfo};
    /// # use aster_core::{Game, Character, AssetId, Resolution, GameSettings, Position, VoiceConfig};
    /// # use std::collections::HashMap;
    /// #
    /// # let mut characters = HashMap::new();
    /// # let mut sprites = HashMap::new();
    /// # sprites.insert("default".into(), AssetId(1));
    /// # characters.insert("sayori".into(), Character {
    /// #     id: "sayori".into(), name: "小百合".into(),
    /// #     display_color: "#F8BBD0".into(), description: None, birthday: None,
    /// #     default_position: Position::Center, sprites,
    /// #     voice: Some(VoiceConfig { volume: 0.9 }),
    /// # });
    /// #
    /// # let ctx = GameContext::new(
    /// #     GameManifest {
    /// #         project: Game {
    /// #             name: "test".into(), version: "0.1.0".into(), entry_scene: "prologue".into(),
    /// #             resolution: Resolution::default(), settings: GameSettings::default(),
    /// #         },
    /// #         characters, scenes: vec![],
    /// #         build_config: aster_core::BuildConfig::default(),
    /// #     },
    /// #     CompiledGame {
    /// #         game_name: "test".into(), game_version: "0.1.0".into(),
    /// #         entry_scene_id: "prologue".into(),
    /// #         scenes: HashMap::new(), characters: HashMap::new(),
    /// #         build_info: BuildInfo { source_file_count: 0, total_instructions: 0, optimization_level: "optimized".into(), build_timestamp: "2026-06-15T00:00:00Z".into() },
    /// #     },
    /// # );
    /// let path = ctx.resolve_voice_path("sayori", "001").unwrap();
    /// assert_eq!(path.to_str().unwrap(), "assets/voices/sayori/001.ogg");
    /// // 角色未启用语音 → 返回 None
    /// assert!(ctx.resolve_voice_path("unknown", "001").is_none());
    /// ```
    pub fn resolve_voice_path(&self, char_id: &str, number: &str) -> Option<PathBuf> {
        // 验证角色存在且有语音配置
        self.characters.get(char_id)?.voice.as_ref()?;
        Some(PathBuf::from(format!(
            "assets/voices/{}/{}.ogg",
            char_id, number
        )))
    }

    /// 检查场景是否已编译并存在于上下文中。
    ///
    /// # 参数
    ///
    /// - `scene_id`：场景标识符
    ///
    /// # 返回值
    ///
    /// - `true`：场景已编译并存在
    /// - `false`：场景不存在或未编译
    ///
    /// # 示例
    ///
    /// ```
    /// # use aster_runtime::{GameContext, GameManifest};
    /// # use aster_compiler::{CompiledGame, CompiledScene, BuildInfo};
    /// # use aster_core::{Game, Resolution, GameSettings};
    /// # use std::collections::HashMap;
    /// #
    /// # let mut scenes = HashMap::new();
    /// # scenes.insert("prologue".into(), CompiledScene { version: 1, instructions: vec![], constant_pool: vec![], label_table: HashMap::new() });
    /// #
    /// # let ctx = GameContext::new(
    /// #     GameManifest {
    /// #         project: Game {
    /// #             name: "t".into(), version: "0.1.0".into(), entry_scene: "prologue".into(),
    /// #             resolution: Resolution::default(), settings: GameSettings::default(),
    /// #         },
    /// #         characters: HashMap::new(), scenes: vec![],
    /// #         build_config: aster_core::BuildConfig::default(),
    /// #     },
    /// #     CompiledGame {
    /// #         game_name: "t".into(), game_version: "0.1.0".into(),
    /// #         entry_scene_id: "prologue".into(), scenes, characters: HashMap::new(),
    /// #         build_info: BuildInfo { source_file_count: 0, total_instructions: 0, optimization_level: "optimized".into(), build_timestamp: "2026-06-15T00:00:00Z".into() },
    /// #     },
    /// # );
    /// assert!(ctx.is_scene_loaded("prologue"));
    /// assert!(!ctx.is_scene_loaded("chapter2"));
    /// ```
    pub fn is_scene_loaded(&self, scene_id: &str) -> bool {
        self.scenes.contains_key(scene_id)
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use aster_compiler::{BuildInfo, CompiledScene};
    use aster_core::{GameSettings, Position, Resolution, VoiceConfig};

    /// 创建一个用于测试的最小化 GameManifest。
    ///
    /// 包含 1 个项目（1280×720）+ 2 个角色（sayori / akane）
    /// + 空场景清单 + 默认 BuildConfig。
    fn make_test_manifest() -> GameManifest {
        let mut characters = HashMap::new();

        // 角色 sayori：2 个表情（default / smile），有语音
        {
            let mut sprites = HashMap::new();
            sprites.insert("default".into(), AssetId(100));
            sprites.insert("smile".into(), AssetId(101));
            characters.insert(
                "sayori".into(),
                Character {
                    id: "sayori".into(),
                    name: "小百合".into(),
                    display_color: "#F8BBD0".into(),
                    description: Some("温柔内向的青梅竹马".into()),
                    birthday: Some("03-21".into()),
                    default_position: Position::Center,
                    sprites,
                    voice: Some(VoiceConfig { volume: 0.9 }),
                },
            );
        }

        // 角色 akane：仅 default 表情，无语音
        {
            let mut sprites = HashMap::new();
            sprites.insert("default".into(), AssetId(200));
            characters.insert(
                "akane".into(),
                Character {
                    id: "akane".into(),
                    name: "朱音".into(),
                    display_color: "#FF5722".into(),
                    description: None,
                    birthday: None,
                    default_position: Position::Right,
                    sprites,
                    voice: None,
                },
            );
        }

        GameManifest {
            project: Game {
                name: "测试游戏".into(),
                version: "0.1.0".into(),
                entry_scene: "prologue".into(),
                resolution: Resolution {
                    width: 1280,
                    height: 720,
                },
                settings: GameSettings::default(),
            },
            characters,
            scenes: vec![],
            build_config: aster_core::BuildConfig::default(),
        }
    }

    /// 创建一个用于测试的 CompiledGame。
    ///
    /// 包含 2 个已编译场景（prologue / chapter1）。
    fn make_test_compiled() -> CompiledGame {
        let mut scenes = HashMap::new();

        // 场景 prologue：3 条指令，2 个常量，无标签
        scenes.insert(
            "prologue".into(),
            CompiledScene {
                version: 1,
                instructions: vec![0x01, 0x00, 0x00], // NOP
                constant_pool: vec!["春天".into(), "樱花".into()],
                label_table: HashMap::new(),
            },
        );

        // 场景 chapter1：5 条指令，1 个标签
        let mut label_table = HashMap::new();
        label_table.insert("start".into(), 0usize);
        scenes.insert(
            "chapter1".into(),
            CompiledScene {
                version: 1,
                instructions: vec![0x01, 0x00, 0x00, 0x02, 0x00],
                constant_pool: vec!["第1章".into()],
                label_table,
            },
        );

        CompiledGame {
            game_name: "测试游戏".into(),
            game_version: "0.1.0".into(),
            entry_scene_id: "prologue".into(),
            scenes,
            characters: HashMap::new(), // CompiledGame 的角色表不使用
            build_info: BuildInfo {
                source_file_count: 2,
                total_instructions: 8,
                optimization_level: "optimized".into(),
                build_timestamp: "2026-06-15T00:00:00Z".into(),
            },
        }
    }

    /// 创建一个完整的 GameContext 用于测试。
    fn make_test_context() -> GameContext {
        GameContext::new(make_test_manifest(), make_test_compiled())
    }

    // ─── AC01: get_scene 场景查询 ───────────────────────────────────────

    /// AC01 — `get_scene("prologue")` 返回已编译场景。
    ///
    /// 验证：
    /// 1. 存在场景返回 Some
    /// 2. 不存在场景返回 None
    /// 3. 返回的场景数据正确（指令/常量池）
    #[test]
    fn ac01_get_scene_query() {
        let ctx = make_test_context();

        // 已编译场景 — 应返回 Some
        let prologue = ctx.get_scene("prologue");
        assert!(prologue.is_some(), "prologue 场景应存在于 GameContext 中");
        let prologue = prologue.unwrap();
        assert_eq!(prologue.version, 1);
        assert_eq!(prologue.instructions.len(), 3);
        assert_eq!(prologue.constant_pool, vec!["春天", "樱花"]);

        // 不存在的场景 — 应返回 None
        assert!(
            ctx.get_scene("nonexistent").is_none(),
            "不存在的场景应返回 None"
        );
        assert!(ctx.get_scene("").is_none(), "空字符串场景 ID 应返回 None");
    }

    // ─── AC02: get_character 角色查询 ──────────────────────────────────

    /// AC02 — `get_character("sayori")` 返回完整角色定义。
    ///
    /// 验证：
    /// 1. 存在角色返回 Some
    /// 2. 角色 name 字段正确
    /// 3. 角色 sprites 映射包含预期表情
    /// 4. 不存在角色返回 None
    #[test]
    fn ac02_get_character_query() {
        let ctx = make_test_context();

        // 角色 sayori — 应返回完整定义
        let sayori = ctx.get_character("sayori");
        assert!(sayori.is_some(), "角色 sayori 应存在于 GameContext 中");
        let sayori = sayori.unwrap();
        assert_eq!(sayori.name, "小百合");
        assert_eq!(sayori.display_color, "#F8BBD0");
        assert_eq!(sayori.description.as_deref(), Some("温柔内向的青梅竹马"));
        assert_eq!(sayori.birthday.as_deref(), Some("03-21"));
        assert_eq!(sayori.default_position, Position::Center);

        // sprites 映射包含 default 和 smile
        assert!(
            sayori.sprites.contains_key("default"),
            "sayori 应有 default 表情"
        );
        assert!(
            sayori.sprites.contains_key("smile"),
            "sayori 应有 smile 表情"
        );
        assert_eq!(sayori.sprites.get("default"), Some(&AssetId(100)));

        // 语音配置
        assert!(sayori.voice.is_some());
        assert!((sayori.voice.as_ref().unwrap().volume - 0.9).abs() < f32::EPSILON);

        // 角色 akane — 无语音
        let akane = ctx.get_character("akane");
        assert!(akane.is_some());
        let akane = akane.unwrap();
        assert_eq!(akane.name, "朱音");
        assert!(akane.voice.is_none());
        assert_eq!(akane.default_position, Position::Right);

        // 不存在的角色
        assert!(
            ctx.get_character("nonexistent").is_none(),
            "不存在的角色应返回 None"
        );
    }

    // ─── AC03: get_character_sprite 表情→资源映射 ──────────────────────

    /// AC03 — `get_character_sprite("sayori", "smile")` 返回对应 AssetId。
    ///
    /// 验证：
    /// 1. 存在的表情返回 Some(AssetId)
    /// 2. 不存在的表情返回 None
    /// 3. 不存在的角色返回 None
    #[test]
    fn ac03_get_character_sprite_query() {
        let ctx = make_test_context();

        // sayori 的 default → AssetId(100)
        assert_eq!(
            ctx.get_character_sprite("sayori", "default"),
            Some(AssetId(100))
        );

        // sayori 的 smile → AssetId(101)
        assert_eq!(
            ctx.get_character_sprite("sayori", "smile"),
            Some(AssetId(101))
        );

        // sayori 不存在的表情 → None
        assert_eq!(
            ctx.get_character_sprite("sayori", "angry"),
            None,
            "sayori 没有 angry 表情，应返回 None"
        );

        // akane 的 default → AssetId(200)
        assert_eq!(
            ctx.get_character_sprite("akane", "default"),
            Some(AssetId(200))
        );

        // akane 不存在的表情 → None
        assert_eq!(ctx.get_character_sprite("akane", "smile"), None);

        // 不存在的角色 → None
        assert_eq!(
            ctx.get_character_sprite("unknown", "default"),
            None,
            "不存在的角色应返回 None"
        );
    }

    // ─── AC04: resolve_sprite_path 路径约定 ─────────────────────────────

    /// AC04 — `resolve_sprite_path("sayori", "default")` 返回约定路径。
    ///
    /// 验证：
    /// 1. 路径格式符合 `assets/sprites/{char_id}/{emotion}.png`
    /// 2. 路径使用正斜杠
    /// 3. 角色/表情不存在时返回 None
    #[test]
    fn ac04_resolve_sprite_path_convention() {
        let ctx = make_test_context();

        // sayori/default → assets/sprites/sayori/default.png
        let path = ctx.resolve_sprite_path("sayori", "default");
        assert!(path.is_some(), "sayori:default 应返回路径");
        let path_str = path.unwrap().to_str().unwrap().to_string();
        assert_eq!(
            path_str, "assets/sprites/sayori/default.png",
            "立绘路径应符合约定格式"
        );

        // sayori/smile → assets/sprites/sayori/smile.png
        let path = ctx.resolve_sprite_path("sayori", "smile").unwrap();
        assert_eq!(path.to_str().unwrap(), "assets/sprites/sayori/smile.png");

        // akane/default → assets/sprites/akane/default.png
        let path = ctx.resolve_sprite_path("akane", "default").unwrap();
        assert_eq!(path.to_str().unwrap(), "assets/sprites/akane/default.png");

        // 路径须使用正斜杠（而非反斜杠）
        assert!(
            !path_str.contains('\\'),
            "路径应使用正斜杠而非反斜杠：{path_str}"
        );

        // 不存在的表情 → None
        assert!(
            ctx.resolve_sprite_path("sayori", "angry").is_none(),
            "sayori 没有 angry 表情，resolve_sprite_path 应返回 None"
        );

        // 不存在的角色 → None
        assert!(
            ctx.resolve_sprite_path("unknown", "default").is_none(),
            "不存在的角色 resolve_sprite_path 应返回 None"
        );
    }

    // ─── AC05: 空场景集合不 panic ───────────────────────────────────────

    /// AC05 — 空场景集合不 panic。
    ///
    /// 验证：
    /// 1. scenes 为空时 get_scene 返回 None 而不 panic
    /// 2. 所有 getter 方法在空集合下安全
    #[test]
    fn ac05_empty_scenes_no_panic() {
        let manifest = GameManifest {
            project: Game {
                name: "空游戏".into(),
                version: "0.1.0".into(),
                entry_scene: "nonexistent".into(),
                resolution: Resolution::default(),
                settings: GameSettings::default(),
            },
            characters: HashMap::new(),
            scenes: vec![],
            build_config: aster_core::BuildConfig::default(),
        };

        let compiled = CompiledGame {
            game_name: "空游戏".into(),
            game_version: "0.1.0".into(),
            entry_scene_id: "nonexistent".into(),
            scenes: HashMap::new(),
            characters: HashMap::new(),
            build_info: BuildInfo {
                source_file_count: 0,
                total_instructions: 0,
                optimization_level: "optimized".into(),
                build_timestamp: "2026-06-15T00:00:00Z".into(),
            },
        };

        // 构造不应 panic（即使入口场景不存在）
        let ctx = GameContext::new(manifest, compiled);

        // 所有查询方法在空集合下安全返回
        assert!(
            ctx.get_scene("prologue").is_none(),
            "空场景集合: get_scene 应返回 None"
        );
        assert!(
            ctx.get_character("any").is_none(),
            "空角色表: get_character 应返回 None"
        );
        assert!(
            ctx.get_character_sprite("any", "default").is_none(),
            "空角色表: get_character_sprite 应返回 None"
        );
        assert!(
            ctx.resolve_sprite_path("any", "default").is_none(),
            "空角色表: resolve_sprite_path 应返回 None"
        );
        assert!(
            ctx.resolve_voice_path("any", "001").is_none(),
            "空角色表: resolve_voice_path 应返回 None"
        );
        assert!(
            !ctx.is_scene_loaded("prologue"),
            "空场景集合: is_scene_loaded 应返回 false"
        );
    }

    // ─── 补充测试: 入口场景验证 ─────────────────────────────────────────

    /// 验证入口场景不存在时仅记录 warn 日志，不 panic。
    #[test]
    fn entry_scene_missing_does_not_panic() {
        let mut manifest = make_test_manifest();
        // 设置一个不存在的入口场景
        manifest.project.entry_scene = "chapter99".into();

        let compiled = make_test_compiled();
        // compiled 中没有 "chapter99" 场景

        // 构造不应 panic
        let ctx = GameContext::new(manifest, compiled);

        // entry_scene_id 来自 compiled，不是 manifest
        assert_eq!(ctx.entry_scene_id, "prologue");

        // 查询不存在的入口场景
        assert!(ctx.get_scene("chapter99").is_none());
    }

    // ─── 补充测试: resolve_voice_path ───────────────────────────────────

    /// 验证语音路径解析逻辑。
    #[test]
    fn resolve_voice_path_behavior() {
        let ctx = make_test_context();

        // sayori 有语音配置 — 可解析路径
        let path = ctx.resolve_voice_path("sayori", "001");
        assert!(path.is_some());
        assert_eq!(
            path.unwrap().to_str().unwrap(),
            "assets/voices/sayori/001.ogg"
        );

        let path = ctx.resolve_voice_path("sayori", "042");
        assert_eq!(
            path.unwrap().to_str().unwrap(),
            "assets/voices/sayori/042.ogg"
        );

        // akane 无语音配置 — 返回 None
        assert!(
            ctx.resolve_voice_path("akane", "001").is_none(),
            "akane 无语音配置，resolve_voice_path 应返回 None"
        );

        // 不存在的角色 — 返回 None
        assert!(ctx.resolve_voice_path("unknown", "001").is_none());
    }

    // ─── 补充测试: 便捷字段提取 ─────────────────────────────────────────

    /// 验证从 GameSettings 提取的便捷字段值正确。
    #[test]
    fn convenience_fields_extraction() {
        let mut manifest = make_test_manifest();
        manifest.project.settings = GameSettings {
            language: "ja-JP".into(),
            text_speed: TextSpeed::Slow,
            default_bgm_volume: 0.5,
            default_se_volume: 0.7,
            default_voice_volume: 0.3,
        };

        let ctx = GameContext::new(manifest, make_test_compiled());

        assert_eq!(ctx.resolution, (1280, 720));
        assert_eq!(ctx.default_text_speed, TextSpeed::Slow);
        assert!((ctx.default_bgm_volume - 0.5).abs() < f32::EPSILON);
        assert!((ctx.default_se_volume - 0.7).abs() < f32::EPSILON);
        assert!((ctx.default_voice_volume - 0.3).abs() < f32::EPSILON);
    }

    // ─── 补充测试: is_scene_loaded ──────────────────────────────────────

    /// 验证 is_scene_loaded 的各种情况。
    #[test]
    fn is_scene_loaded_scenarios() {
        let ctx = make_test_context();

        // 存在的场景
        assert!(ctx.is_scene_loaded("prologue"));
        assert!(ctx.is_scene_loaded("chapter1"));

        // 不存在的场景
        assert!(!ctx.is_scene_loaded("chapter2"));
        assert!(!ctx.is_scene_loaded(""));
        assert!(!ctx.is_scene_loaded("nonexistent"));
    }

    // ─── 补充测试: GameContext Clone ────────────────────────────────────

    /// 验证 GameContext 可正确 Clone（深拷贝语义）。
    #[test]
    fn game_context_clone_is_independent() {
        let ctx = make_test_context();
        let cloned = ctx.clone();

        // Clone 后值相等
        assert_eq!(ctx.entry_scene_id, cloned.entry_scene_id);
        assert_eq!(ctx.resolution, cloned.resolution);
        assert_eq!(ctx.default_text_speed, cloned.default_text_speed);

        // Clone 后场景查询结果相同
        assert!(cloned.get_scene("prologue").is_some());
        assert!(cloned.get_character("sayori").is_some());
    }
}
