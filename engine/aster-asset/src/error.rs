//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-asset/src/error.rs
//! 功能概述：资源管理错误类型 — 使用 thiserror 定义 `AssetError` 枚举，
//!           覆盖资源加载全流程中可能出现的错误（文件缺失、格式不支持、
//!           解码失败、IO 错误）。所有错误携带上下文信息便于创作者排查。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - thiserror（错误派生宏）
//!
//! 对应任务：PH2-T04 — aster-asset 资源加载基础设施

use thiserror::Error;

/// 资源管理错误 — 覆盖资源加载全流程的错误类型。
///
/// 所有变体携带足够的上下文信息（文件路径、格式名、原因描述），
/// 使得创作者和开发者能快速定位问题。
///
/// # 错误分类
///
/// | 变体 | 场景 | 可恢复？ |
/// |------|------|---------|
/// | `NotFound` | 资源文件缺失或路径错误 | ✅（提示用户检查文件） |
/// | `UnsupportedFormat` | 文件扩展名无对应加载器 | ✅（提示用户转换格式） |
/// | `DecodeError` | 文件内容损坏或格式不匹配 | ✅（提示用户修复文件） |
/// | `Io` | 文件系统错误（权限/磁盘） | ⚠️（取决于具体原因） |
#[derive(Debug, Error)]
pub enum AssetError {
    /// 资源文件不存在。
    ///
    /// 发生场景：
    /// - `AssetId` 对应的文件在磁盘上已被删除
    /// - 扫描后文件被移动/重命名
    /// - 路径拼写错误
    ///
    /// # 示例
    /// ```
    /// # use aster_asset::AssetError;
    /// let err = AssetError::NotFound {
    ///     path: "assets/bg/classroom.png".into(),
    /// };
    /// assert!(err.to_string().contains("assets/bg/classroom.png"));
    /// ```
    #[error("资源文件不存在：{path}")]
    NotFound { path: String },

    /// 不支持的资源格式 — 文件扩展名无对应加载器。
    ///
    /// 发生场景：
    /// - 尝试以 Background 类型加载 `.exe` 文件
    /// - 项目使用了引擎尚未支持的图片格式（如 `.heic`）
    ///
    /// `format` 字段记录实际的文件扩展名或 MIME 类型，
    /// 便于创作者了解当前文件为何无法加载。
    #[error("不支持的资源格式：{path}（格式：{format}）")]
    UnsupportedFormat { path: String, format: String },

    /// 资源解码失败 — 文件存在但内容无法解析。
    ///
    /// 发生场景：
    /// - PNG 文件头损坏
    /// - OGG 音频流包含非法帧
    /// - 图片尺寸超过 GPU 限制（> 8192×8192）
    ///
    /// `reason` 字段包含来自底层解码库的详细错误信息。
    #[error("资源解码失败：{reason}")]
    DecodeError { reason: String },

    /// IO 错误 — 文件系统操作失败。
    ///
    /// 发生场景：
    /// - 磁盘读取权限不足
    /// - 文件被其他进程锁定
    /// - 磁盘硬件故障
    ///
    /// 通过 `#[from]` 自动从 `std::io::Error` 转换。
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 NotFound 错误的格式化输出包含文件路径。
    #[test]
    fn test_not_found_error_format() {
        let err = AssetError::NotFound {
            path: "assets/bg/missing.png".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("assets/bg/missing.png"));
        assert!(msg.contains("不存在"));
    }

    /// 验证 UnsupportedFormat 错误的格式化输出包含路径和格式。
    #[test]
    fn test_unsupported_format_error_format() {
        let err = AssetError::UnsupportedFormat {
            path: "data.exe".into(),
            format: "exe".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("data.exe"));
        assert!(msg.contains("exe"));
    }

    /// 验证 DecodeError 错误的格式化输出包含原因。
    #[test]
    fn test_decode_error_format() {
        let err = AssetError::DecodeError {
            reason: "损坏的 PNG 文件头".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("损坏的 PNG 文件头"));
    }

    /// 验证从 std::io::Error 自动转换的 Io 变体。
    #[test]
    fn test_io_error_from_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "拒绝访问");
        let asset_err: AssetError = io_err.into();
        let msg = asset_err.to_string();
        assert!(msg.contains("拒绝访问") || msg.contains("IO"));
    }

    /// 验证 Debug 输出包含变体名称（用于开发调试）。
    #[test]
    fn test_debug_format() {
        let err = AssetError::NotFound {
            path: "test.png".into(),
        };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotFound"), "Debug 输出应包含变体名称");
    }
}
