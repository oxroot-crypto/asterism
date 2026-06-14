//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/src/lib.rs
//! 功能概述：.aster 编译器 — 将 AST（aster-parser 输出的 `aster_core::Scene`）
//!           编译为字节码（`CompiledScene`），供字节码虚拟机（aster-vm）执行。
//!           负责语义检查、变量解析、跳转目标绑定。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - aster_core：基础数据类型（Scene, SceneNode, Expr, Value 等）
//! - bincode：字节码二进制序列化
//! - serde：CompiledScene 的序列化/反序列化
//! - thiserror：CompileError 的 Error trait 派生
//!
//! 架构位置：aster-core ← aster-parser ← aster-compiler ← aster-vm
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `ir` | `ir.rs` | IR 类型定义：`IrInstruction`（46 变体）、`PositionEncoding`、`ChoiceData`、`RegisterAllocator` |
//! | `bytecode` | `bytecode.rs` | 字节码定义：`Opcode`（45 个操作码）、`CompiledScene`、IR↔字节码编解码 |
//! | `compiler` | `compiler.rs` | `Compiler`：Scene→IR→Bytecode 三步编译管线 |
//! | `error` | `error.rs` | `CompileError`：携带位置信息的中文编译错误 |
//!
//! ## 编译流程
//! ```text
//! aster_core::Scene → Compiler::compile()
//!   → Pass 0: 字符串收集 + 标签记录
//!   → Pass 1: IR 生成（25 种 SceneNode → 46 种 IrInstruction）
//!   → Pass 2: 字节码编码（IrInstruction → 变长字节码 + label_table 解析）
//!   → CompiledScene { version, instructions, constant_pool, label_table }
//! ```
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use aster_core::{Scene, SceneNode, Expr};
//! use aster_compiler::Compiler;
//!
//! fn compile_example() {
//!     let scene = Scene {
//!         id: "prologue".into(),
//!         label: None,
//!         background: None,
//!         music: None,
//!         nodes: vec![
//!             SceneNode::Narration {
//!                 text: Expr::string_literal("春天，樱花盛开的季节。"),
//!             },
//!             SceneNode::Dialogue {
//!                 speaker: Expr::string_literal("小百合"),
//!                 text: Expr::string_literal("初次见面！"),
//!                 voice_id: None,
//!             },
//!         ],
//!     };
//!
//!     let compiled = Compiler::new().compile(&scene).expect("编译失败");
//!
//!     // 序列化为 .asterbyte 文件供 VM 或 GameLauncher 加载
//!     let bytes = bincode::serialize(&compiled).expect("序列化失败");
//!     std::fs::write("prologue.asterbyte", bytes).expect("写入失败");
//! }
//! ```

// 模块声明
pub mod bytecode;
pub mod compiler;
pub mod error;
pub mod ir;

// 重导出 — 外部 crate 通过 `aster_compiler::` 路径直接使用
pub use bytecode::{CompiledScene, Opcode};
pub use compiler::Compiler;
pub use error::CompileError;
