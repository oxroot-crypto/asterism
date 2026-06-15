//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-audio/src/lib.rs
//! 功能概述：音频系统 — 管理游戏音频播放，支持 BGM（背景音乐）和
//!           SE（音效）的加载、播放、停止、循环和独立音量控制。
//!           BGM/SE 通过 kira 子轨道（TrackHandle）隔离混音，互不干扰。
//!           后端基于 kira（跨平台游戏音频库，通过 cpal 与系统音频设备交互）。
//!           功能规划：
//!           - Phase 2-T01 ✅ BGM 播放（当前实现）
//!           - Phase 2-T02 ✅ SE 音效播放 + 多通道混音（当前实现）
//!           - Phase 2-T03 🔲 fade_in/fade_out + 音频状态快照
//!           - Phase 4    🔲 Voice 角色语音通道
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - kira（音频引擎后端：AudioManager / TrackBuilder / TrackHandle / StaticSoundData）
//! - symphonia（音频格式解码：OGG / FLAC / MP3 / WAV）
//! - aster_core（AssetId 资源引用、GameSettings 默认音量）
//!
//! 架构位置：aster-platform/aster-core ← aster-audio
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `audio_system` | `audio_system.rs` | `AudioSystem` 结构体 — BGM/SE 播放/停止/循环/音量控制 |
//! | `error` | `error.rs` | `AudioError` 枚举 — 资源/解码/播放/IO 四类错误 |

// 模块声明
pub mod audio_system;
pub mod error;

// 重导出公开类型
pub use audio_system::AudioSystem;
pub use error::AudioError;
