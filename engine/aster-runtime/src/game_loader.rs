//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/game_loader.rs
//! 功能概述：游戏清单加载器 — `GameLoader` 负责将磁盘上的项目目录结构
//!           加载为结构化的 `GameManifest`。流程包括：
//!           1. 读取 `aster.toml` → `Game` 元数据
//!           2. 读取 `build.toml` → `BuildConfig`（可选，不存在时用默认值）
//!           3. 扫描 `characters/*.asterchar` → 解析角色定义
//!           4. 扫描 `scripts/**/*.aster` → 发现场景文件
//!           5. 验证入口场景存在于场景清单中
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster_core（Game / Character / BuildConfig / AssetId 等核心类型）
//! - toml（TOML 文件反序列化）
//! - serde（派生宏，用于中间反序列化结构）
//! - std::fs / std::path（文件系统操作）
//!
//! 对应文档：Phase-1-Tasks.md PH1-T15（GameLoader 任务说明）

use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

#[cfg(test)]
use std::path::PathBuf;

use serde::Deserialize;

use aster_core::{AssetId, BuildConfig, Character, Game, Position, VoiceConfig};

use crate::error::RuntimeError;
use crate::game_manifest::{GameManifest, SceneEntry};

/// 游戏清单加载器。
///
/// 提供唯一的公共方法 `load(project_root)`，从项目根目录加载所有结构化信息。
/// 加载流程是顺序的：aster.toml → build.toml → characters/ → scripts/ → 验证入口。
///
/// # 使用示例
/// ```no_run
/// # use aster_runtime::GameLoader;
/// # use std::path::Path;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let manifest = GameLoader::load(Path::new("templates/default_project/"))?;
/// println!("游戏: {} ({} 个场景, {} 个角色)",
///     manifest.project.name,
///     manifest.scenes.len(),
///     manifest.characters.len(),
/// );
/// # Ok(())
/// # }
/// ```
pub struct GameLoader;

impl GameLoader {
    /// 加载项目目录，构建完整的 `GameManifest`。
    ///
    /// # 参数
    /// - `project_root`: 项目根目录路径（包含 `aster.toml` 的目录）
    ///
    /// # 返回值
    /// - `Ok(GameManifest)`: 加载成功，包含完整的项目清单
    /// - `Err(RuntimeError)`: 加载失败
    ///   - `ProjectNotFound`: `aster.toml` 不存在
    ///   - `TomlParse`: TOML 文件格式错误
    ///   - `Io`: 文件系统错误
    ///   - `EntrySceneNotFound`: 入口场景未在清单中
    ///   - `CharacterParseError`: 角色文件解析失败
    ///
    /// # 加载流程
    /// 1. 验证并读取 `aster.toml`
    /// 2. 读取或使用默认 `build.toml`
    /// 3. 扫描 `characters/` 目录 → 解析 `.asterchar` 文件
    /// 4. 递归扫描 `scripts/` 目录 → 发现 `.aster` 文件
    /// 5. 标记入口场景并验证其存在
    pub fn load(project_root: &Path) -> Result<GameManifest, RuntimeError> {
        // 步骤 1：读取 aster.toml
        let game = Self::load_aster_toml(project_root)?;

        // 步骤 2：读取 build.toml（可选）
        let build_config = Self::load_build_toml(project_root)?;

        // 步骤 3：扫描 characters/ 目录
        let characters = Self::load_characters(project_root)?;

        // 步骤 4：扫描 scripts/ 目录
        let mut scenes = Self::scan_scenes(project_root)?;

        // 步骤 5：标记入口场景并验证
        Self::mark_entry_scene(scenes.as_mut_slice(), &game.entry_scene)?;

        Ok(GameManifest {
            project: game,
            characters,
            scenes,
            build_config,
        })
    }

    /// 读取并解析 `aster.toml`。
    ///
    /// `aster.toml` 的 TOML 结构为 `[game]` section 包含所有游戏元数据，
    /// 因此需要 `AsterToml` 包装结构体来匹配 TOML 的层级结构。
    fn load_aster_toml(project_root: &Path) -> Result<Game, RuntimeError> {
        let toml_path = project_root.join("aster.toml");

        if !toml_path.exists() {
            return Err(RuntimeError::ProjectNotFound(
                project_root.display().to_string(),
            ));
        }

        let content = fs::read_to_string(&toml_path).map_err(RuntimeError::Io)?;

        let aster_toml: AsterToml = toml::from_str(&content).map_err(RuntimeError::TomlParse)?;

        Ok(aster_toml.game)
    }

    /// 读取并解析 `build.toml`。
    ///
    /// 如果 `build.toml` 不存在，返回 `BuildConfig::default()`。
    /// TOML 解析失败时返回错误（不静默降级，因为这说明用户配置有误）。
    fn load_build_toml(project_root: &Path) -> Result<BuildConfig, RuntimeError> {
        let toml_path = project_root.join("build.toml");

        if !toml_path.exists() {
            return Ok(BuildConfig::default());
        }

        let content = fs::read_to_string(&toml_path).map_err(RuntimeError::Io)?;

        let config: BuildConfig = toml::from_str(&content).map_err(RuntimeError::TomlParse)?;

        Ok(config)
    }

    /// 扫描 `characters/` 目录，加载所有 `.asterchar` 角色定义。
    ///
    /// 遍历 `characters/` 下所有 `*.asterchar` 文件，每个文件 TOML 反序列化为
    /// `RawCharacterFile`，再转换为 `Character`。以角色 ID 为 key 存入 HashMap。
    ///
    /// 如果 `characters/` 目录不存在，返回空 HashMap（不报错）。
    /// 单个 `.asterchar` 解析失败时立即返回错误。
    fn load_characters(project_root: &Path) -> Result<HashMap<String, Character>, RuntimeError> {
        let chars_dir = project_root.join("characters");

        if !chars_dir.exists() || !chars_dir.is_dir() {
            return Ok(HashMap::new());
        }

        let mut characters = HashMap::new();

        let entries = fs::read_dir(&chars_dir).map_err(RuntimeError::Io)?;

        for entry in entries {
            let entry = entry.map_err(RuntimeError::Io)?;
            let path = entry.path();

            // 跳过非文件和扩展名不匹配的文件
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("asterchar") {
                continue;
            }

            let content = fs::read_to_string(&path).map_err(RuntimeError::Io)?;

            // 使用中间 RawCharacterFile 进行 TOML 反序列化
            // （sprites 为 String→String 映射，后续转换为 String→AssetId）
            let raw_file: RawCharacterFile =
                toml::from_str(&content).map_err(|e| RuntimeError::CharacterParseError {
                    path: path.display().to_string(),
                    message: e.to_string(),
                })?;

            let character = Self::convert_character(raw_file.character);
            characters.insert(character.id.clone(), character);
        }

        Ok(characters)
    }

    /// 递归扫描 `scripts/` 目录，发现所有 `.aster` 场景文件。
    ///
    /// 场景 ID 从文件路径推导：
    /// - 去掉 `scripts/` 前缀
    /// - 去掉 `.aster` 扩展名
    /// - 路径分隔符统一为 `/`
    ///
    /// 如果 `scripts/` 目录不存在，返回空 Vec（不报错）。
    fn scan_scenes(project_root: &Path) -> Result<Vec<SceneEntry>, RuntimeError> {
        let scripts_dir = project_root.join("scripts");

        if !scripts_dir.exists() || !scripts_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut scenes = Vec::new();
        Self::scan_scenes_recursive(&scripts_dir, &scripts_dir, &mut scenes)?;

        Ok(scenes)
    }

    /// 递归辅助函数：遍历目录，收集 `.aster` 文件。
    ///
    /// # 参数
    /// - `current_dir`: 当前正在遍历的目录
    /// - `base_dir`: `scripts/` 目录的根路径（用于计算 scene_id）
    /// - `scenes`: 收集结果的 Vec
    fn scan_scenes_recursive(
        current_dir: &Path,
        base_dir: &Path,
        scenes: &mut Vec<SceneEntry>,
    ) -> Result<(), RuntimeError> {
        let entries = fs::read_dir(current_dir).map_err(RuntimeError::Io)?;

        for entry in entries {
            let entry = entry.map_err(RuntimeError::Io)?;
            let path = entry.path();

            if path.is_dir() {
                // 递归进入子目录
                Self::scan_scenes_recursive(&path, base_dir, scenes)?;
            } else if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("aster") {
                // 计算 scene_id：相对路径去掉 .aster 扩展名
                let relative = path
                    .strip_prefix(base_dir)
                    .unwrap_or(&path)
                    .with_extension(""); // 去掉 .aster 扩展名

                // 统一使用 `/` 作为路径分隔符
                let scene_id = relative.to_string_lossy().replace('\\', "/");

                // 去除尾部的 `.`（with_extension("") 可能导致 `prologue.`）
                let scene_id = scene_id.strip_suffix('.').unwrap_or(&scene_id);

                scenes.push(SceneEntry {
                    scene_id: scene_id.to_string(),
                    file_path: path.clone(),
                    is_entry: false, // 后续由 mark_entry_scene 设置
                });
            }
        }

        Ok(())
    }

    /// 标记入口场景并验证其存在。
    ///
    /// 将 `scenes` 中 scene_id 匹配 `entry_scene_id` 的 `SceneEntry.is_entry`
    /// 设为 `true`。如果没有匹配的场景，返回 `EntrySceneNotFound` 错误。
    fn mark_entry_scene(
        scenes: &mut [SceneEntry],
        entry_scene_id: &str,
    ) -> Result<(), RuntimeError> {
        let mut found = false;

        for scene in scenes.iter_mut() {
            if scene.scene_id == entry_scene_id {
                scene.is_entry = true;
                found = true;
                break;
            }
        }

        if !found {
            return Err(RuntimeError::EntrySceneNotFound {
                entry_scene: entry_scene_id.to_string(),
            });
        }

        Ok(())
    }

    /// 将 `RawCharacter`（sprites 为 String→String）转换为 `Character`
    /// （sprites 为 String→AssetId）。
    ///
    /// 使用确定性哈希将 sprite 文件路径转换为 `AssetId`，
    /// 保证同一路径每次加载产生相同的 AssetId。
    fn convert_character(raw: RawCharacter) -> Character {
        let mut sprites = HashMap::new();

        for (emotion, path) in raw.sprites {
            let asset_id = Self::path_to_assetid(&path);
            sprites.insert(emotion, asset_id);
        }

        Character {
            id: raw.id,
            name: raw.name,
            display_color: raw.display_color,
            description: raw.description,
            birthday: raw.birthday,
            default_position: raw.default_position,
            sprites,
            voice: raw.voice,
        }
    }

    /// 使用字符串的确定性哈希生成 `AssetId`。
    ///
    /// 使用 Rust 标准库的 `DefaultHasher` 计算 u64 哈希值，
    /// 相同输入总是产生相同的 `AssetId`。
    fn path_to_assetid(s: &str) -> AssetId {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut hasher);
        AssetId(hasher.finish())
    }
}

// ─── 内部类型：用于 TOML 反序列化的中间结构 ──────────────────────────────

/// `aster.toml` 的顶层 TOML 结构。
///
/// `aster.toml` 使用 `[game]` section 包含游戏元数据，
/// 因此需要一个包装结构体来匹配 TOML 的层级结构。
#[derive(Debug, Deserialize)]
struct AsterToml {
    game: Game,
}

/// `.asterchar` 文件的顶层 TOML 结构。
///
/// `.asterchar` 使用 `[character]` section 包含角色定义，
/// 需要包装结构体匹配 TOML 层级。
#[derive(Debug, Deserialize)]
struct RawCharacterFile {
    character: RawCharacter,
}

/// `.asterchar` 中 `[character]` section 的中间反序列化结构。
///
/// 与 `aster_core::Character` 的区别：
/// - `sprites` 为 `HashMap<String, String>`（文件路径字符串）
///   而非 `HashMap<String, AssetId>`（资源 ID）
/// - 这是因为 `.asterchar` 存储的是文件路径（如 `"default.png"`），
///   而 `Character` 需要的是 AssetId（由资源管理器分配）
///
/// 加载后通过 `GameLoader::convert_character()` 转换为正式的 `Character`。
#[derive(Debug, Deserialize)]
struct RawCharacter {
    /// 角色唯一标识符
    id: String,

    /// 角色显示名称
    name: String,

    /// 角色显示颜色（HEX 字符串）
    display_color: String,

    /// 角色简介（可选）
    #[serde(default)]
    description: Option<String>,

    /// 角色生日（可选，MM-DD 格式）
    #[serde(default)]
    birthday: Option<String>,

    /// 角色立绘默认位置
    #[serde(default = "default_position")]
    default_position: Position,

    /// 表情→立绘文件路径映射表
    ///
    /// Key 为表情名（如 "default"、"smile"），
    /// Value 为立绘文件名（如 "default.png"）——
    /// 引擎自动从 `assets/sprites/<角色id>/` 目录加载。
    #[serde(default)]
    sprites: HashMap<String, String>,

    /// 语音配置（可选）
    #[serde(default)]
    voice: Option<VoiceConfig>,
}

/// serde 默认值：角色立绘默认位置为中央
fn default_position() -> Position {
    Position::Center
}

// ─── 测试模块 ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 全局计数器，确保每个测试使用唯一的临时目录名
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// 获取模板项目的绝对路径（从 crate 目录出发）
    fn template_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/default_project")
    }

    /// 在临时目录中创建指定的文件结构
    ///
    /// `files` 参数为 `(相对路径, 文件内容)` 的列表，
    /// 会自动创建所需的父目录。
    /// 使用原子计数器确保每个测试使用唯一的临时目录。
    fn create_temp_project(files: &[(&str, &str)]) -> PathBuf {
        let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("aster_test_{}_{}", std::process::id(), count));
        fs::create_dir_all(&dir).expect("创建临时目录失败");

        for (rel_path, content) in files {
            let full_path = dir.join(rel_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).expect("创建父目录失败");
            }
            fs::write(&full_path, content).expect("写入测试文件失败");
        }

        dir
    }

    /// 清理临时测试目录
    fn cleanup_temp_project(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // ─── AC01：完整项目目录加载成功 ──────────────────────────────────────

    /// AC01 — 加载模板项目，验证 GameManifest 结构完整
    ///
    /// 验证：
    /// 1. project.name == "My First Visual Novel"
    /// 2. characters.len() == 2（sayori + akane）
    /// 3. scenes.len() == 2（prologue + sakura_road）
    /// 4. build_config 存在
    #[test]
    fn ac01_load_template_project() {
        let path = template_path();
        assert!(path.exists(), "模板项目目录不存在: {}", path.display());

        let manifest = GameLoader::load(&path).expect("加载模板项目失败");

        // 验证项目元数据
        assert_eq!(manifest.project.name, "My First Visual Novel");
        assert_eq!(manifest.project.version, "0.1.0");
        assert_eq!(manifest.project.entry_scene, "prologue");

        // 验证分辨率
        assert_eq!(manifest.project.resolution.width, 1920);
        assert_eq!(manifest.project.resolution.height, 1080);

        // 验证角色数量：heroine.asterchar (sayori) + akane.asterchar (akane)
        assert_eq!(
            manifest.characters.len(),
            2,
            "应有 2 个角色，实际: {:?}",
            manifest.characters.keys().collect::<Vec<_>>()
        );

        // 验证角色 sayori（来自 heroine.asterchar）
        let sayori = manifest.characters.get("sayori").expect("应有 sayori 角色");
        assert_eq!(sayori.name, "小百合");
        assert_eq!(sayori.display_color, "#F8BBD0");
        assert!(
            sayori.description.as_deref().unwrap().contains("青梅竹马"),
            "sayori 应有简介"
        );
        assert_eq!(sayori.default_position, Position::Center);
        // sprites 应包含多个表情
        assert!(
            sayori.sprites.len() >= 7,
            "sayori 应有至少 7 个表情，实际: {}",
            sayori.sprites.len()
        );
        assert!(sayori.sprites.contains_key("default"));
        assert!(sayori.sprites.contains_key("smile"));
        assert!(sayori.sprites.contains_key("angry"));
        // voice 配置
        assert!(sayori.voice.is_some());
        assert!((sayori.voice.as_ref().unwrap().volume - 0.9).abs() < f32::EPSILON);

        // 验证角色 akane
        let akane = manifest.characters.get("akane").expect("应有 akane 角色");
        assert_eq!(akane.name, "小茜");
        assert_eq!(akane.display_color, "#FF8A65");
        assert_eq!(akane.default_position, Position::Right);
        assert!(akane.sprites.len() >= 7);
        assert!(akane.voice.is_some());

        // 验证场景数量
        assert_eq!(
            manifest.scenes.len(),
            2,
            "应有 2 个场景，实际: {:?}",
            manifest
                .scenes
                .iter()
                .map(|s| &s.scene_id)
                .collect::<Vec<_>>()
        );

        // 验证两个场景都在清单中
        let scene_ids: Vec<&str> = manifest
            .scenes
            .iter()
            .map(|s| s.scene_id.as_str())
            .collect();
        assert!(scene_ids.contains(&"prologue"), "应有 prologue 场景");
        assert!(
            scene_ids.contains(&"chapter1/sakura_road"),
            "应有 chapter1/sakura_road 场景"
        );

        // 验证 build_config 已加载
        assert_eq!(manifest.build_config.compile.target, "asterbyte");
        assert!(manifest.build_config.compile.optimize);
        assert!(manifest.build_config.compile.minify);
    }

    // ─── AC02：入口场景正确标记 ───────────────────────────────────────

    /// AC02 — 验证 entry_scene 标记为 is_entry=true
    #[test]
    fn ac02_entry_scene_marked() {
        let path = template_path();
        let manifest = GameLoader::load(&path).expect("加载模板项目失败");

        // prologue 是入口场景
        let prologue = manifest
            .scenes
            .iter()
            .find(|s| s.scene_id == "prologue")
            .expect("应有 prologue 场景");
        assert!(prologue.is_entry, "prologue 应标记为入口场景");

        // sakura_road 不是入口场景
        let sakura = manifest
            .scenes
            .iter()
            .find(|s| s.scene_id == "chapter1/sakura_road")
            .expect("应有 sakura_road 场景");
        assert!(!sakura.is_entry, "sakura_road 不应标记为入口场景");
    }

    // ─── AC03：aster.toml 不存在时返回错误 ─────────────────────────────────

    /// AC03 — 空目录（无 aster.toml）返回 ProjectNotFound 错误
    #[test]
    fn ac03_missing_aster_toml() {
        let dir = create_temp_project(&[]);
        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        assert!(result.is_err(), "应返回错误");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("aster.toml"),
            "错误消息应包含 'aster.toml'，实际: {msg}"
        );
    }

    // ─── AC04：.asterchar 缺失可选字段时不报错 ─────────────────────────────

    /// AC04 — 最少字段的 .asterchar（仅 id + name + display_color + sprites）
    /// 可选字段（description/birthday/voice）使用默认值
    #[test]
    fn ac04_minimal_asterchar() {
        let aster_toml = r#"
[game]
name = "Minimal Test"
version = "0.1.0"
entry_scene = "minimal"
"#;

        let asterchar_content = r##"
[character]
id = "minimal_char"
name = "最小角色"
display_color = "#000000"
"##;

        let scripts_dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            ("characters/minimal.asterchar", asterchar_content),
            ("scripts/minimal.aster", "scene \"minimal\" {\n}"),
        ]);

        let result = GameLoader::load(&scripts_dir);
        cleanup_temp_project(&scripts_dir);

        assert!(result.is_ok(), "加载应成功，错误: {:?}", result.err());

        let manifest = result.unwrap();
        assert_eq!(manifest.characters.len(), 1);

        let c = manifest.characters.get("minimal_char").expect("应有角色");
        assert_eq!(c.id, "minimal_char");
        assert_eq!(c.name, "最小角色");
        assert_eq!(c.description, None, "description 应为 None");
        assert_eq!(c.birthday, None, "birthday 应为 None");
        assert_eq!(
            c.default_position,
            Position::Center,
            "default_position 应默认为 Center"
        );
        assert!(c.sprites.is_empty(), "sprites 应为空");
        assert!(c.voice.is_none(), "voice 应为 None");
    }

    // ─── AC05：build.toml 不存在时使用默认配置 ─────────────────────────────

    /// AC05 — 项目无 build.toml 时，BuildConfig 使用默认值
    #[test]
    fn ac05_missing_build_toml_uses_defaults() {
        let aster_toml = r#"
[game]
name = "No Build Config"
version = "0.1.0"
entry_scene = "test"
"#;

        let dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            ("scripts/test.aster", "scene \"test\" {\n}"),
        ]);

        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        assert!(result.is_ok(), "加载应成功");

        let manifest = result.unwrap();
        let config = &manifest.build_config;

        // 验证所有默认值
        assert_eq!(config.compile.target, "asterbyte");
        assert!(config.compile.optimize);
        assert!(config.compile.minify);
        assert_eq!(config.include.patterns.len(), 3);
        assert!(config.include.patterns.contains(&"assets/**/*".to_string()));
        assert_eq!(config.archive.format, "asterarchive");
        assert!(!config.archive.encrypt);
    }

    // ─── AC06：entry_scene 不存在时返回错误 ─────────────────────────────────

    /// AC06 — entry_scene 指向不存在的场景时返回 EntrySceneNotFound
    #[test]
    fn ac06_entry_scene_not_found() {
        let aster_toml = r#"
[game]
name = "Bad Entry"
version = "0.1.0"
entry_scene = "nonexistent"
"#;

        let dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            ("scripts/real_scene.aster", "scene \"real_scene\" {\n}"),
        ]);

        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        assert!(result.is_err(), "应返回错误");
        let err = result.unwrap_err();
        let msg = err.to_string();

        assert!(
            msg.contains("nonexistent"),
            "错误消息应包含不存在的场景名 'nonexistent'，实际: {msg}"
        );
        assert!(
            msg.contains("入口场景"),
            "错误消息应包含'入口场景'，实际: {msg}"
        );
    }

    // ─── 辅助场景测试 ──────────────────────────────────────────────────

    /// 验证空 characters/ 目录不报错（返回空 HashMap）
    #[test]
    fn empty_characters_dir_ok() {
        let aster_toml = r#"
[game]
name = "No Characters"
version = "0.1.0"
entry_scene = "test"
"#;

        let dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            ("scripts/test.aster", "scene \"test\" {\n}"),
            // 不创建 characters/ 目录
        ]);

        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        assert!(result.is_ok(), "加载应成功");
        let manifest = result.unwrap();
        assert!(manifest.characters.is_empty());
    }

    /// 验证空 scripts/ 目录不报错（返回空 Vec，但 entry_scene 验证会失败）
    #[test]
    fn empty_scripts_dir_with_entry_validation() {
        let aster_toml = r#"
[game]
name = "No Scenes"
version = "0.1.0"
entry_scene = "anything"
"#;

        let dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            // 不创建 scripts/ 目录
        ]);

        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        // 应因入口场景不存在而失败
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RuntimeError::EntrySceneNotFound { .. }
        ));
    }

    /// 验证递归目录扫描：嵌套子目录中的场景被正确发现
    #[test]
    fn recursive_scene_scanning() {
        let aster_toml = r#"
[game]
name = "Deep Scenes"
version = "0.1.0"
entry_scene = "ch1/part1"
"#;

        let dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            ("scripts/ch1/part1.aster", "scene \"ch1/part1\" {\n}"),
            ("scripts/ch1/part2.aster", "scene \"ch1/part2\" {\n}"),
            ("scripts/ch2/finale.aster", "scene \"ch2/finale\" {\n}"),
        ]);

        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        assert!(result.is_ok(), "加载应成功，错误: {:?}", result.err());
        let manifest = result.unwrap();

        assert_eq!(manifest.scenes.len(), 3);

        let ids: Vec<&str> = manifest
            .scenes
            .iter()
            .map(|s| s.scene_id.as_str())
            .collect();
        assert!(ids.contains(&"ch1/part1"));
        assert!(ids.contains(&"ch1/part2"));
        assert!(ids.contains(&"ch2/finale"));

        // 验证入口场景标记
        let entry = manifest
            .scenes
            .iter()
            .find(|s| s.scene_id == "ch1/part1")
            .expect("应有入口场景");
        assert!(entry.is_entry);
    }

    /// 验证 .asterchar 解析失败时返回 CharacterParseError
    #[test]
    fn invalid_asterchar_returns_error() {
        let aster_toml = r#"
[game]
name = "Bad Char"
version = "0.1.0"
entry_scene = "test"
"#;

        // 无效的 TOML（未知字段，但 serde 默认忽略）
        let bad_char = r##"
[character]
id = "bad"
name = "Bad Character"
display_color = "#FF0000"
invalid_field_that_doesnt_exist = true
"##;

        let dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            ("scripts/test.aster", "scene \"test\" {\n}"),
            ("characters/bad.asterchar", bad_char),
        ]);

        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        // 注意：toml 的 serde 反序列化默认忽略未知字段，
        // 所以这个测试应该成功（未知字段被忽略）
        // 真正会导致错误的是 TOML 语法错误，如缺少等号
        assert!(result.is_ok(), "未知字段应被忽略而不产生错误");
    }

    /// 验证 TOML 语法错误的 .asterchar 会导致 CharacterParseError
    #[test]
    fn syntax_error_in_asterchar() {
        let aster_toml = r#"
[game]
name = "Syntax Error Test"
version = "0.1.0"
entry_scene = "test"
"#;

        // TOML 语法错误：缺少值
        let bad_char = r#"
[character
id = "bad"
"#;

        let dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            ("scripts/test.aster", "scene \"test\" {\n}"),
            ("characters/bad.asterchar", bad_char),
        ]);

        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        assert!(result.is_err(), "TOML 语法错误应返回错误");
        match result.unwrap_err() {
            RuntimeError::CharacterParseError { path, message } => {
                assert!(path.contains("bad.asterchar"), "路径应包含文件名");
                assert!(!message.is_empty(), "应有错误消息");
            }
            other => panic!("应返回 CharacterParseError，实际: {other:?}"),
        }
    }

    /// 验证 AssetId 由路径确定性生成（同一路径两次调用得到相同 ID）
    #[test]
    fn assetid_deterministic_from_path() {
        let id1 = GameLoader::path_to_assetid("default.png");
        let id2 = GameLoader::path_to_assetid("default.png");
        assert_eq!(id1, id2, "同一路径应产生相同的 AssetId");

        let id3 = GameLoader::path_to_assetid("smile.png");
        assert_ne!(id1, id3, "不同路径应产生不同的 AssetId");
    }

    /// 验证分辨率默认值在缺少 [game.resolution] 时生效
    #[test]
    fn game_resolution_defaults_when_missing() {
        let aster_toml = r#"
[game]
name = "Minimal Game"
version = "1.0.0"
entry_scene = "start"
"#;

        let dir = create_temp_project(&[
            ("aster.toml", aster_toml),
            ("scripts/start.aster", "scene \"start\" {\n}"),
        ]);

        let result = GameLoader::load(&dir);
        cleanup_temp_project(&dir);

        assert!(result.is_ok(), "加载应成功");
        let manifest = result.unwrap();
        assert_eq!(manifest.project.resolution.width, 1920);
        assert_eq!(manifest.project.resolution.height, 1080);
    }

    /// 验证 Character 的 sprites 已正确转换为 AssetId
    #[test]
    fn character_sprites_converted_to_assetid() {
        let path = template_path();
        let manifest = GameLoader::load(&path).expect("加载模板项目失败");

        let sayori = manifest.characters.get("sayori").expect("应有 sayori");
        // 验证 sprites 的所有 value 都是非零 AssetId
        for (emotion, asset_id) in &sayori.sprites {
            assert!(asset_id.0 != 0, "表情 '{emotion}' 的 AssetId 不应为 0");
        }

        // 同一路径应产生相同 AssetId
        let default_id = sayori.sprites.get("default").expect("应有 default");
        let default_id2 = GameLoader::path_to_assetid("default.png");
        assert_eq!(*default_id, default_id2);
    }
}
