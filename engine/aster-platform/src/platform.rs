//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-platform/src/platform.rs
//! 功能概述：定义 `Platform` trait（跨平台抽象接口）、`PlatformError`（平台错误类型）、
//!           `LanguageTag`（BCP 47 语言标签）。本模块是 aster-platform 的核心，
//!           所有平台实现（Windows/macOS/Linux）必须实现此 trait。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - 标准库 std::path / std::ffi / std::process / std::fmt
//!
//! 架构位置：aster-platform 是架构分层最底层（Architecture.md §4.1），
//!           仅依赖标准库，不依赖其他 engine crate。

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Child;

// ============================================================================
// PlatformError — 平台操作错误类型
// ============================================================================

/// 平台操作错误类型 — 涵盖 IO 错误、锁获取失败、进程启动失败等场景。
///
/// 所有错误信息使用中文描述，携带足够的上下文供上层模块展示和记录。
/// 手动实现 `std::error::Error` 以保持 crate 的零外部依赖约束。
#[derive(Debug)]
pub enum PlatformError {
    /// IO 错误 — 文件系统操作失败（如目录创建失败、文件写入失败）
    Io(io::Error),

    /// 单实例锁获取失败 — 同一应用已在运行中
    LockFailed(String),

    /// 进程启动失败 — 无法启动外部可执行文件
    ProcessLaunchFailed(String),

    /// 通用平台错误 — 不属于上述分类的其他错误
    Generic(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Io(e) => write!(f, "IO 错误：{e}"),
            PlatformError::LockFailed(msg) => write!(f, "单实例锁获取失败：{msg}"),
            PlatformError::ProcessLaunchFailed(msg) => write!(f, "进程启动失败：{msg}"),
            PlatformError::Generic(msg) => write!(f, "平台错误：{msg}"),
        }
    }
}

impl std::error::Error for PlatformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlatformError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for PlatformError {
    fn from(e: io::Error) -> Self {
        PlatformError::Io(e)
    }
}

// ============================================================================
// LanguageTag — BCP 47 语言标签
// ============================================================================

/// BCP 47 语言标签 — 表示系统当前语言环境。
///
/// 格式示例：`zh-CN`（简体中文）、`ja-JP`（日语）、`en-US`（美式英语）。
/// 内部存储为 `String`，提供 `as_str()` 访问器和 `Display` 实现。
///
/// # 示例
/// ```rust
/// use aster_platform::LanguageTag;
/// let lang = LanguageTag::new("zh-CN");
/// assert_eq!(lang.as_str(), "zh-CN");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanguageTag(String);

impl LanguageTag {
    /// 从字符串创建语言标签。
    ///
    /// # 参数
    /// - `tag`: BCP 47 格式的语言标签（如 `"zh-CN"`）
    ///
    /// # 返回值
    /// 始终返回 `LanguageTag`，不做严格验证（不 panic）。
    /// 传入空字符串将生成 `"und"`（undetermined，未确定语言）标签。
    pub fn new(tag: &str) -> Self {
        if tag.is_empty() {
            LanguageTag("und".to_string())
        } else {
            LanguageTag(tag.to_string())
        }
    }

    /// 获取标签的字符串切片。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 获取主语言代码（BCP 47 的 primary language subtag）。
    ///
    /// 对于 `"zh-CN"` 返回 `"zh"`，对于 `"en"` 返回 `"en"`。
    /// 如果标签中不包含 `-`，返回整个标签。
    pub fn primary_language(&self) -> &str {
        self.0.split('-').next().unwrap_or(&self.0)
    }

    /// 获取地区代码（BCP 47 的 region subtag）。
    ///
    /// 对于 `"zh-CN"` 返回 `Some("CN")`，对于 `"en"` 返回 `None`。
    pub fn region(&self) -> Option<&str> {
        self.0.split('-').nth(1)
    }
}

impl fmt::Display for LanguageTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for LanguageTag {
    fn from(s: &str) -> Self {
        LanguageTag::new(s)
    }
}

impl From<String> for LanguageTag {
    fn from(s: String) -> Self {
        if s.is_empty() {
            LanguageTag("und".to_string())
        } else {
            LanguageTag(s)
        }
    }
}

// ============================================================================
// Platform trait — 跨平台抽象接口
// ============================================================================

/// 跨平台抽象 trait — 所有引擎模块通过此接口获取平台能力。
///
/// 提供统一的系统级能力入口，包括：
/// - **路径管理**：用户配置目录、存档目录、路径规范化
/// - **剪贴板操作**：复制/粘贴文本
/// - **系统信息**：系统语言
/// - **进程管理**：单实例锁、启动外部进程
///
/// 各平台（Windows/macOS/Linux）各自提供具体实现，
/// 通过条件编译（`#[cfg(target_os)]`）选择。
///
/// # 线程安全
/// 派生 `Send + Sync`，可在线程间共享。
pub trait Platform: Send + Sync {
    /// 获取用户配置目录。
    ///
    /// 此目录用于存放引擎和游戏的配置文件、日志、缓存等用户级数据。
    /// 目录不存在时会自动创建。
    ///
    /// ## 各平台路径
    /// - **Windows**: `%APPDATA%/Asterism/`
    /// - **macOS**: `~/Library/Application Support/com.asterism.engine/`
    /// - **Linux**: `~/.local/share/asterism/`（遵循 XDG 规范）
    fn user_config_dir(&self) -> PathBuf;

    /// 获取指定游戏的默认存档目录。
    ///
    /// ## 各平台路径
    /// - **Windows**: `%USERPROFILE%/Documents/My Games/{game_name}/saves/`
    /// - **macOS**: `~/Library/Application Support/{game_name}/saves/`
    /// - **Linux**: `~/.local/share/{game_name}/saves/`（遵循 XDG 规范）
    ///
    /// # 参数
    /// - `game_name`: 游戏名称，用作目录名的一部分
    fn default_save_dir(&self, game_name: &str) -> PathBuf;

    /// 路径规范化 — 统一路径分隔符为正斜杠 `/`。
    ///
    /// 处理以下情况：
    /// - Windows 反斜杠 `\` → 正斜杠 `/`
    /// - Windows 长路径前缀 `\\?\` 被移除
    /// - 连续多个分隔符合并为一个
    ///
    /// # 参数
    /// - `raw`: 原始操作系统路径（`OsStr`）
    ///
    /// # 返回值
    /// 规范化后的路径，分隔符统一为 `/`
    fn normalize_path(&self, raw: &OsStr) -> PathBuf;

    /// 将文本复制到系统剪贴板。
    ///
    /// # 注意
    /// Phase 1 阶段此方法为存根（stub），不执行实际操作。
    /// 完整的剪贴板功能将在后续 Phase 中通过 winit 的 Clipboard 能力实现。
    ///
    /// # 参数
    /// - `text`: 要复制的文本
    fn clipboard_copy(&self, text: &str);

    /// 从系统剪贴板粘贴文本。
    ///
    /// # 返回值
    /// - `Some(String)`: 剪贴板中的文本内容
    /// - `None`: 剪贴板为空或内容不是文本
    ///
    /// # 注意
    /// Phase 1 阶段此方法为存根（stub），始终返回 `None`。
    fn clipboard_paste(&self) -> Option<String>;

    /// 获取系统当前语言环境。
    ///
    /// # 返回值
    /// BCP 47 格式的语言标签，如 `"zh-CN"`、`"ja-JP"`、`"en-US"`。
    /// 如果无法确定系统语言，返回 `"en-US"` 作为默认值。
    fn system_language(&self) -> LanguageTag;

    /// 尝试获取单实例锁 — 防止同一个游戏被多次启动。
    ///
    /// 使用文件锁机制：在临时目录创建一个以 `app_id` 命名的锁文件，
    /// 如果文件已存在且被其他进程持有，则获取失败。
    ///
    /// # 参数
    /// - `app_id`: 应用唯一标识符，用于区分不同游戏的锁
    ///
    /// # 返回值
    /// - `true`: 锁获取成功，当前是第一个实例
    /// - `false`: 锁获取失败，已有另一实例在运行
    fn try_acquire_single_instance(&self, app_id: &str) -> bool;

    /// 启动外部进程。
    ///
    /// 封装 `std::process::Command`，提供统一的错误处理。
    /// 用于启动外部工具（如 packager 打包、IDE 预览桥接等）。
    ///
    /// # 参数
    /// - `executable`: 可执行文件路径
    /// - `args`: 命令行参数列表
    ///
    /// # 返回值
    /// - `Ok(Child)`: 成功启动的子进程句柄
    /// - `Err(PlatformError)`: 启动失败的原因
    fn launch_process(&self, executable: &Path, args: &[&str]) -> Result<Child, PlatformError>;
}

// ============================================================================
// 辅助函数 — 各平台实现共用
// ============================================================================

/// 确保目录存在，不存在则递归创建。
///
/// # 参数
/// - `path`: 需要确保存在的目录路径
///
/// # 返回值
/// 如果目录已存在或创建成功返回 `Ok(())`，否则返回 `PlatformError`
pub(crate) fn ensure_dir(path: &Path) -> Result<(), PlatformError> {
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(PlatformError::Io)?;
    }
    Ok(())
}

/// 从环境变量获取 home 目录路径。
///
/// 按以下优先级查找：
/// 1. `HOME`（Unix/Linux/macOS）
/// 2. `USERPROFILE`（Windows）
/// 3. 组合 `HOMEDRIVE` + `HOMEPATH`（Windows 备选方案）
///
/// # 返回值
/// - `Some(PathBuf)`: 找到 home 目录
/// - `None`: 所有环境变量均不存在（极端情况）
pub(crate) fn home_dir() -> Option<PathBuf> {
    // Unix 风格：HOME
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home));
    }
    // Windows 风格：USERPROFILE
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return Some(PathBuf::from(profile));
    }
    // Windows 备选：HOMEDRIVE + HOMEPATH
    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        let combined = format!("{drive}{path}");
        if !combined.is_empty() {
            return Some(PathBuf::from(combined));
        }
    }
    None
}

/// 将路径字符串中的所有反斜杠替换为正斜杠，并合并连续分隔符。
///
/// 这是 `normalize_path` 的通用实现，所有平台共享。
/// macOS/Linux 通常不需要此处理（原生路径已为正斜杠），但调用无害。
pub(crate) fn normalize_path_string(raw: &str) -> String {
    // 步骤1：替换反斜杠为正斜杠
    let mut result = raw.replace('\\', "/");
    // 步骤2：移除 Windows 长路径前缀 `\\?\`
    if result.starts_with("//?/") {
        let after_prefix = &result[4..];
        // `\\?\UNC\server\share\...` → 还原为 UNC 路径 `//server/share/...`
        if let Some(unc_path) = after_prefix.strip_prefix("UNC/") {
            result = format!("//{unc_path}");
        } else {
            result = after_prefix.to_string();
        }
    }
    // 步骤3：合并连续的正斜杠（保留开头的双斜杠用于 UNC 路径）
    let prefix = if result.starts_with("//") && !result.starts_with("///") {
        "//"
    } else {
        ""
    };
    let rest = if prefix.is_empty() {
        result.as_str()
    } else {
        // 去除开头的 "//" 后再去除前导斜杠，防止形成 "///" 前缀
        let without_prefix = &result[2..];
        without_prefix.trim_start_matches('/')
    };
    let mut normalized = String::with_capacity(rest.len());
    let mut prev_was_slash = false;
    for ch in rest.chars() {
        if ch == '/' {
            if !prev_was_slash {
                normalized.push('/');
                prev_was_slash = true;
            }
        } else {
            normalized.push(ch);
            prev_was_slash = false;
        }
    }
    if prefix.is_empty() {
        normalized
    } else {
        format!("{prefix}{normalized}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_language_tag_new_normal() {
        let tag = LanguageTag::new("zh-CN");
        assert_eq!(tag.as_str(), "zh-CN");
        assert_eq!(tag.primary_language(), "zh");
        assert_eq!(tag.region(), Some("CN"));
    }

    #[test]
    fn test_language_tag_empty_returns_und() {
        let tag = LanguageTag::new("");
        assert_eq!(tag.as_str(), "und");
        assert_eq!(tag.primary_language(), "und");
        assert_eq!(tag.region(), None);
    }

    #[test]
    fn test_language_tag_no_region() {
        let tag = LanguageTag::new("en");
        assert_eq!(tag.as_str(), "en");
        assert_eq!(tag.primary_language(), "en");
        assert_eq!(tag.region(), None);
    }

    #[test]
    fn test_language_tag_display() {
        let tag = LanguageTag::new("ja-JP");
        assert_eq!(format!("{tag}"), "ja-JP");
    }

    #[test]
    fn test_language_tag_from_str() {
        let tag: LanguageTag = "ko-KR".into();
        assert_eq!(tag.as_str(), "ko-KR");
    }

    #[test]
    fn test_platform_error_display() {
        let io_err = PlatformError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "文件不存在",
        ));
        assert!(format!("{io_err}").contains("IO 错误"));

        let lock_err = PlatformError::LockFailed("test_app".to_string());
        assert!(format!("{lock_err}").contains("单实例锁获取失败"));

        let launch_err = PlatformError::ProcessLaunchFailed("notepad".to_string());
        assert!(format!("{launch_err}").contains("进程启动失败"));

        let generic_err = PlatformError::Generic("未知错误".to_string());
        assert!(format!("{generic_err}").contains("平台错误"));
    }

    #[test]
    fn test_platform_error_source() {
        let io_err = PlatformError::Io(std::io::Error::other("test"));
        assert!(io_err.source().is_some());

        let generic_err = PlatformError::Generic("test".to_string());
        assert!(generic_err.source().is_none());
    }

    #[test]
    fn test_platform_error_from_io() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "禁止访问");
        let err: PlatformError = io.into();
        assert!(matches!(err, PlatformError::Io(_)));
    }

    #[test]
    fn test_normalize_path_string_backslash_to_forward() {
        let result = normalize_path_string("a\\b\\c");
        assert_eq!(result, "a/b/c");
    }

    #[test]
    fn test_normalize_path_string_removes_extended_prefix() {
        let result = normalize_path_string("//?/C:/Users/test");
        assert_eq!(result, "C:/Users/test");
    }

    #[test]
    fn test_normalize_path_string_collapses_multiple_slashes() {
        let result = normalize_path_string("a///b//c");
        assert_eq!(result, "a/b/c");
    }

    #[test]
    fn test_normalize_path_string_handles_empty() {
        let result = normalize_path_string("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_normalize_path_string_preserves_unc_prefix() {
        let result = normalize_path_string("//server/share/path");
        // 应该以 "//" 开头，但不能有多余的前导斜杠（即不应变成 "///"）
        assert!(
            result.starts_with("//"),
            "UNC 路径应以 // 开头，实际: {result}"
        );
        assert!(
            !result.starts_with("///"),
            "UNC 路径不应有 /// 前缀，实际: {result}"
        );
        assert_eq!(result, "//server/share/path", "UNC 路径应保持原样");
    }

    #[test]
    fn test_normalize_path_string_extended_unc_prefix() {
        // \\?\UNC\server\share\path → //server/share/path
        let result = normalize_path_string("//?/UNC/server/share/path");
        assert_eq!(
            result, "//server/share/path",
            "扩展 UNC 前缀应转换为标准 UNC"
        );
    }
}
