//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/src/lib.rs
//! 功能概述：.aster 编译器 — 将 AST（aster-parser 的输出）编译为字节码（Bytecode），
//!           供字节码虚拟机（aster-vm）执行。负责语义检查、变量解析、跳转目标绑定。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-12
//!
//! 依赖模块：
//! - aster_core（待 Phase 1 添加）：基础数据类型
//! - aster_parser（待 Phase 1 添加）：AST 输入
//!
//! 架构位置：aster-parser ← aster-compiler ← aster-vm

/// .aster 编译器 — 待 Phase 2 实现
///
/// 将定义：
/// - `compile(scene: &ParsedScene) -> Result<CompiledScene, Vec<CompileError>>`
/// - `CompiledScene`：编译产物（字节码 + 符号表 + 元数据）
/// - `Bytecode`：虚拟机的指令序列
/// - `CompileError`：语义错误（未定义变量、无效跳转等）
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // Phase 0 占位测试，Phase 2 实际开发时替换为编译流程测试
        assert_eq!(2 + 2, 4);
    }
}
