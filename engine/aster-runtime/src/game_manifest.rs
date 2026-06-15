//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/game_manifest.rs
//! 功能概述：游戏清单类型 — 定义 `GameManifest` 和 `SceneEntry` 结构体，
//!           是 `GameLoader` 的产出物，持有项目元数据、角色表、场景清单和构建配置。
//!           供 `GameCompiler`（PH1-T16）和 `GameContext`（PH1-T17）消费。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster_core（Game / Character / BuildConfig 等核心类型）
//! - std::collections::HashMap（角色表）
//! - std::path::PathBuf（场景文件路径）
//!
//! 对应文档：Phase-1-Tasks.md PH1-T15（GameManifest 定义）

use std::collections::HashMap;
use std::path::PathBuf;

use aster_core::{BuildConfig, Character, Game};

/// 游戏清单 — `GameLoader::load()` 的产出物。
///
/// 包含从项目目录中加载的所有结构化信息：
/// - 项目元数据（`aster.toml`）
/// - 所有角色定义（`characters/*.asterchar`）
/// - 所有场景文件列表（`scripts/**/*.aster`）
/// - 构建配置（`build.toml`）
///
/// 这是引擎启动和项目编译的共同输入：
/// ```text
/// 项目磁盘目录 → GameLoader → GameManifest → GameCompiler（PH1-T16）
///                                            → GameContext（PH1-T17）
///                                            → SceneManager（PH1-T18）
/// ```
///
/// # 示例
/// ```no_run
/// use aster_runtime::{GameLoader, GameManifest};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // 加载模板项目
/// let manifest = GameLoader::load(Path::new("templates/default_project/"))?;
/// assert_eq!(manifest.project.name, "My First Visual Novel");
/// assert_eq!(manifest.scenes.len(), 2);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GameManifest {
    /// 项目元数据 — 来自 `aster.toml`
    pub project: Game,

    /// 角色表 — key 为角色 ID（如 `"sayori"`、`"akane"`），
    /// value 为对应的 `Character` 定义（来自 `characters/*.asterchar`）
    pub characters: HashMap<String, Character>,

    /// 场景清单 — 包含 `scripts/` 目录下发现的所有 `.aster` 文件
    pub scenes: Vec<SceneEntry>,

    /// 构建配置 — 来自 `build.toml`（不存在时使用默认值）
    pub build_config: BuildConfig,
}

/// 场景条目 — `scripts/` 目录下发现的单个 `.aster` 场景文件。
///
/// 记录场景的标识符（相对路径去前缀/扩展名）、文件路径和是否为入口场景。
///
/// # 场景 ID 约定
///
/// 场景 ID 从文件路径推导：
/// - `scripts/prologue.aster` → `"prologue"`
/// - `scripts/chapter1/sakura_road.aster` → `"chapter1/sakura_road"`
///
/// 这与 `.aster` 脚本内 `scene "xxx"` 声明的 ID 应保持一致（由创作者负责）。
///
/// # 示例
/// ```
/// use aster_runtime::SceneEntry;
/// use std::path::PathBuf;
///
/// let entry = SceneEntry {
///     scene_id: "prologue".into(),
///     file_path: PathBuf::from("scripts/prologue.aster"),
///     is_entry: true,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneEntry {
    /// 场景标识符 — 如 `"prologue"`、`"chapter1/sakura_road"`
    /// 从文件路径推导（去掉 `scripts/` 前缀和 `.aster` 扩展名）
    pub scene_id: String,

    /// 场景文件路径 — 相对于项目根目录，如 `"scripts/chapter1/prologue.aster"`
    pub file_path: PathBuf,

    /// 是否为入口场景 — 与 `Game.entry_scene` 匹配时为 `true`
    pub is_entry: bool,
}
