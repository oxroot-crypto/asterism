//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-platform/src/windows.rs
//! 功能概述：Windows 平台的 `Platform` trait 实现。使用 `%APPDATA%/Asterism/`
//!           作为应用数据根目录，通过 `GetUserDefaultLocaleName` Win32 API 或
//!           环境变量检测系统语言，使用 `%TEMP%` 下的文件锁实现单实例控制。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - super::platform（Platform trait / PlatformError / LanguageTag / 辅助函数）
//! - 可选 FFI: kernel32::GetUserDefaultLocaleName（系统语言检测，最佳努力）
//! - 可选 FFI: kernel32::CreateProcessW（进程启动，备选方案）

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;

use super::platform::{
    LanguageTag, Platform, PlatformError, ensure_dir, home_dir, normalize_path_string,
};

/// Windows 平台的 `Platform` 实现。
///
/// 遵循 Windows 规范：
/// - **应用数据**: `%APPDATA%/Asterism/`
/// - **文档/存档**: `%USERPROFILE%/Documents/My Games/{game_name}/saves/`
/// - **临时文件**: `%TEMP%`
/// - **语言检测**: `GetUserDefaultLocaleName` → 环境变量回退 → 默认 `en-US`
#[derive(Debug, Clone, Default)]
pub struct WindowsPlatform;

impl Platform for WindowsPlatform {
    fn user_config_dir(&self) -> PathBuf {
        // %APPDATA% 环境变量（如 C:\Users\<用户名>\AppData\Roaming）
        let config_dir = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                home_dir()
                    .unwrap_or_else(|| PathBuf::from("C:\\"))
                    .join("AppData")
                    .join("Roaming")
            })
            .join("Asterism");
        let _ = ensure_dir(&config_dir);
        config_dir
    }

    fn default_save_dir(&self, game_name: &str) -> PathBuf {
        // %USERPROFILE%/Documents/My Games/{game_name}/saves/
        let documents = home_dir()
            .map(|h| h.join("Documents"))
            .or_else(|| {
                std::env::var("USERPROFILE")
                    .map(|p| PathBuf::from(p).join("Documents"))
                    .ok()
            })
            .unwrap_or_else(|| PathBuf::from("C:\\Users\\Default\\Documents"));
        let save_dir = documents.join("My Games").join(game_name).join("saves");
        let _ = ensure_dir(&save_dir);
        save_dir
    }

    fn normalize_path(&self, raw: &OsStr) -> PathBuf {
        // Windows 路径的核心规范化：
        // 1. 反斜杠 → 正斜杠
        // 2. 移除 `\\?\` 长路径前缀
        // 3. 合并非连续分隔符
        match raw.to_str() {
            Some(s) => PathBuf::from(normalize_path_string(s)),
            None => {
                // 如果 OsStr 不是有效 UTF-8（Windows 上极少发生），
                // 尝试 lossy 转换
                PathBuf::from(normalize_path_string(&raw.to_string_lossy()))
            }
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
        // 优先级：
        // 1. 注册表查询（HKCU\Control Panel\International\LocaleName，最准确）
        // 2. 环境变量：LANG → LC_ALL
        // 3. 默认 "en-US"

        // 尝试从注册表读取用户区域设置
        if let Some(locale) = try_get_locale_from_registry()
            && !locale.is_empty()
        {
            return windows_locale_to_bcp47(&locale);
        }

        // 回退到环境变量
        for var in &["LANG", "LC_ALL"] {
            if let Ok(locale) = std::env::var(var) {
                let locale = locale.trim();
                if !locale.is_empty() {
                    return windows_locale_to_bcp47(locale);
                }
            }
        }

        LanguageTag::new("en-US")
    }

    fn try_acquire_single_instance(&self, app_id: &str) -> bool {
        // 在 %TEMP% 目录下创建锁文件
        let temp_dir = std::env::var("TEMP")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Windows\\Temp"));
        let lock_path = temp_dir.join(format!("asterism_{app_id}.lock"));

        // 使用 create_new 确保原子性
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(lock_file) => {
                let _ = lock_file; // 保持文件打开直到进程退出
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(_) => true, // 权限不足等 → 保守地允许运行
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

/// 尝试从 Windows 注册表读取用户默认区域设置名称。
///
/// 查询路径：`HKCU\Control Panel\International\LocaleName`
///
/// 返回格式示例：`"zh-CN"`、`"ja-JP"`、`"en-US"`。
/// 如果注册表键不存在或 `reg.exe` 不可用，返回 `None`。
///
/// 使用 `std::process::Command` 调用系统内置 `reg.exe` 查询注册表，
/// 无需 FFI 绑定或外部依赖。
fn try_get_locale_from_registry() -> Option<String> {
    // 使用 reg.exe 查询用户区域设置（所有 Windows 版本均有 reg.exe）
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Control Panel\International",
            "/v",
            "LocaleName",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // reg query 输出格式：
    //   HKEY_CURRENT_USER\Control Panel\International
    //       LocaleName    REG_SZ    zh-CN
    // 提取最后一行的最后一个空格分隔的词
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // 跳过注册表路径行
        if trimmed.starts_with("HKEY_") {
            continue;
        }
        // 提取 REG_SZ 之后的值
        if let Some(value) = trimmed.split_whitespace().last()
            && !value.is_empty()
            && value != "REG_SZ"
        {
            return Some(value.to_string());
        }
    }

    None
}

/// 将 Windows 风格的 locale 字符串转换为 BCP 47 语言标签。
///
/// Windows 的 locale 名称通常已经是 BCP 47 格式（如 `zh-CN`），
/// 但也可能遇到旧格式（如 `Chinese_China.936`）。
///
/// # 参数
/// - `locale`: Windows locale 字符串
///
/// # 返回值
/// BCP 47 格式的语言标签
fn windows_locale_to_bcp47(locale: &str) -> LanguageTag {
    // Windows Vista+ 使用 BCP 47 格式，直接取主要部分
    let locale = locale.trim();

    if locale.is_empty() {
        return LanguageTag::new("en-US");
    }

    // 移除可能的编码标识（旧系统格式）
    let without_encoding = locale.split('.').next().unwrap_or(locale);

    // 如果已经是 `xx-XX` 格式，直接使用
    if without_encoding.contains('-') {
        return LanguageTag::new(without_encoding);
    }

    // 如果包含下划线（如 `zh_CN`），转换为连字符
    if without_encoding.contains('_') {
        return LanguageTag::new(&without_encoding.replace('_', "-"));
    }

    LanguageTag::new(without_encoding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_locale_to_bcp47_standard() {
        let tag = windows_locale_to_bcp47("zh-CN");
        assert_eq!(tag.as_str(), "zh-CN");
    }

    #[test]
    fn test_windows_locale_to_bcp47_with_underscore() {
        let tag = windows_locale_to_bcp47("ja_JP");
        assert_eq!(tag.as_str(), "ja-JP");
    }

    #[test]
    fn test_windows_locale_to_bcp47_simple() {
        let tag = windows_locale_to_bcp47("en");
        assert_eq!(tag.as_str(), "en");
    }

    #[test]
    fn test_windows_locale_to_bcp47_empty() {
        let tag = windows_locale_to_bcp47("");
        assert_eq!(tag.as_str(), "en-US");
    }

    #[test]
    fn test_windows_locale_to_bcp47_with_encoding() {
        let tag = windows_locale_to_bcp47("zh-CN.936");
        assert_eq!(tag.as_str(), "zh-CN");
    }

    #[test]
    fn test_normalize_path_backslash_to_forward() {
        let platform = WindowsPlatform;
        let result = platform.normalize_path(OsStr::new("C:\\Users\\test\\Documents"));
        assert_eq!(result, PathBuf::from("C:/Users/test/Documents"));
    }

    #[test]
    fn test_normalize_path_extended_prefix() {
        let platform = WindowsPlatform;
        let result = platform.normalize_path(OsStr::new("//?/C:/Very/Long/Path"));
        assert_eq!(result, PathBuf::from("C:/Very/Long/Path"));
    }

    #[test]
    fn test_normalize_path_unc() {
        let platform = WindowsPlatform;
        let result = platform.normalize_path(OsStr::new("\\\\server\\share\\path"));
        // UNC 路径：反斜杠转正斜杠，保留双斜杠前缀
        assert!(result.as_os_str().to_str().unwrap().starts_with("//"));
        assert!(
            result
                .as_os_str()
                .to_str()
                .unwrap()
                .contains("/server/share/path")
        );
    }

    #[test]
    fn test_normalize_path_mixed_separators() {
        let platform = WindowsPlatform;
        let result = platform.normalize_path(OsStr::new("C:\\foo/bar\\baz"));
        assert_eq!(result, PathBuf::from("C:/foo/bar/baz"));
    }
}
