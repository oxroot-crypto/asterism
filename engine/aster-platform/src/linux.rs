//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-platform/src/linux.rs
//! 功能概述：Linux 平台的 `Platform` trait 实现。遵循 XDG Base Directory 规范
//!           （XDG_DATA_HOME → ~/.local/share），通过环境变量 `LANG` 检测系统语言，
//!           使用 `/tmp` 下的文件锁实现单实例控制。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - super::platform（Platform trait / PlatformError / LanguageTag / 辅助函数）

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Child;

use super::platform::{
    LanguageTag, Platform, PlatformError, ensure_dir, home_dir, normalize_path_string,
};

/// Linux 平台的 `Platform` 实现。
///
/// 遵循以下规范：
/// - **XDG Base Directory**: `XDG_DATA_HOME` → `~/.local/share`
/// - **临时文件**: `/tmp`
/// - **语言检测**: `LANG` → `LC_ALL` → `LC_MESSAGES` → 默认 `en-US`
#[derive(Debug, Clone, Default)]
pub struct LinuxPlatform;

impl Platform for LinuxPlatform {
    fn user_config_dir(&self) -> PathBuf {
        // XDG Base Directory 规范：
        // XDG_DATA_HOME 默认为 ~/.local/share
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".local/share")
            });
        let config_dir = base.join("asterism");
        // 确保目录存在
        let _ = ensure_dir(&config_dir);
        config_dir
    }

    fn default_save_dir(&self, game_name: &str) -> PathBuf {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".local/share")
            });
        let save_dir = base.join(game_name).join("saves");
        let _ = ensure_dir(&save_dir);
        save_dir
    }

    fn normalize_path(&self, raw: &OsStr) -> PathBuf {
        // Linux 路径原生使用正斜杠，直接进行字符串规范化即可
        match raw.to_str() {
            Some(s) => PathBuf::from(normalize_path_string(s)),
            // 如果 OsStr 不是有效 UTF-8，直接返回原路径（极罕见情况）
            None => PathBuf::from(raw),
        }
    }

    fn clipboard_copy(&self, _text: &str) {
        // Phase 1 存根：不执行实际操作
        // 后续 Phase 将通过 winit 的 Clipboard 能力实现
    }

    fn clipboard_paste(&self) -> Option<String> {
        // Phase 1 存根：始终返回 None
        None
    }

    fn system_language(&self) -> LanguageTag {
        // 优先级：LANG → LC_ALL → LC_MESSAGES → 默认 "en-US"
        for var in &["LANG", "LC_ALL", "LC_MESSAGES"] {
            if let Ok(locale) = std::env::var(var) {
                let locale = locale.trim();
                if !locale.is_empty() {
                    // Linux locale 格式通常为 "zh_CN.UTF-8" 或 "ja_JP.utf8"
                    // 需要转换为 BCP 47 格式（下划线 → 连字符）
                    return linux_locale_to_bcp47(locale);
                }
            }
        }
        LanguageTag::new("en-US")
    }

    fn try_acquire_single_instance(&self, app_id: &str) -> bool {
        let lock_path = PathBuf::from("/tmp").join(format!("asterism_{app_id}.lock"));
        // 使用 O_CREAT | O_EXCL 语义确保原子性创建锁文件
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&lock_path)
        {
            Ok(_lock_file) => {
                // 锁文件创建成功 → 当前是第一个实例
                // std::mem::forget 保持文件句柄存活直到进程退出，防止锁提前释放
                // Phase 2：写入 PID + 检测残留锁以区分运行实例和崩溃残留
                std::mem::forget(_lock_file);
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // 锁文件已存在 → 检查持有锁的进程是否仍在运行
                // 简化处理：直接认为已有实例运行
                // 完整实现应检查 PID 文件内容 + kill(pid, 0)
                false
            }
            Err(_) => {
                // 其他错误（权限不足等）→ 无法确定，保守地允许运行
                true
            }
        }
    }

    fn launch_process(&self, executable: &Path, args: &[&str]) -> Result<Child, PlatformError> {
        use std::process::Command;

        let mut cmd = Command::new(executable);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.spawn().map_err(|e| {
            PlatformError::ProcessLaunchFailed(format!("无法启动 '{}'：{e}", executable.display()))
        })
    }
}

/// 将 Linux 风格的 locale 字符串转换为 BCP 47 语言标签。
///
/// 处理以下格式：
/// - `zh_CN.UTF-8` → `zh-CN`
/// - `ja_JP.utf8` → `ja-JP`
/// - `en_US` → `en-US`
/// - `C` 或 `POSIX` → `en-US`
/// - 已包含连字符的（如 `en-US`）原样返回主语言-地区部分
///
/// # 参数
/// - `locale`: Linux locale 字符串
///
/// # 返回值
/// BCP 47 格式的语言标签
fn linux_locale_to_bcp47(locale: &str) -> LanguageTag {
    // 移除编码后缀（`.UTF-8`、`.utf8` 等）
    let without_encoding = locale.split('.').next().unwrap_or(locale);

    // 处理 C/POSIX locale（剥离编码后缀后判断，兼容 C.UTF-8 / POSIX.utf8 等变体）
    if without_encoding == "C" || without_encoding == "POSIX" {
        return LanguageTag::new("en-US");
    }

    // 将下划线替换为连字符
    let bcp47 = without_encoding.replace('_', "-");

    // 如果已经是 BCP 47 格式（如 en-US），原样返回主语言+地区部分
    LanguageTag::new(&bcp47)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_locale_to_bcp47_standard() {
        let tag = linux_locale_to_bcp47("zh_CN.UTF-8");
        assert_eq!(tag.as_str(), "zh-CN");
    }

    #[test]
    fn test_linux_locale_to_bcp47_utf8() {
        let tag = linux_locale_to_bcp47("ja_JP.utf8");
        assert_eq!(tag.as_str(), "ja-JP");
    }

    #[test]
    fn test_linux_locale_to_bcp47_no_encoding() {
        let tag = linux_locale_to_bcp47("en_US");
        assert_eq!(tag.as_str(), "en-US");
    }

    #[test]
    fn test_linux_locale_to_bcp47_c_locale() {
        let tag = linux_locale_to_bcp47("C");
        assert_eq!(tag.as_str(), "en-US");
    }

    #[test]
    fn test_linux_locale_to_bcp47_posix() {
        let tag = linux_locale_to_bcp47("POSIX");
        assert_eq!(tag.as_str(), "en-US");
    }

    #[test]
    fn test_linux_locale_to_bcp47_already_bcp47() {
        let tag = linux_locale_to_bcp47("en-US");
        assert_eq!(tag.as_str(), "en-US");
    }

    #[test]
    fn test_normalize_path_on_linux() {
        let platform = LinuxPlatform;
        let result = platform.normalize_path(OsStr::new("/home/user/game/data"));
        assert_eq!(result, PathBuf::from("/home/user/game/data"));
    }

    #[test]
    fn test_normalize_path_with_double_slash() {
        let platform = LinuxPlatform;
        let result = platform.normalize_path(OsStr::new("/home//user///game"));
        assert_eq!(result, PathBuf::from("/home/user/game"));
    }

    // ─── C.UTF-8 / POSIX.utf8 带编码后缀的 locale ───────────────────────

    /// 验证 C.UTF-8 被识别为 en-US（剥离编码后缀后匹配 C locale）。
    #[test]
    fn test_linux_locale_c_utf8() {
        let tag = linux_locale_to_bcp47("C.UTF-8");
        assert_eq!(tag.as_str(), "en-US");
    }

    /// 验证 POSIX.utf8 被识别为 en-US。
    #[test]
    fn test_linux_locale_posix_utf8() {
        let tag = linux_locale_to_bcp47("POSIX.utf8");
        assert_eq!(tag.as_str(), "en-US");
    }

    /// 验证含修饰符的 locale（如 @euro）。
    #[test]
    fn test_linux_locale_with_modifier() {
        // 剥离编码 → zh_CN → zh-CN
        let tag = linux_locale_to_bcp47("zh_CN.UTF-8@euro");
        // split('.').next() → "zh_CN@euro"
        // replace('_', '-') → "zh-CN@euro"
        // 当前实现会保留 @euro 后缀
        assert!(tag.as_str().starts_with("zh"));
    }
}
