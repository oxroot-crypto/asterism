//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/src/lib.rs
//! 功能概述：字节码虚拟机 — 执行编译后的场景脚本（CompiledScene），
//!           管理运行时执行状态（指令指针、寄存器、调用栈、变量作用域）。
//!           通过 `VmAction` 枚举向 SceneManager 报告需要外部处理的"意图"
//!          （渲染命令、输入等待、菜单选择、场景结束）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - aster_core — 运行时数据类型（Value, VariableStore, FlagSet）
//! - aster_compiler — 编译产物（CompiledScene, Opcode 枚举）
//!
//! 架构位置：aster-compiler ← aster-vm（执行引擎核心）
//!
//! ## 模块概览
//!
//! | 模块 | 文件 | 说明 |
//! |------|------|------|
//! | `opcode` | `opcode.rs` | 操作码工具：Opcode 重导出 + 指令尺寸 + 解码辅助 |
//! | `engine_command` | `engine_command.rs` | 引擎命令：VM→SceneManager 的渲染/音频操作指令 |
//! | `action` | `action.rs` | VM 动作：step() 返回的交互/等待/命令/结束动作 |
//! | `vm` | `vm.rs` | VM 核心：Vm/CallFrame 结构体 + step() dispatch 循环 |
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use aster_compiler::{Compiler, CompiledScene};
//! use aster_core::Scene;
//! use aster_vm::Vm;
//!
//! fn run_scene(scene: &Scene) {
//!     let compiled = Compiler::new().compile(scene).expect("编译失败");
//!     let mut vm = Vm::new();
//!
//!     loop {
//!         match vm.step(&compiled) {
//!             aster_vm::VmAction::WaitForInput => {
//!                 // 等待用户点击后继续
//!             }
//!             aster_vm::VmAction::SceneEnd => break,
//!             aster_vm::VmAction::Command(cmd) => {
//!                 // 处理渲染/音频命令
//!             }
//!             aster_vm::VmAction::ShowMenu { .. } => {
//!                 // 显示菜单并等待选择
//!             }
//!         }
//!     }
//! }
//! ```

// 模块声明
pub mod action;
pub mod engine_command;
pub mod opcode;
pub mod vm;

// 重导出 — 外部 crate 通过 `aster_vm::` 路径直接使用
pub use action::{MenuChoiceData, VmAction};
pub use engine_command::EngineCommand;
pub use vm::{CallFrame, Vm};
