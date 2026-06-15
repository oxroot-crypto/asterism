//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/asset.rs
//! 功能概述：资源类型定义 — `AssetId`（newtype 资源标识符）、`AssetType`（资源类型枚举）、
//!           `Asset`（资源元数据）。这些类型是整个引擎资源系统的基础，
//!           被 `aster-asset`（资源加载/缓存）、`aster-compiler`（编译期资源引用）、
//!           `aster-vm`（运行期资源命令）等模块使用。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//! - std::collections::HashMap（资源元数据存储）
//! - std::path::PathBuf（资源文件路径）
//!
//! 对应文档：Architecture.md §4.2（核心类型清单）
//!           任务：PH1-T03 — 实现 aster-core 资源与变量类型

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 资源唯一标识符 — newtype 包装的 `u64`。
///
/// 采用 newtype 模式而非裸 `u64`，确保类型安全：不会将 `AssetId` 与
/// 其他 `u64` 值（如偏移量、计数器）混淆。
///
/// # ID 分配策略
///
/// 按类型分段分配 ID 号段，便于从 ID 快速推断资源类型：
///
/// | 号段 | 类型 |
/// |------|------|
/// | `0x0000_0000` ~ `0x0FFF_FFFF` | Background（背景图片） |
/// | `0x1000_0000` ~ `0x1FFF_FFFF` | CharacterSprite（角色立绘） |
/// | `0x2000_0000` ~ `0x2FFF_FFFF` | Bgm（背景音乐） |
/// | `0x3000_0000` ~ `0x3FFF_FFFF` | Se（音效） |
/// | `0x4000_0000` ~ `0x4FFF_FFFF` | Voice（语音） |
/// | `0x5000_0000` ~ `0x5FFF_FFFF` | Font（字体） |
/// | `0x6000_0000` ~ `0x6FFF_FFFF` | Video（视频） |
/// | `0x7000_0000` ~ `0x7FFF_FFFF` | GuiElement（GUI 元素） |
///
/// # 示例
/// ```
/// use aster_core::AssetId;
/// use std::collections::HashMap;
///
/// let mut map = HashMap::new();
/// map.insert(AssetId(1), "test");
/// assert_eq!(map.get(&AssetId(1)), Some(&"test"));
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AssetId(pub u64);

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetId({})", self.0)
    }
}

/// 资源类型枚举 — 定义引擎支持的全部资源类别。
///
/// 每个 variant 对应 `assets/` 下的一个子目录，方便资源扫描器
/// 按类型遍历文件系统。
///
/// # 资源目录映射
///
/// | Variant | 子目录 | 典型文件格式 |
/// |---------|--------|-------------|
/// | `Background` | `assets/bg/` | PNG, WebP, JPG |
/// | `CharacterSprite` | `assets/char/` | PNG, WebP（含 Alpha 通道） |
/// | `Bgm` | `assets/bgm/` | OGG, MP3, WAV |
/// | `Se` | `assets/se/` | OGG, WAV |
/// | `Voice` | `assets/voice/` | OGG |
/// | `Font` | `assets/font/` | TTF, OTF |
/// | `Video` | `assets/video/` | WebM, MP4 |
/// | `GuiElement` | `assets/gui/` | PNG, WebP |
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetType {
    /// 背景图片 — 存放在 `assets/bg/` 目录
    Background,
    /// 角色立绘精灵 — 存放在 `assets/char/` 目录，含 Alpha 透明通道
    CharacterSprite,
    /// 背景音乐 — 存放在 `assets/bgm/` 目录
    Bgm,
    /// 音效 — 存放在 `assets/se/` 目录
    Se,
    /// 语音/配音 — 存放在 `assets/voice/` 目录
    Voice,
    /// 字体文件 — 存放在 `assets/font/` 目录
    Font,
    /// 视频文件 — 存放在 `assets/video/` 目录（Phase 5 使用）
    Video,
    /// GUI 元素（按钮、面板等）— 存放在 `assets/gui/` 目录（Phase 5 使用）
    GuiElement,
}

impl AssetType {
    /// 返回该资源类型在 `assets/` 下的子目录名。
    ///
    /// # 示例
    /// ```
    /// use aster_core::AssetType;
    ///
    /// assert_eq!(AssetType::Background.dir_name(), "bg");
    /// assert_eq!(AssetType::CharacterSprite.dir_name(), "char");
    /// ```
    pub fn dir_name(&self) -> &'static str {
        match self {
            AssetType::Background => "bg",
            AssetType::CharacterSprite => "char",
            AssetType::Bgm => "bgm",
            AssetType::Se => "se",
            AssetType::Voice => "voice",
            AssetType::Font => "font",
            AssetType::Video => "video",
            AssetType::GuiElement => "gui",
        }
    }
}

/// 资源元数据 — 描述单个游戏资源的标识、类型和位置。
///
/// 不包含资源的实际数据（纹理像素、音频采样等）；实际数据由
/// `aster-asset`（Phase 2）中的 `AssetManager` 负责加载和缓存。
///
/// # 字段
/// - `id`：资源唯一标识符（由资源注册表分配）
/// - `asset_type`：资源类别（决定加载器选择）
/// - `path`：相对于项目 `assets/` 目录的文件路径
/// - `metadata`：可选的扩展元数据（如 `{"width": "1920", "height": "1080"}`）
///
/// # 示例
/// ```
/// use aster_core::{Asset, AssetId, AssetType};
/// use std::collections::HashMap;
/// use std::path::PathBuf;
///
/// let asset = Asset {
///     id: AssetId(1),
///     asset_type: AssetType::Background,
///     path: PathBuf::from("bg/school_classroom.png"),
///     metadata: HashMap::new(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    /// 资源唯一标识符
    pub id: AssetId,
    /// 资源类型（背景/立绘/音乐/音效/语音/字体/视频/GUI 元素）
    pub asset_type: AssetType,
    /// 相对于项目 `assets/` 目录的文件路径
    pub path: PathBuf,
    /// 可扩展的元数据键值对
    /// 例如 `{"width": "1920", "height": "1080", "duration_ms": "3000"}`
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── AC01: AssetId 可用作 HashMap key ───────────────────────────────────

    /// AC01 — `AssetId` newtype 可用作 HashMap key。
    ///
    /// 验证：
    /// 1. 插入后查询返回正确的值
    /// 2. 查询不存在的 key 返回 None
    /// 3. Copy 语义：赋值后原变量仍可用
    #[test]
    fn ac01_assetid_as_hashmap_key() {
        let mut m = HashMap::new();
        let id1 = AssetId(1);
        let id2 = AssetId(2);

        m.insert(id1, "background");
        m.insert(id2, "character");

        // 通过值查询（Copy 语义）
        assert_eq!(m.get(&AssetId(1)), Some(&"background"));
        assert_eq!(m.get(&AssetId(2)), Some(&"character"));
        assert_eq!(m.get(&AssetId(999)), None);

        // Copy 语义验证：id1 被 insert 后仍可用
        assert_eq!(id1, AssetId(1));
        assert_eq!(m.get(&id1), Some(&"background"));
    }

    /// 验证 AssetId 的 Ord 排序正确（从小到大按 u64 值排序）。
    #[test]
    fn assetid_ordering() {
        let mut ids = vec![
            AssetId(100),
            AssetId(1),
            AssetId(50),
            AssetId(200),
            AssetId(0),
        ];
        ids.sort();
        assert_eq!(
            ids,
            vec![
                AssetId(0),
                AssetId(1),
                AssetId(50),
                AssetId(100),
                AssetId(200),
            ]
        );
    }

    /// 验证 AssetId 的 Display 实现输出正确的格式。
    #[test]
    fn assetid_display() {
        assert_eq!(format!("{}", AssetId(42)), "AssetId(42)");
        assert_eq!(format!("{}", AssetId(0)), "AssetId(0)");
        assert_eq!(format!("{}", AssetId(0x1000_0000)), "AssetId(268435456)");
    }

    /// 验证 AssetId 的 Serialize/Deserialize round-trip。
    #[test]
    fn assetid_serde_roundtrip() {
        let id = AssetId(42);
        let json = serde_json::to_string(&id).expect("JSON 序列化失败");
        let restored: AssetId = serde_json::from_str(&json).expect("JSON 反序列化失败");
        assert_eq!(restored, AssetId(42));
    }

    // ─── AssetType 测试 ────────────────────────────────────────────────────

    /// 验证 AssetType 每个 variant 的 dir_name() 返回正确子目录名。
    #[test]
    fn assettype_dir_names() {
        assert_eq!(AssetType::Background.dir_name(), "bg");
        assert_eq!(AssetType::CharacterSprite.dir_name(), "char");
        assert_eq!(AssetType::Bgm.dir_name(), "bgm");
        assert_eq!(AssetType::Se.dir_name(), "se");
        assert_eq!(AssetType::Voice.dir_name(), "voice");
        assert_eq!(AssetType::Font.dir_name(), "font");
        assert_eq!(AssetType::Video.dir_name(), "video");
        assert_eq!(AssetType::GuiElement.dir_name(), "gui");
    }

    /// 验证 AssetType 的 JSON 序列化 round-trip。
    #[test]
    fn assettype_serde_roundtrip() {
        let types = vec![
            AssetType::Background,
            AssetType::CharacterSprite,
            AssetType::Bgm,
            AssetType::Se,
            AssetType::Voice,
            AssetType::Font,
            AssetType::Video,
            AssetType::GuiElement,
        ];

        for at in &types {
            let json =
                serde_json::to_string(at).unwrap_or_else(|_| panic!("{:?} JSON 序列化失败", at));
            let restored: AssetType = serde_json::from_str(&json)
                .unwrap_or_else(|_| panic!("{:?} JSON 反序列化失败", at));
            assert_eq!(&restored, at);
        }
    }

    /// 验证 AssetType 的 PartialEq + Hash 正确性（用作 HashMap key）。
    #[test]
    fn assettype_as_hashmap_key() {
        let mut m = HashMap::new();
        m.insert(AssetType::Background, "background_files");
        m.insert(AssetType::Bgm, "music_files");
        assert_eq!(m.get(&AssetType::Background), Some(&"background_files"));
        assert_eq!(m.get(&AssetType::Bgm), Some(&"music_files"));
        assert_eq!(m.get(&AssetType::Se), None);
    }

    // ─── Asset 测试 ───────────────────────────────────────────────────────

    /// 验证 Asset 结构体的 JSON 序列化 round-trip。
    #[test]
    fn asset_serde_json_roundtrip() {
        let mut metadata = HashMap::new();
        metadata.insert("width".into(), "1920".into());
        metadata.insert("height".into(), "1080".into());

        let asset = Asset {
            id: AssetId(1),
            asset_type: AssetType::Background,
            path: PathBuf::from("bg/school.png"),
            metadata,
        };

        let json = serde_json::to_string(&asset).expect("JSON 序列化失败");
        let restored: Asset = serde_json::from_str(&json).expect("JSON 反序列化失败");

        assert_eq!(restored.id, AssetId(1));
        assert_eq!(restored.asset_type, AssetType::Background);
        assert_eq!(restored.path, PathBuf::from("bg/school.png"));
        assert_eq!(restored.metadata.get("width"), Some(&"1920".to_string()));
        assert_eq!(restored.metadata.get("height"), Some(&"1080".to_string()));
    }

    /// 验证 Asset 默认 metadata（空 HashMap）可正常序列化。
    #[test]
    fn asset_with_empty_metadata() {
        let asset = Asset {
            id: AssetId(42),
            asset_type: AssetType::Bgm,
            path: PathBuf::from("bgm/theme.ogg"),
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&asset).expect("JSON 序列化失败");
        let restored: Asset = serde_json::from_str(&json).expect("JSON 反序列化失败");

        assert_eq!(restored.id, AssetId(42));
        assert_eq!(restored.asset_type, AssetType::Bgm);
        assert!(restored.metadata.is_empty());
    }

    /// 验证 Asset 结构体的 Clone 语义（深拷贝）。
    #[test]
    fn asset_clone_is_deep() {
        let mut metadata = HashMap::new();
        metadata.insert("key".into(), "value".into());

        let asset = Asset {
            id: AssetId(1),
            asset_type: AssetType::CharacterSprite,
            path: PathBuf::from("char/hero.png"),
            metadata,
        };

        let cloned = asset.clone();
        assert_eq!(cloned, asset);

        // 修改 clone 的 metadata 不影响原值
        // （由于测试结构中没有可变引用验证方式，通过断言分开的值来确认）
        let mut cloned2 = asset.clone();
        cloned2
            .metadata
            .insert("new_key".into(), "new_value".into());
        assert_ne!(cloned2.metadata.len(), asset.metadata.len());
    }

    // ─── 边界值测试 ─────────────────────────────────────────────────────

    /// 验证 AssetId(u64::MAX) 的 Display 实现。
    #[test]
    fn assetid_display_max_value() {
        let id = AssetId(u64::MAX);
        assert_eq!(format!("{}", id), format!("AssetId({})", u64::MAX));
    }

    /// 验证 AssetId(u64::MIN) 的 Display 实现。
    #[test]
    fn assetid_display_min_value() {
        let id = AssetId(0);
        assert_eq!(format!("{}", id), "AssetId(0)");
    }

    /// 验证 AssetId 的 Clone 语义（Copy + Clone 正确工作）。
    #[test]
    fn assetid_clone_is_copy() {
        let id = AssetId(42);
        let copied = id; // Copy 而非 move
        assert_eq!(id, AssetId(42));
        assert_eq!(copied, AssetId(42));
        // id 仍可使用
        let _ = format!("{}", id);
    }
}
