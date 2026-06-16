//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/lib.rs
//! 功能概述：核心数据类型 — 定义整个引擎共享的基础数据结构：
//!           `Game`（游戏元数据）/ `Character`（角色定义）/ `Scene`（场景定义）/
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
//! | `asset` | `asset.rs` | 资源类型：`AssetId`（newtype）、`AssetType`（8 种资源类别）、`Asset`（资源元数据） |
//! | `build_config` | `build_config.rs` | 构建配置：`BuildConfig`、`CompileConfig`、`GlobPatterns`、`ArchiveConfig` |
//! | `character` | `character.rs` | 角色定义：`Character`（id/name/display_color/description/birthday/default_position/sprites/voice）、`VoiceConfig` |
//! | `expr` | `expr.rs` | 表达式类型：`Expr`（7 种 AST 节点）、`BinaryOp`（12 种二元运算符）、`UnaryOp`（2 种一元运算符），parser 和 compiler 共享 |
//! | `game` | `game.rs` | 游戏元数据：`Game`、`Resolution`、`GameSettings`、`TextSpeed` |
//! | `scene` | `scene.rs` | 场景定义：`Scene`、`SceneNode`（25 种变体）、`Choice`、`Position`、`TransitionSpec` |
//! | `variable` | `variable.rs` | 变量系统：`VariableStore`（变量表）、`Value`（6 种值类型）、`FlagSet`（旗标集合） |
//!
//! ## 待后续任务实现
//!
//! - **Phase 2**：`SaveData`（`save.rs`）、`Theme`（`theme.rs`）

// 模块声明
pub mod asset;
pub mod build_config;
pub mod character;
pub mod expr;
pub mod game;
pub mod save;
pub mod scene;
pub mod variable;

// 重导出所有公开类型，方便外部 crate 通过 `aster_core::TypeName` 直接引用
pub use asset::{Asset, AssetId, AssetType};
pub use build_config::{ArchiveConfig, BuildConfig, CompileConfig, GlobPatterns};
pub use character::{Character, VoiceConfig};
pub use expr::{BinaryOp, Expr, UnaryOp};
pub use game::{Game, GameSettings, Resolution, TextSpeed};
pub use save::{AudioSnapshot, RenderState, SaveData, SaveSlotInfo, SpriteState, VmSnapshot};
pub use scene::{Choice, Position, Scene, SceneNode, TransitionSpec};
pub use variable::{FlagSet, Value, VariableStore};
