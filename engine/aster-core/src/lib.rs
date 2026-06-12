//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/lib.rs
//! 功能概述：核心数据类型 — 定义整个引擎共享的基础数据结构：
//!           Scene（场景）/ SceneNode（演出单元）/ AssetId（资源标识）/
//!           VariableStore（变量存储）/ Choice（选择支）等。
//!           本 crate 不依赖任何其他 engine crate（Architecture.md §4.2）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-12
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//!
//! 架构位置：aster-platform ← aster-core ← aster-parser/compiler/vm/...

/// 核心数据类型 — 待 Phase 1 实现
///
/// 将定义：
/// - `Scene`：场景（一组 SceneNode 的有序列表）
/// - `SceneNode`：演出单元枚举（Dialogue/Menu/Narration/...）
/// - `AssetId`：newtype 资源标识符
/// - `VariableStore`：运行时变量存储
/// - `Choice`：选择支（文本 + 跳转目标）
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // Phase 0 占位测试，Phase 1 实际开发时替换为核心类型序列化测试
        assert_eq!(2 + 2, 4);
    }
}
