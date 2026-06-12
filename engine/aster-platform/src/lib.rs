//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-platform/src/lib.rs
//! 功能概述：平台抽象层 — 封装 winit（窗口管理）、wgpu（GPU 渲染后端）、
//!           rodio（音频播放）等平台后端，为上层提供统一的 Platform trait 接口。
//!           本 crate 是架构分层的最底层，仅依赖标准库。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-12
//!
//! 依赖模块：
//! - 标准库（无外部依赖，Architecture.md §4.1）
//!
//! 架构位置：aster-platform ← aster-core ← 上层 crate（Architecture.md §4 分层图）

/// 平台抽象层 — 待 Phase 1 实现
///
/// 将定义：
/// - `Platform` trait：窗口创建、输入轮询、音频设备
/// - `WindowHandle`：跨平台窗口句柄
/// - `InputEvent`：统一的输入事件枚举
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // Phase 0 占位测试，Phase 1 实际开发时替换为平台初始化测试
        assert_eq!(2 + 2, 4);
    }
}
