//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-audio/src/snapshot.rs
//! 功能概述：音频状态快照 — 重新导出 `aster_core::save::AudioSnapshot`。
//!           PH2-T08 统一后，AudioSnapshot 统一定义在 `aster-core` 中，
//!           本模块仅作为向后兼容的重新导出，避免破坏现有 `use aster_audio::AudioSnapshot` 的引用。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core::save（AudioSnapshot 的正统定义位置）

// PH2-T08: 重新导出 aster_core::save::AudioSnapshot
// 移除 aster-audio 中的重复定义，统一使用 aster-core 中的版本。
// 保留 snapshot 模块以维持向后兼容性。
pub use aster_core::save::AudioSnapshot;

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
