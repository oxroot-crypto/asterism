//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/project.rs
//! 功能概述：项目元数据类型 — 定义 `Project` 结构体，对应 `project.toml` 的 `[project]` section。
//!           包含项目名称、版本、入口场景、分辨率、默认设置等元数据。
//!           所有类型均派生 `Debug + Clone + Serialize + Deserialize`。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持，用于 TOML/JSON 读写）
//!
//! 对应文档：Architecture.md §4.2（核心类型清单）、§5.2（project.toml 格式）

use serde::{Deserialize, Serialize};

/// 游戏项目元数据 — 对应 `project.toml` 中的 `[project]` section。
///
/// 包含项目的基本信息（名称、版本）、入口场景、设计分辨率和默认设置。
/// `characters` 和 `scenes` 列表由引擎运行时从 `characters/` 和 `scripts/`
/// 目录自动扫描生成，不存储在 `Project` 结构体中。
///
/// # 序列化
///
/// 通过 serde 派生支持 TOML 序列化/反序列化：
/// ```rust,ignore
/// let project: Project = toml::from_str(&fs::read_to_string("project.toml")?)?;
/// ```
///
/// # 示例
/// ```rust,ignore
/// use aster_core::Project;
///
/// let project = Project {
///     name: "My First Visual Novel".into(),
///     version: "0.1.0".into(),
///     entry_scene: "prologue".into(),
///     resolution: Resolution { width: 1920, height: 1080 },
///     settings: ProjectSettings::default(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    /// 项目名称（显示在窗口标题和关于对话框中）
    pub name: String,

    /// 语义化版本号（遵循 SemVer 2.0 规范）
    pub version: String,

    /// 入口场景 ID（对应 `scripts/` 下的 .aster 文件名，不含扩展名）
    /// 引擎启动时首先加载并执行此场景
    pub entry_scene: String,

    /// 游戏设计分辨率（逻辑像素），引擎自动适配实际窗口大小
    #[serde(default = "Resolution::default")]
    pub resolution: Resolution,

    /// 项目全局默认设置（语言、文字速度、音量等）
    #[serde(default = "ProjectSettings::default")]
    pub settings: ProjectSettings,
}

/// 游戏设计分辨率 — 定义画布的逻辑像素尺寸。
///
/// 引擎以此分辨率为基准进行布局计算和资源缩放，
/// 实际窗口大小变化时引擎自动进行等比缩放适配。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resolution {
    /// 宽度（逻辑像素），默认 1920
    #[serde(default = "default_width")]
    pub width: u32,

    /// 高度（逻辑像素），默认 1080
    #[serde(default = "default_height")]
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
        }
    }
}

/// 默认分辨率宽度 — 1920（Full HD）
const fn default_width() -> u32 {
    1920
}

/// 默认分辨率高度 — 1080（Full HD）
const fn default_height() -> u32 {
    1080
}

/// 项目全局设置 — 对应 `project.toml` 中的 `[project.settings]` section。
///
/// 包含语言偏好、文字显示速度和默认音量等可配置项。
/// 这些设置在游戏运行时可被玩家覆盖，此处定义的是首次启动时的默认值。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    /// 默认语言（BCP 47 语言标签，如 `"zh-CN"`、`"en-US"`、`"ja-JP"`）
    /// 多语言支持将在 v1.0.0 完整实现
    #[serde(default = "default_language")]
    pub language: String,

    /// 默认文字显示速度 — 控制打字机效果的逐字推进速率
    #[serde(default)]
    pub text_speed: TextSpeed,

    /// 默认 BGM 音量（0.0 ~ 1.0），默认 0.8
    #[serde(default = "default_bgm_volume")]
    pub default_bgm_volume: f32,

    /// 默认音效音量（0.0 ~ 1.0），默认 1.0
    #[serde(default = "default_se_volume")]
    pub default_se_volume: f32,

    /// 默认语音音量（0.0 ~ 1.0），默认 1.0
    #[serde(default = "default_voice_volume")]
    pub default_voice_volume: f32,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            language: default_language(),
            text_speed: TextSpeed::default(),
            default_bgm_volume: default_bgm_volume(),
            default_se_volume: default_se_volume(),
            default_voice_volume: default_voice_volume(),
        }
    }
}

/// 默认语言标签 — `"zh-CN"`（简体中文）
fn default_language() -> String {
    "zh-CN".to_string()
}

/// 默认 BGM 音量 — 0.8（80%）
const fn default_bgm_volume() -> f32 {
    0.8
}

/// 默认音效音量 — 1.0（100%）
const fn default_se_volume() -> f32 {
    1.0
}

/// 默认语音音量 — 1.0（100%）
const fn default_voice_volume() -> f32 {
    1.0
}

/// 文字显示速度 — 控制打字机效果中每个字符的显示间隔。
///
/// 对应 `project.toml` 中 `text_speed` 字段。
/// 预设四种速度档位（instant/slow/normal/fast），
/// `Custom(f32)` 变体允许创作者指定任意 ms/char 速率（Phase 1+ 功能）。
///
/// # 序列化格式
///
/// | 变体 | 序列化格式 | 示例 |
/// |------|-----------|------|
/// | `Instant` | 字符串 `"instant"` | `text_speed = "instant"` |
/// | `Slow` | 字符串 `"slow"` | `text_speed = "slow"` |
/// | `Normal` | 字符串 `"normal"` | `text_speed = "normal"` |
/// | `Fast` | 字符串 `"fast"` | `text_speed = "fast"` |
/// | `Custom(ms)` | 数值 | `text_speed = 25.0` |
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TextSpeed {
    /// 瞬间完成 — 所有文字立刻全部显示（等同于跳过打字机效果）
    Instant,

    /// 慢速 — 约 50ms/字符，适合注重氛围的叙事
    Slow,

    /// 正常速度 — 约 30ms/字符，适合大多数对话场景
    #[default]
    Normal,

    /// 快速 — 约 15ms/字符，适合快节奏对话
    Fast,

    /// 自定义速度 — 以 ms/char 为单位的精确速率
    /// 例如 `Custom(25.0)` 表示每 25ms 显示一个字符
    Custom(f32),
}

// TextSpeed 的自定义 Serialize 实现
// 处理混合类型：字符串变体序列化为字符串，Custom 序列化为数值
impl Serialize for TextSpeed {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TextSpeed::Instant => serializer.serialize_str("instant"),
            TextSpeed::Slow => serializer.serialize_str("slow"),
            TextSpeed::Normal => serializer.serialize_str("normal"),
            TextSpeed::Fast => serializer.serialize_str("fast"),
            TextSpeed::Custom(ms) => serializer.serialize_f32(*ms),
        }
    }
}

// TextSpeed 的自定义 Deserialize 实现
// 使用 Visitor 模式同时支持字符串（预设速度）和数值（自定义速度）的反序列化
impl<'de> Deserialize<'de> for TextSpeed {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        struct TextSpeedVisitor;

        impl<'de> de::Visitor<'de> for TextSpeedVisitor {
            type Value = TextSpeed;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "一个速度字符串 (\"instant\", \"slow\", \"normal\", \"fast\") 或一个数值 (自定义 ms/char)",
                )
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<TextSpeed, E> {
                match v {
                    "instant" => Ok(TextSpeed::Instant),
                    "slow" => Ok(TextSpeed::Slow),
                    "normal" => Ok(TextSpeed::Normal),
                    "fast" => Ok(TextSpeed::Fast),
                    other => Err(E::custom(format!(
                        "未知的文字速度: \"{other}\"，有效值为 instant/slow/normal/fast 或数值"
                    ))),
                }
            }

            fn visit_f32<E: de::Error>(self, v: f32) -> Result<TextSpeed, E> {
                Ok(TextSpeed::Custom(v))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<TextSpeed, E> {
                Ok(TextSpeed::Custom(v as f32))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<TextSpeed, E> {
                Ok(TextSpeed::Custom(v as f32))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<TextSpeed, E> {
                Ok(TextSpeed::Custom(v as f32))
            }
        }

        deserializer.deserialize_any(TextSpeedVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC01 — `Project` 结构体可正确序列化为 TOML 并反序列化回来
    ///
    /// 验证完整的 Project → TOML 字符串 → Project 的 round-trip，
    /// 确保所有字段（包括嵌套结构 Resolution 和 ProjectSettings）序列化/反序列化一致。
    #[test]
    fn ac01_project_toml_roundtrip() {
        let project = Project {
            name: "Test Visual Novel".into(),
            version: "1.2.3".into(),
            entry_scene: "prologue".into(),
            resolution: Resolution {
                width: 1280,
                height: 720,
            },
            settings: ProjectSettings {
                language: "ja-JP".into(),
                text_speed: TextSpeed::Slow,
                default_bgm_volume: 0.5,
                default_se_volume: 0.7,
                default_voice_volume: 0.9,
            },
        };

        // 序列化为 TOML
        let toml_str = toml::to_string(&project).expect("序列化为 TOML 失败");

        // 验证 TOML 输出包含关键字段
        assert!(
            toml_str.contains("Test Visual Novel"),
            "TOML 应包含项目名称"
        );
        assert!(toml_str.contains("1.2.3"), "TOML 应包含版本号");
        assert!(toml_str.contains("prologue"), "TOML 应包含入口场景");

        // 反序列化回来
        let restored: Project = toml::from_str(&toml_str).expect("从 TOML 反序列化失败");

        // 断言 round-trip 一致
        assert_eq!(restored.name, project.name);
        assert_eq!(restored.version, project.version);
        assert_eq!(restored.entry_scene, project.entry_scene);
        assert_eq!(restored.resolution.width, project.resolution.width);
        assert_eq!(restored.resolution.height, project.resolution.height);
        assert_eq!(restored.settings.language, project.settings.language);
        assert_eq!(restored.settings.text_speed, project.settings.text_speed);
        assert_eq!(
            restored.settings.default_bgm_volume,
            project.settings.default_bgm_volume
        );
        assert_eq!(
            restored.settings.default_se_volume,
            project.settings.default_se_volume
        );
        assert_eq!(
            restored.settings.default_voice_volume,
            project.settings.default_voice_volume
        );
    }

    /// AC01 补充 — 验证 Default 实现产生合理的值
    #[test]
    fn ac01_project_settings_default_values() {
        let settings = ProjectSettings::default();
        assert_eq!(settings.language, "zh-CN");
        assert_eq!(settings.text_speed, TextSpeed::Normal);
        // 比较浮点数（允许误差）
        assert!((settings.default_bgm_volume - 0.8).abs() < f32::EPSILON);
        assert!((settings.default_se_volume - 1.0).abs() < f32::EPSILON);
        assert!((settings.default_voice_volume - 1.0).abs() < f32::EPSILON);
    }

    /// AC01 补充 — 验证 Resolution 默认值为 1920×1080
    #[test]
    fn ac01_resolution_default_values() {
        let resolution = Resolution::default();
        assert_eq!(resolution.width, 1920);
        assert_eq!(resolution.height, 1080);
    }

    /// 验证 TextSpeed 在结构体中的 TOML 序列化/反序列化正确
    ///
    /// TOML 不支持顶层枚举序列化，TextSpeed 始终作为 ProjectSettings
    /// 或其他结构体的字段使用。本测试验证在实际使用场景中的 round-trip。
    #[test]
    fn text_speed_toml_roundtrip_in_struct() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestWrapper {
            text_speed: TextSpeed,
        }

        let test_cases: Vec<(TextSpeed, &str)> = vec![
            (TextSpeed::Instant, "instant"),
            (TextSpeed::Slow, "slow"),
            (TextSpeed::Normal, "normal"),
            (TextSpeed::Fast, "fast"),
        ];

        for (speed, expected_str) in test_cases {
            let wrapper = TestWrapper {
                text_speed: speed.clone(),
            };
            let toml_str = toml::to_string(&wrapper).expect("TOML 序列化失败");

            // 字符串变体验证 TOML 输出包含引号包裹的速度值
            assert!(
                toml_str.contains(&format!("text_speed = \"{}\"", expected_str)),
                "TOML 输出应包含 text_speed = \"{expected_str}\"，实际输出: {toml_str}"
            );

            // 反序列化回来
            let restored: TestWrapper = toml::from_str(&toml_str).expect("TOML 反序列化失败");
            assert_eq!(restored.text_speed, speed);
        }

        // 测试 Custom 变体（数值序列化）
        let custom_wrapper = TestWrapper {
            text_speed: TextSpeed::Custom(25.0),
        };
        let custom_toml = toml::to_string(&custom_wrapper).expect("Custom TOML 序列化失败");
        // Custom 变体序列化为数值，不带引号
        assert!(
            custom_toml.contains("text_speed = 25.0"),
            "Custom TOML 输出应包含数值 25.0，实际输出: {custom_toml}"
        );

        let restored_custom: TestWrapper =
            toml::from_str(&custom_toml).expect("Custom TOML 反序列化失败");
        assert_eq!(restored_custom.text_speed, TextSpeed::Custom(25.0));
    }

    /// 验证 TextSpeed 通过 serde_json 的序列化（更宽容的格式）
    #[test]
    fn text_speed_json_roundtrip() {
        let cases = vec![
            TextSpeed::Instant,
            TextSpeed::Slow,
            TextSpeed::Normal,
            TextSpeed::Fast,
            TextSpeed::Custom(42.5),
            TextSpeed::Custom(0.0),
        ];

        for speed in cases {
            let json_str = serde_json::to_string(&speed).expect("JSON 序列化失败");
            let restored: TextSpeed = serde_json::from_str(&json_str).expect("JSON 反序列化失败");
            assert_eq!(restored, speed);
        }
    }

    /// 验证 Custom 变体的具体 JSON 表示
    #[test]
    fn text_speed_custom_json_format() {
        let speed = TextSpeed::Custom(33.3);
        let json_str = serde_json::to_string(&speed).expect("JSON 序列化失败");
        // Custom 应序列化为裸数值
        assert_eq!(json_str, "33.3");

        // 从数值反序列化
        let restored: TextSpeed = serde_json::from_str("25.0").expect("JSON 反序列化失败");
        assert_eq!(restored, TextSpeed::Custom(25.0));

        // 从整数反序列化
        let from_int: TextSpeed = serde_json::from_str("50").expect("JSON 反序列化失败");
        assert_eq!(from_int, TextSpeed::Custom(50.0));
    }
}
