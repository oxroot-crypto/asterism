//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/character.rs
//! 功能概述：角色定义类型 — 定义 `Character` 结构体和 `VoiceConfig` 语音配置，
//!           包含角色的标识符、显示名称、显示颜色、简介、生日、默认位置、
//!           表情→立绘精灵映射、语音配置等属性。
//!           对应 `.asterchar` TOML 文件的 `[character]` / `[character.voice]` section。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//! - std::collections::HashMap（表情→资源 ID 映射）
//! - crate::asset::AssetId（资源唯一标识符）
//! - crate::scene::Position（立绘默认位置）
//!
//! 对应文档：Architecture.md §4.2（核心类型清单）、§5.2（.asterchar 文件格式）

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::scene::Position;

/// 语音配置 — 对应 `.asterchar` TOML 文件中的 `[character.voice]` section。
///
/// 引擎按 `assets/voices/<角色id>/<编号>.ogg` 路径自动加载语音文件，
/// 无需在此配置资源前缀。
///
/// # 序列化
///
/// ```rust,no_run
/// # use aster_core::VoiceConfig;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let voice: VoiceConfig = toml::from_str("volume = 0.9")?;
/// assert!((voice.volume - 0.9).abs() < f32::EPSILON);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceConfig {
    /// 语音音量（0.0 ~ 1.0，默认 1.0）
    /// 引擎播放该角色语音时使用此音量，可在运行时通过设置面板覆盖
    #[serde(default = "default_voice_volume")]
    pub volume: f32,
}

/// serde 默认值：语音音量默认 1.0
const fn default_voice_volume() -> f32 {
    1.0
}

/// serde 默认值：角色立绘默认位置为中央
const fn default_character_position() -> Position {
    Position::Center
}

/// 角色定义 — 对应 `.asterchar` TOML 文件中的 `[character]` section。
///
/// 每个角色拥有唯一的 `id`（如 `"sayori"`），在场景脚本中通过此 ID 引用。
/// `sprites` 映射表将表情名称（如 `"default"`、`"smile"`、`"angry"`）
/// 关联到对应的立绘文件路径。
///
/// # 资源查找约定
///
/// 立绘和语音文件按角色 ID 目录组织，无需在 sprites 值或 voice 中指定前缀：
/// - 立绘：`assets/sprites/<角色id>/<表情>.png`
/// - 语音：`assets/voices/<角色id>/<编号>.ogg`
///
/// # 序列化
///
/// 通过 serde 派生支持 TOML 序列化/反序列化。
/// `.asterchar` 文件的完整 TOML 结构由 `aster-parser::parse_character_file()`
/// 函数解析，该函数处理 `[character]` / `[character.sprites]` /
/// `[character.voice]` 三个 section 到本结构体的映射。
///
/// # 示例
/// ```
/// use aster_core::{Character, AssetId, VoiceConfig, Position};
/// use std::collections::HashMap;
///
/// let mut sprites = HashMap::new();
/// sprites.insert("default".into(), AssetId(1));
/// sprites.insert("smile".into(), AssetId(2));
///
/// let character = Character {
///     id: "sayori".into(),
///     name: "小百合".into(),
///     display_color: "#F8BBD0".into(),
///     description: Some("温柔内向的青梅竹马，喜欢樱花和文学。".into()),
///     birthday: Some("03-21".into()),
///     default_position: Position::Center,
///     sprites,
///     voice: Some(VoiceConfig { volume: 0.9 }),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Character {
    /// 角色唯一标识符（如 `"sayori"`、`"protagonist"`）
    /// 在场景脚本中通过 `show sayori at center` 引用此 ID
    pub id: String,

    /// 角色显示名称（如 `"小百合"`、`"主人公"`）
    /// 显示在对话文本框的说话者位置
    pub name: String,

    /// 角色显示颜色（HEX 颜色字符串，如 `"#F8BBD0"`）
    /// 用于说话者名字的渲染颜色，增强视觉辨识度
    pub display_color: String,

    /// 角色简介（可选）
    /// 用于 IDE 角色面板、CAST 列表、游戏内角色介绍等
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// 角色生日（可选，MM-DD 格式字符串，如 `"03-21"`）
    /// 用于引擎内置的生日彩蛋功能（v1.0.0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthday: Option<String>,

    /// 角色立绘默认舞台位置
    /// 当脚本中 `show <角色>` 未指定 `at` 子句时使用此位置
    #[serde(default = "default_character_position")]
    pub default_position: Position,

    /// 表情→立绘精灵资源映射表
    ///
    /// Key 为表情名（如 `"default"`、`"smile"`、`"angry"`、`"surprise"`），
    /// Value 为对应的立绘资源 `AssetId`。
    /// 必须至少包含 `"default"` 表情，作为角色的默认立绘。
    pub sprites: HashMap<String, AssetId>,

    /// 语音配置（可选）
    ///
    /// 为 `None` 表示该角色不启用语音。
    /// 为 `Some(VoiceConfig)` 时，引擎按 `assets/voices/<角色id>/<编号>.ogg` 加载语音。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<VoiceConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── VoiceConfig 测试 ───────────────────────────────────────────────

    /// 验证 VoiceConfig 的默认音量值
    #[test]
    fn voice_config_default_volume() {
        let voice: VoiceConfig = toml::from_str("").expect("空 TOML 应使用默认值");
        assert!(
            (voice.volume - 1.0).abs() < f32::EPSILON,
            "默认音量应为 1.0"
        );
    }

    /// 验证 VoiceConfig 自定义音量
    #[test]
    fn voice_config_custom_volume() {
        let voice: VoiceConfig = toml::from_str("volume = 0.75").expect("TOML 反序列化失败");
        assert!((voice.volume - 0.75).abs() < f32::EPSILON);
    }

    /// 验证 VoiceConfig 的 PartialEq
    #[test]
    fn voice_config_equality() {
        let v1 = VoiceConfig { volume: 0.9 };
        let v2 = VoiceConfig { volume: 0.9 };
        let v3 = VoiceConfig { volume: 0.5 };
        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    // ─── Character 测试 ─────────────────────────────────────────────────

    /// AC03 — `Character` 结构体的 `sprites` 映射可正确插入和查询表情
    ///
    /// 验证：
    /// 1. 插入表情→资源映射后，可以正确查询
    /// 2. 查询不存在的表情返回 None
    /// 3. 覆盖插入同名表情会更新值
    #[test]
    fn ac03_character_sprites_insert_and_query() {
        let mut character = Character {
            id: "sayori".into(),
            name: "小百合".into(),
            display_color: "#F8BBD0".into(),
            description: None,
            birthday: None,
            default_position: Position::Center,
            sprites: HashMap::new(),
            voice: None,
        };

        // 插入 default 表情（使用 AssetId 而非 String 占位）
        character.sprites.insert("default".into(), AssetId(100));

        // 插入 smile 表情
        character.sprites.insert("smile".into(), AssetId(101));

        // AC03 核心断言：查询已插入的表情返回 Some
        assert_eq!(character.sprites.get("smile"), Some(&AssetId(101)));
        assert_eq!(character.sprites.get("default"), Some(&AssetId(100)));

        // 查询不存在的表情返回 None
        assert_eq!(character.sprites.get("angry"), None);

        // 覆盖插入：同一表情名插入新值后，get 返回新值
        character.sprites.insert("smile".into(), AssetId(102));
        assert_eq!(character.sprites.get("smile"), Some(&AssetId(102)));
        // 确认只有一个 smile 条目
        assert_eq!(character.sprites.len(), 2);
    }

    /// AC03 补充 — 验证 Character 结构体的 JSON 序列化
    #[test]
    fn ac03_character_json_serialization() {
        let mut sprites = HashMap::new();
        sprites.insert("default".into(), AssetId(200));

        let character = Character {
            id: "hero".into(),
            name: "主人公".into(),
            display_color: "#3498DB".into(),
            description: Some("游戏主人公，转学到樱花镇的高中生。".into()),
            birthday: Some("06-15".into()),
            default_position: Position::Left,
            sprites,
            voice: Some(VoiceConfig { volume: 0.8 }),
        };

        let json_str = serde_json::to_string(&character).expect("JSON 序列化失败");
        let restored: Character = serde_json::from_str(&json_str).expect("JSON 反序列化失败");

        assert_eq!(restored.id, "hero");
        assert_eq!(restored.name, "主人公");
        assert_eq!(restored.display_color, "#3498DB");
        assert_eq!(
            restored.description.as_deref(),
            Some("游戏主人公，转学到樱花镇的高中生。")
        );
        assert_eq!(restored.birthday.as_deref(), Some("06-15"));
        assert_eq!(restored.default_position, Position::Left);
        assert_eq!(restored.sprites.get("default"), Some(&AssetId(200)));
        assert!(restored.voice.is_some());
        assert!((restored.voice.unwrap().volume - 0.8).abs() < f32::EPSILON);
    }

    /// 验证 Character 可选字段为 None 时的 JSON 序列化（skip_serializing_if）
    #[test]
    fn character_optional_fields_none() {
        let character = Character {
            id: "minimal".into(),
            name: "最小角色".into(),
            display_color: "#000000".into(),
            description: None,
            birthday: None,
            default_position: Position::Center,
            sprites: HashMap::new(),
            voice: None,
        };

        let json_str = serde_json::to_string(&character).expect("JSON 序列化失败");

        // None 字段不应出现在 JSON 中
        assert!(!json_str.contains("description"));
        assert!(!json_str.contains("birthday"));
        assert!(!json_str.contains("voice"));

        let restored: Character = serde_json::from_str(&json_str).expect("JSON 反序列化失败");
        assert_eq!(restored.description, None);
        assert_eq!(restored.birthday, None);
        assert_eq!(restored.voice, None);
        // default_position 有 serde default，缺失时应为 Center
        // （但 JSON 序列化会包含它，因为 Position 总是序列化）
    }

    /// 验证 default_position 的反序列化默认值
    #[test]
    fn character_default_position_deserialization() {
        // JSON 中省略 default_position 时应回退到 Center
        let json = r##"{
            "id": "test",
            "name": "测试角色",
            "display_color": "#FFFFFF",
            "sprites": {}
        }"##;
        let character: Character = serde_json::from_str(json).expect("JSON 反序列化失败");
        assert_eq!(character.default_position, Position::Center);
    }
}
