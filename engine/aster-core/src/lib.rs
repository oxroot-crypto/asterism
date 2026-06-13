//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/lib.rs
//! 功能概述：核心数据类型 — 定义整个引擎共享的基础数据结构：
//!           `Project`（项目元数据）/ `Character`（角色定义）/ `Scene`（场景定义）/
//!           `SceneNode`（演出单元枚举）/ `Choice`（选择支）/ `Position`（立绘位置）等。
//!           本 crate 不依赖任何其他 engine crate（Architecture.md §4.2），
//!           是整个引擎类型系统的基石。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//!
//! 架构位置：aster-platform ← aster-core ← aster-parser/compiler/vm/...
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `project` | `project.rs` | 项目元数据：`Project`、`Resolution`、`ProjectSettings`、`TextSpeed` |
//! | `character` | `character.rs` | 角色定义：`Character`（id/name/display_color/sprites/voice_prefix） |
//! | `scene` | `scene.rs` | 场景定义：`Scene`、`SceneNode`（25 种变体）、`Choice`、`Position`、`TransitionSpec` |
//!
//! ## 待后续任务实现
//!
//! - **PH1-T03**：`Asset`、`AssetId`、`AssetType`（`asset.rs`）、`VariableStore`、`Value`、`FlagSet`（`variable.rs`）
//! - **Phase 2**：`SaveData`（`save.rs`）、`Theme`（`theme.rs`）

// 模块声明
pub mod character;
pub mod project;
pub mod scene;

// 重导出所有公开类型，方便外部 crate 通过 `aster_core::TypeName` 直接引用
pub use character::Character;
pub use project::{Project, ProjectSettings, Resolution, TextSpeed};
pub use scene::{Choice, Position, Scene, SceneNode, TransitionSpec};
