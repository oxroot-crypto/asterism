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
//! - aster-vm（字节码虚拟机：Vm / VmAction / EngineCommand）
//! - aster-platform / aster-renderer / aster-audio / aster-ui / aster-save（待后续 Phase 集成）
//!
//! 架构位置：依赖所有下层 crate（Architecture.md §4 分层图的顶层）
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `error` | `error.rs` | 运行时错误类型：`RuntimeError`（IO/TOML/场景/状态错误） |
//! | `game_context` | `game_context.rs` | 游戏上下文：`GameContext` — 持有 CompiledGame + 角色表 + 跨场景导航 |
//! | `game_loader` | `game_loader.rs` | 游戏清单加载器：`GameLoader::load()` → `GameManifest` |
//! | `game_manifest` | `game_manifest.rs` | 游戏清单类型：`GameManifest` / `SceneEntry` |
//! | `command_bridge` | `command_bridge.rs` | 命令桥接器：`CommandBridge` — EngineCommand→Renderer trait 映射 |
//! | `scene_manager` | `scene_manager.rs` | 场景管理器：`SceneManager` — 场景状态机 + VM Action 分发 |
//!
//! ## 待后续任务实现
//!
//! - **PH1-T19**：`DialogueController` — 对话流管理 + 打字机状态控制
//! - ~~**PH1-T20**：`InputManager` — winit 事件→游戏动作映射~~ ✅ 已完成
//! - **Phase 3**：`AsterRuntime` — 引擎运行时主结构 + `run()` 入口

// 模块声明
pub mod command_bridge;
pub mod dialogue_controller;
pub mod error;
pub mod game_context;
pub mod game_loader;
pub mod game_manifest;
pub mod input_manager;
pub mod renderer_impl;
pub mod scene_manager;

// 重导出所有公开类型，方便外部 crate 通过 `aster_runtime::TypeName` 直接引用
pub use command_bridge::{MockRenderer, Renderer, dispatch};
pub use dialogue_controller::{DialogueAction, DialogueController, DialogueLine, DialogueState};
pub use error::RuntimeError;
pub use game_context::GameContext;
pub use game_loader::GameLoader;
pub use game_manifest::{GameManifest, SceneEntry};
pub use input_manager::{GameAction, InputManager};
pub use renderer_impl::GameRenderer;
pub use scene_manager::{SceneManager, SceneState};
