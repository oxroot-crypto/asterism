//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/src/lib.rs
//! 功能概述：字节码虚拟机 — 执行编译后的场景脚本（CompiledScene），
//!           管理运行时执行状态（指令指针、调用栈、变量作用域）。
//!           支持条件跳转、子场景调用、菜单选择支等待等控制流。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-12
//!
//! 依赖模块：
//! - aster_core（待 Phase 2 添加）：Scene/VariableStore 等
//! - aster_compiler（待 Phase 2 添加）：Bytecode 指令定义
//!
//! 架构位置：aster-compiler ← aster-vm（执行引擎核心）

/// 字节码虚拟机 — 待 Phase 2 实现
///
/// 将定义：
/// - `Vm`：虚拟机主结构（寄存器、栈、指令指针）
/// - `Vm::execute(scene: &CompiledScene)`：执行编译后的场景
/// - `VmState`：运行/等待输入/暂停/结束 状态枚举
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // Phase 0 占位测试，Phase 2 实际开发时替换为 VM 指令执行测试
        assert_eq!(2 + 2, 4);
    }
}
