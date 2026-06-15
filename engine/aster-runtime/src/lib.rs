//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/lib.rs
//! 功能概述：运行时集成 — 集成所有 engine 子系统（渲染/音频/VM/UI/存档），
//!           提供统一的游戏启动、主循环、配置管理和生命周期控制。
//!           对外暴露 `AsterRuntime` 作为引擎的唯一入口点。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster-core（核心数据类型：Game / Character / BuildConfig / Scene 等）
//! - aster-compiler（编译产物：CompiledGame / CompiledScene）
//! - aster-platform / aster-renderer / aster-audio / aster-vm / aster-ui / aster-save（待 Phase 3 集成）
//!
//! 架构位置：依赖所有下层 crate（Architecture.md §4 分层图的顶层）
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `error` | `error.rs` | 运行时错误类型：`RuntimeError`（IO/TOML/项目验证/角色解析错误） |
//! | `game_context` | `game_context.rs` | 游戏上下文：`GameContext` — 持有 CompiledGame + 角色表 + 跨场景导航 |
//! | `game_loader` | `game_loader.rs` | 游戏清单加载器：`GameLoader::load()` → `GameManifest` |
//! | `game_manifest` | `game_manifest.rs` | 游戏清单类型：`GameManifest` / `SceneEntry` |
//!
//! ## 待后续任务实现
//!
//! - **PH1-T18**：`SceneManager` — 场景状态机 + VM Action→Renderer 桥接
//! - **Phase 3**：`AsterRuntime` — 引擎运行时主结构 + `run()` 入口

// 模块声明
pub mod error;
pub mod game_context;
pub mod game_loader;
pub mod game_manifest;

// 重导出所有公开类型，方便外部 crate 通过 `aster_runtime::TypeName` 直接引用
pub use error::RuntimeError;
pub use game_context::GameContext;
pub use game_loader::GameLoader;
pub use game_manifest::{GameManifest, SceneEntry};
