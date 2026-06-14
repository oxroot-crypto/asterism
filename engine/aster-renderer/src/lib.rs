//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/lib.rs
//! 功能概述：渲染器 — 基于 wgpu 的 GPU 渲染管线。
//!           负责：GPU 上下文管理（设备/表面/队列）/ 精灵批处理（sprite batching）/
//!           场景转场特效（fade/dissolve/wipe）/ 文本渲染（cosmic-text 集成）/
//!           后处理（post-processing）。
//!           目标：1080p@60fps（集成显卡），单帧 draw call ≤ 50。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - wgpu 24.x（GPU 抽象层，Vulkan/DX12/Metal 后端）
//! - winit 0.30.x（跨平台窗口创建与事件处理）
//! - aster_platform（Phase 2 添加）：平台服务
//! - aster_core（Phase 2 添加）：AssetId 纹理引用
//!
//! 架构位置：aster-platform/aster-core ← aster-renderer
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 职责 |
//! |------|------|------|
//! | `config` | `config.rs` | `RenderConfig` — 渲染器配置（分辨率/全屏/vsync/MSAA） |
//! | `gpu_context` | `gpu_context.rs` | `GpuContext` / `Frame` / `RenderError` — GPU 资源管理 |

// ============================================================================
// 模块声明
// ============================================================================

pub mod background_layer;
pub mod config;
pub mod gpu_context;
pub mod layer_manager;
pub mod sprite_layer;
pub mod texture;

// ============================================================================
// 公开导出 — 核心类型
// ============================================================================

pub use background_layer::{BackgroundLayer, FitMode};
pub use config::RenderConfig;
pub use gpu_context::{Frame, GpuContext, RenderError};
pub use layer_manager::{Layer, LayerManager};
pub use sprite_layer::{Sprite, SpriteDescriptor, SpriteLayer, SpritePosition};
pub use texture::{Texture, create_texture_bind_group_layout};
