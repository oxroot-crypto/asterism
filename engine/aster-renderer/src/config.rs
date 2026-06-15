//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/config.rs
//! 功能概述：渲染器配置 — 定义窗口分辨率、垂直同步、全屏、MSAA 等渲染参数。
//!           纯数据结构，不依赖 GPU 或窗口系统，可独立测试。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：无（纯数据结构）

// ============================================================================
// RenderConfig — 渲染器配置
// ============================================================================

/// 渲染器配置 — 控制窗口尺寸、垂直同步、全屏模式、MSAA 等参数。
///
/// 所有字段均为公开，支持直接构造或通过 `Default` trait 获取推荐默认值。
/// 默认值对应 1080p 窗口模式，垂直同步开启，无 MSAA（v0.1 阶段暂不支持）。
///
/// # 字段说明
/// - `width` / `height`: 窗口内尺寸（逻辑像素），必须 ≥ 1
/// - `fullscreen`: 全屏模式开关（默认 false，窗口模式）
/// - `vsync`: 垂直同步开关（默认 true，防止画面撕裂）
/// - `msaa_samples`: 多重采样数（默认 1 = 无 MSAA，有效值：1/2/4/8）
/// - `clear_color`: 清屏颜色 RGBA（默认黑色 `[0, 0, 0, 1]`）
///
/// # 使用示例
/// ```rust
/// use aster_renderer::RenderConfig;
///
/// // 使用默认配置
/// let config = RenderConfig::default();
/// assert_eq!(config.width, 1920);
/// assert_eq!(config.height, 1080);
///
/// // 自定义配置
/// let config = RenderConfig {
///     width: 1280,
///     height: 720,
///     vsync: false,
///     ..RenderConfig::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// 窗口宽度（逻辑像素），默认 1920
    pub width: u32,
    /// 窗口高度（逻辑像素），默认 1080
    pub height: u32,
    /// 是否全屏模式，默认 false（窗口模式）
    pub fullscreen: bool,
    /// 是否开启垂直同步，默认 true
    pub vsync: bool,
    /// 多重采样数，默认 1（无 MSAA），有效值：1/2/4/8
    pub msaa_samples: u32,
    /// 清屏颜色 RGBA，各分量范围 0.0~1.0，默认黑色 `[0.0, 0.0, 0.0, 1.0]`
    pub clear_color: [f64; 4],
}

impl Default for RenderConfig {
    /// 返回推荐的默认渲染配置：
    /// - 分辨率：1920×1080（Full HD）
    /// - 窗口模式
    /// - 垂直同步开启
    /// - MSAA 关闭（v0.1 阶段暂不支持）
    /// - 清屏颜色：黑色
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fullscreen: false,
            vsync: true,
            msaa_samples: 1,
            clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC02
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // AC02 — RenderConfig::default() 默认值验证
    // ========================================================================

    /// AC02: 验证 `RenderConfig::default()` 返回 1920×1080、vsync=true、fullscreen=false。
    #[test]
    fn ac02_default_values() {
        let config = RenderConfig::default();
        assert_eq!(config.width, 1920, "默认宽度应为 1920");
        assert_eq!(config.height, 1080, "默认高度应为 1080");
        assert!(!config.fullscreen, "默认应为窗口模式");
        assert!(config.vsync, "默认应开启垂直同步");
        assert_eq!(config.msaa_samples, 1, "默认 MSAA 应为 1（关闭）");
        assert_eq!(
            config.clear_color,
            [0.0, 0.0, 0.0, 1.0],
            "默认清屏颜色应为黑色"
        );
    }

    /// AC02: 验证 `RenderConfig::default()` 通过 Default trait 调用。
    #[test]
    fn ac02_default_via_trait() {
        let config: RenderConfig = Default::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert!(config.vsync);
    }

    // ========================================================================
    // 边界测试
    // ========================================================================

    /// 验证零尺寸构造不 panic（clamp 由 GpuContext 处理）。
    #[test]
    fn zero_size_does_not_panic() {
        let config = RenderConfig {
            width: 0,
            height: 0,
            ..RenderConfig::default()
        };
        // 配置结构体本身允许零值，实际 clamp 在 GpuContext 中处理
        assert_eq!(config.width, 0);
        assert_eq!(config.height, 0);
    }

    /// 验证自定义配置可正确构造。
    #[test]
    fn custom_config_construction() {
        let config = RenderConfig {
            width: 1280,
            height: 720,
            fullscreen: true,
            vsync: false,
            msaa_samples: 4,
            clear_color: [1.0, 0.0, 0.0, 1.0],
        };
        assert_eq!(config.width, 1280);
        assert_eq!(config.height, 720);
        assert!(config.fullscreen);
        assert!(!config.vsync);
        assert_eq!(config.msaa_samples, 4);
        assert_eq!(config.clear_color, [1.0, 0.0, 0.0, 1.0]);
    }

    /// 验证 struct update 语法（..Default::default()）。
    #[test]
    fn struct_update_syntax() {
        let config = RenderConfig {
            width: 800,
            ..RenderConfig::default()
        };
        assert_eq!(config.width, 800);
        // 其余字段应为默认值
        assert_eq!(config.height, 1080);
        assert!(config.vsync);
        assert!(!config.fullscreen);
    }
}
