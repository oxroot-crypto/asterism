//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-audio/src/error.rs
//! 功能概述：音频系统错误类型 — 定义 `AudioError` 枚举，
//!           覆盖资源未找到、解码失败、播放失败和 IO 错误四种场景。
//!           使用 thiserror 派生，自动实现 Display + Error trait。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - thiserror（派生宏）
//! - std::io（IO 错误转换）

use thiserror::Error;

/// 音频系统错误类型 — 覆盖 BGM/SE 播放过程中可能出现的所有错误场景。
///
/// 所有错误信息均为中文，面向引擎开发者（而非最终玩家），
/// 包含足够的上下文信息以便定位问题。
///
/// # 错误分类
///
/// | 变体 | 场景 | 携带信息 |
/// |------|------|---------|
/// | `AssetNotFound` | 音频文件不存在 | 文件路径 |
/// | `DecodeError` | 音频解码失败（格式不支持/文件损坏） | 失败原因 |
/// | `PlaybackError` | kira 播放提交失败（音频设备异常等） | 失败原因 |
/// | `Io` | 文件系统 IO 错误 | 原始 std::io::Error |
///
/// # 示例
/// ```
/// use aster_audio::AudioError;
///
/// let err = AudioError::AssetNotFound {
///     path: "bgm/missing.ogg".into(),
/// };
/// assert_eq!(
///     err.to_string(),
///     "音频资源不存在：bgm/missing.ogg"
/// );
/// ```
#[derive(Debug, Error)]
pub enum AudioError {
    /// 音频文件不存在
    ///
    /// `path` 为相对于项目根目录或 `assets/` 目录的文件路径。
    /// 上层调用者应根据场景决定降级策略（静默跳过 / 警告 / 报错）。
    #[error("音频资源不存在：{path}")]
    AssetNotFound {
        /// 不存在的文件路径
        path: String,
    },

    /// 音频文件解码失败
    ///
    /// 可能原因：
    /// - 文件格式不支持（非 OGG/FLAC/MP3/WAV）
    /// - 文件内容已损坏
    /// - 文件头与扩展名不匹配
    /// - symphonia 解码器内部错误
    #[error("音频解码失败：{reason}")]
    DecodeError {
        /// 解码失败的具体原因（来自 kira/symphonia 的错误信息）
        reason: String,
    },

    /// 音频播放提交失败
    ///
    /// 可能原因：
    /// - 系统音频设备不可用（被其他程序独占、驱动异常）
    /// - kira 内部队列已满
    /// - 音频设备采样率配置不匹配
    #[error("音频播放失败：{reason}")]
    PlaybackError {
        /// 播放失败的具体原因（来自 kira 的错误信息）
        reason: String,
    },

    /// 文件系统 IO 错误
    ///
    /// 通过 `#[from]` 自动从 `std::io::Error` 转换，
    /// 覆盖文件读取、写入、权限拒绝等场景。
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
}
