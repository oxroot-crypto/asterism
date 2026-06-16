//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-asset/src/lib.rs
//! 功能概述：资源管理 crate — 统一管理游戏资源的加载、索引和生命周期。
//!           提供 `AssetManager`（资源索引/扫描/加载中枢）、
//!           `AssetLoader` trait（可扩展的资源加载接口）、
//!           `TextureLoader`（PNG/WebP→GPU 纹理）和
//!           `AudioLoader`（OGG/FLAC/MP3/WAV→PCM 样本）。
//!           本 crate 是 PH2-T04 的核心交付物，为 PH2-T05（LRU 缓存）
//!           和 PH2-T08（运行时集成）提供基础设施。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core（AssetId、AssetType 核心资源类型）
//! - wgpu（GPU 纹理创建）
//! - image（PNG/WebP/JPEG 图片解码）
//! - symphonia（OGG/FLAC/MP3/WAV 音频解码）
//! - lru（LRU 缓存，PH2-T05 使用）
//!
//! 架构位置：aster-core ← aster-asset
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `asset_manager` | `asset_manager.rs` | AssetManager：扫描/索引/查询/加载中枢 + LRU 缓存 |
//! | `loader` | `loader.rs` | AssetLoader trait + TextureLoader + AudioLoader |
//! | `error` | `error.rs` | AssetError：NotFound/UnsupportedFormat/DecodeError/Io |
//! | `cache` | `cache.rs` | CachedAsset + CacheStats + 内存估算（PH2-T05） |
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use aster_asset::{AssetManager, TextureLoader, AudioLoader, AssetError};
//! use std::sync::Arc;
//!
//! fn init_assets(project_root: &str, device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>)
//!     -> Result<AssetManager, AssetError>
//! {
//!     let mut manager = AssetManager::new(project_root);
//!     manager.scan_assets()?;
//!     manager.register_loader(Arc::new(TextureLoader::new(device, queue)));
//!     manager.register_loader(Arc::new(AudioLoader::new()));
//!     Ok(manager)
//! }
//! ```

// 模块声明
pub mod asset_manager;
pub mod cache;
pub mod error;
pub mod loader;

// 重导出所有公开类型，方便外部 crate 通过 `aster_asset::TypeName` 直接引用
pub use asset_manager::{AssetManager, AssetMetadata};
pub use cache::{CacheStats, CachedAsset};
pub use error::AssetError;
pub use loader::{AssetLoader, AudioLoader, LoadedAsset, TextureLoader};
