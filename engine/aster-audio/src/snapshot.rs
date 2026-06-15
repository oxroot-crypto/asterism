//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-audio/src/snapshot.rs
//! 功能概述：音频状态快照 — `AudioSnapshot` 结构体捕获音频系统的完整运行时状态，
//!           包括 BGM 播放信息（路径、位置、循环、音量）和 SE 音量。
//!           支持 serde 序列化/反序列化，为存档系统（PH2-T06）提供音频状态恢复能力。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - serde（序列化/反序列化 derive）

use serde::{Deserialize, Serialize};

/// 音频系统状态快照 — 捕获某一时刻 AudioSystem 的完整播放状态。
///
/// 该结构体是存档数据（`SaveData`）的组成部分，用于在读档时恢复音频系统
/// 到与存档时完全一致的状态。快照使用 `String` 路径而非 `AssetId`，
/// 以确保存档文件的独立性和跨版本兼容性。
///
/// # 序列化格式
///
/// 通过 serde 支持 JSON / MessagePack 等多种格式：
/// ```rust,no_run
/// # use aster_audio::AudioSnapshot;
/// let snapshot = AudioSnapshot::default();
/// let json = serde_json::to_string(&snapshot).unwrap();
/// let restored: AudioSnapshot = serde_json::from_str(&json).unwrap();
/// ```
///
/// # 字段说明
///
/// | 字段 | 类型 | 说明 |
/// |------|------|------|
/// | `current_bgm_path` | `Option<String>` | 当前 BGM 文件路径，`None` 表示无 BGM 播放 |
/// | `bgm_position_secs` | `f64` | BGM 播放位置（秒），用于恢复时 seek |
/// | `bgm_looping` | `bool` | BGM 是否循环播放 |
/// | `bgm_volume` | `f32` | BGM 通道音量（0.0 ~ 1.0） |
/// | `se_volume` | `f32` | SE 通道音量（0.0 ~ 1.0） |
///
/// # 已知限制
///
/// - BGM 位置精度取决于音频编码格式，VBR 编码的 OGG 文件 seek 精度约 ±50ms，
///   对视觉小说存档体验无实质性影响
/// - 快照不包含 SE 播放队列（SE 是瞬时音效，存档时不应有正在播放的 SE）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSnapshot {
    /// 当前播放的 BGM 资源路径（None = 无 BGM 播放）
    pub current_bgm_path: Option<String>,
    /// BGM 播放位置（秒，用于恢复时 seek）
    pub bgm_position_secs: f64,
    /// BGM 是否循环播放
    pub bgm_looping: bool,
    /// BGM 通道音量（0.0 ~ 1.0）
    pub bgm_volume: f32,
    /// SE 通道音量（0.0 ~ 1.0）
    pub se_volume: f32,
}

impl Default for AudioSnapshot {
    /// 创建默认的音频快照 — 无 BGM 播放，音量为默认值 0.8。
    ///
    /// 此默认值对应 AudioSystem 初始化后的状态。
    fn default() -> Self {
        Self {
            current_bgm_path: None,
            bgm_position_secs: 0.0,
            bgm_looping: false,
            bgm_volume: 0.8,
            se_volume: 0.8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AudioSnapshot 默认值对应无播放状态
    #[test]
    fn test_default_snapshot() {
        let snapshot = AudioSnapshot::default();
        assert!(snapshot.current_bgm_path.is_none());
        assert!((snapshot.bgm_position_secs - 0.0).abs() < f64::EPSILON);
        assert!(!snapshot.bgm_looping);
        assert!((snapshot.bgm_volume - 0.8).abs() < f32::EPSILON);
        assert!((snapshot.se_volume - 0.8).abs() < f32::EPSILON);
    }

    /// AudioSnapshot 序列化/反序列化往返（JSON）
    #[test]
    fn test_snapshot_serde_roundtrip_json() {
        let original = AudioSnapshot {
            current_bgm_path: Some("assets/bgm/theme.ogg".to_string()),
            bgm_position_secs: 42.5,
            bgm_looping: true,
            bgm_volume: 0.7,
            se_volume: 0.3,
        };

        let json = serde_json::to_string(&original).expect("序列化应成功");
        let restored: AudioSnapshot = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(original, restored, "往返后字段应一致");
    }

    /// AudioSnapshot 序列化/反序列化 — 无 BGM 快照
    #[test]
    fn test_snapshot_serde_roundtrip_no_bgm() {
        let original = AudioSnapshot {
            current_bgm_path: None,
            bgm_position_secs: 0.0,
            bgm_looping: false,
            bgm_volume: 0.5,
            se_volume: 1.0,
        };

        let json = serde_json::to_string(&original).expect("序列化应成功");
        let restored: AudioSnapshot = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(original, restored, "空 BGM 快照往返后应一致");
    }

    /// AudioSnapshot Clone 行为正确
    #[test]
    fn test_snapshot_clone() {
        let original = AudioSnapshot {
            current_bgm_path: Some("test.ogg".into()),
            bgm_position_secs: 10.0,
            bgm_looping: true,
            bgm_volume: 0.9,
            se_volume: 0.1,
        };

        let cloned = original.clone();
        assert_eq!(original, cloned, "clone 后字段应一致");
    }
}
