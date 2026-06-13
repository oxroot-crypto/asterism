//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/character.rs
//! 功能概述：角色定义类型 — 定义 `Character` 结构体，包含角色的标识符、显示名称、
//!           显示颜色、表情→立绘精灵映射和语音前缀等属性。
//!           对应 `.asterchar` TOML 文件的 `[character]` section。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//! - std::collections::HashMap（表情→资源 ID 映射）
//!
//! 注意：
//! - `sprites` 的 value 类型当前为 `String` 占位，将在 PH1-T03 中替换为 `AssetId`
//!
//! 对应文档：Architecture.md §4.2（核心类型清单）、§5.2（.asterchar 文件格式）

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 角色定义 — 对应 `.asterchar` TOML 文件中的 `[character]` section。
///
/// 每个角色拥有唯一的 `id`（如 `"sayori"`），在场景脚本中通过此 ID 引用。
/// `sprites` 映射表将表情名称（如 `"default"`、`"smile"`、`"angry"`）
/// 关联到对应的立绘精灵资源。
///
/// > **注意**：`sprites` 的 value 类型当前使用 `String` 占位，
/// > 待 PH1-T03 定义 `AssetId` 后替换为 `HashMap<String, AssetId>`。
///
/// # 序列化
///
/// 通过 serde 派生支持 TOML 序列化/反序列化：
/// ```rust,ignore
/// let character: Character = toml::from_str(&fs::read_to_string("characters/heroine.asterchar")?)?;
/// ```
///
/// # 示例
/// ```rust,ignore
/// use aster_core::Character;
/// use std::collections::HashMap;
///
/// let mut sprites = HashMap::new();
/// sprites.insert("default".into(), "sayori_default.png".into());
/// sprites.insert("smile".into(), "sayori_smile.png".into());
///
/// let character = Character {
///     id: "sayori".into(),
///     name: "小百合".into(),
///     display_color: "#FF6B9D".into(),
///     sprites,
///     voice_prefix: Some("sayori_".into()),
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

    /// 角色显示颜色（CSS 颜色字符串，如 `"#FF6B9D"`）
    /// 用于说话者名字的渲染颜色，增强视觉辨识度
    pub display_color: String,

    /// 表情→立绘精灵资源映射表
    ///
    /// Key 为表情名（如 `"default"`、`"smile"`、`"angry"`、`"surprised"`），
    /// Value 为对应的立绘资源标识符。
    ///
    /// **当前使用 `String` 占位**，将在 PH1-T03 中替换为 `AssetId` newtype。
    /// 必须至少包含 `"default"` 表情，作为角色的默认立绘。
    pub sprites: HashMap<String, String>,

    /// 语音文件前缀（可选）
    ///
    /// 如果角色有配音，此字段定义语音文件的命名前缀。
    /// 例如 `voice_prefix = "sayori_"` 对应文件 `sayori_001.ogg`、`sayori_002.ogg` 等。
    /// `None` 表示该角色无配音。
    pub voice_prefix: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            display_color: "#FF6B9D".into(),
            sprites: HashMap::new(),
            voice_prefix: Some("sayori_".into()),
        };

        // 插入 default 表情
        character
            .sprites
            .insert("default".into(), "sayori_default.png".into());

        // 插入 smile 表情
        character
            .sprites
            .insert("smile".into(), "sayori_smile.png".into());

        // AC03 核心断言：查询已插入的表情返回 Some
        assert_eq!(
            character.sprites.get("smile"),
            Some(&"sayori_smile.png".to_string())
        );
        assert_eq!(
            character.sprites.get("default"),
            Some(&"sayori_default.png".to_string())
        );

        // 查询不存在的表情返回 None
        assert_eq!(character.sprites.get("angry"), None);

        // 覆盖插入：同一表情名插入新值后，get 返回新值
        character
            .sprites
            .insert("smile".into(), "sayori_smile_v2.png".into());
        assert_eq!(
            character.sprites.get("smile"),
            Some(&"sayori_smile_v2.png".to_string())
        );
        // 确认只有一个 smile 条目
        assert_eq!(character.sprites.len(), 2);
    }

    /// AC03 补充 — 验证 Character 结构体的 JSON 序列化
    #[test]
    fn ac03_character_json_serialization() {
        let mut sprites = HashMap::new();
        sprites.insert("default".into(), "char_default.png".into());

        let character = Character {
            id: "hero".into(),
            name: "主人公".into(),
            display_color: "#3498DB".into(),
            sprites,
            voice_prefix: None,
        };

        let json_str = serde_json::to_string(&character).expect("JSON 序列化失败");
        let restored: Character = serde_json::from_str(&json_str).expect("JSON 反序列化失败");

        assert_eq!(restored.id, "hero");
        assert_eq!(restored.name, "主人公");
        assert_eq!(restored.display_color, "#3498DB");
        assert_eq!(
            restored.sprites.get("default"),
            Some(&"char_default.png".to_string())
        );
        assert_eq!(restored.voice_prefix, None);
    }

    /// 验证 voice_prefix 为 None 时的序列化正确性
    #[test]
    fn character_without_voice_prefix() {
        let character = Character {
            id: "narrator".into(),
            name: "旁白".into(),
            display_color: "#95A5A6".into(),
            sprites: HashMap::new(),
            voice_prefix: None,
        };

        let json_str = serde_json::to_string(&character).expect("JSON 序列化失败");
        let restored: Character = serde_json::from_str(&json_str).expect("JSON 反序列化失败");

        assert_eq!(restored.voice_prefix, None);
        assert!(restored.sprites.is_empty());
    }
}
