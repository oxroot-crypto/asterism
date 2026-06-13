//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-platform/src/macos.rs
//! 功能概述：macOS 平台的 `Platform` trait 实现。使用 `~/Library/Application Support/`
//!           作为应用数据根目录，通过 `AppleLocale` 偏好设置或环境变量 `LANG` 检测
//!           系统语言，使用 `/tmp` 下的文件锁实现单实例控制。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - super::platform（Platform trait / PlatformError / LanguageTag / 辅助函数）
//! - 不依赖任何 macOS 系统 framework（无 FFI），全部通过环境变量和标准文件 API 实现

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Child;

use super::platform::{
    LanguageTag, Platform, PlatformError, ensure_dir, home_dir, normalize_path_string,
};

/// macOS 平台的 `Platform` 实现。
///
/// 遵循 Apple 规范：
/// - **应用数据**: `~/Library/Application Support/`
/// - **临时文件**: `/tmp`
/// - **语言检测**: `AppleLocale` → `LANG` → 默认 `en-US`
///
/// 当前实现不依赖任何 macOS 系统 framework（Foundation/CoreFoundation），
/// 全部通过标准 API 实现，确保跨平台开发时可在非 macOS 上编译（仅不运行）。
#[derive(Debug, Clone, Default)]
pub struct MacOSPlatform;

impl Platform for MacOSPlatform {
    fn user_config_dir(&self) -> PathBuf {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let config_dir = home
            .join("Library")
            .join("Application Support")
            .join("com.asterism.engine");
        let _ = ensure_dir(&config_dir);
        config_dir
    }

    fn default_save_dir(&self, game_name: &str) -> PathBuf {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let save_dir = home
            .join("Library")
            .join("Application Support")
            .join(game_name)
            .join("saves");
        let _ = ensure_dir(&save_dir);
        save_dir
    }

    fn normalize_path(&self, raw: &OsStr) -> PathBuf {
        // macOS 路径原生使用正斜杠，直接进行字符串规范化即可
        match raw.to_str() {
            Some(s) => PathBuf::from(normalize_path_string(s)),
            None => PathBuf::from(raw),
        }
    }

    fn clipboard_copy(&self, _text: &str) {
        // Phase 1 存根：不执行实际操作
    }

    fn clipboard_paste(&self) -> Option<String> {
        // Phase 1 存根：始终返回 None
        None
    }

    fn system_language(&self) -> LanguageTag {
        // 优先级：
        // 1. AppleLocale（macOS 偏好设置中的语言，通过 `defaults read` 获取）
        //    格式示例：`zh-Hans-CN`、`ja-JP`
        // 2. LANG 环境变量
        // 3. 默认 "en-US"

        // 尝试读取 AppleLocale（通过 NSUserDefaults 的 shell 接口）
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
            && output.status.success()
        {
            let locale = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !locale.is_empty() {
                return macos_locale_to_bcp47(&locale);
            }
        }

        // 尝试读取 AppleLanguages（首选语言列表，取第一个）
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLanguages"])
            .output()
            && output.status.success()
        {
            let raw = String::from_utf8_lossy(&output.stdout);
            // AppleLanguages 输出格式类似：`(    "zh-Hans-CN",    "en-CN",    ...)`
            // 提取第一个引号中的内容
            if let Some(first_lang) = raw
                .trim()
                .trim_start_matches('(')
                .split(',')
                .next()
                .and_then(|s| s.trim().trim_matches('"').strip_suffix('"'))
                .or_else(|| {
                    raw.trim()
                        .trim_start_matches('(')
                        .split(',')
                        .next()
                        .map(|s| s.trim().trim_matches('"'))
                })
                && !first_lang.is_empty()
            {
                return macos_locale_to_bcp47(first_lang);
            }
        }

        // 回退到环境变量
        for var in &["LANG", "LC_ALL", "LC_MESSAGES"] {
            if let Ok(locale) = std::env::var(var) {
                let locale = locale.trim();
                if !locale.is_empty() {
                    return macos_locale_to_bcp47(locale);
                }
            }
        }

        LanguageTag::new("en-US")
    }

    fn try_acquire_single_instance(&self, app_id: &str) -> bool {
        let lock_path = PathBuf::from("/tmp").join(format!("asterism_{app_id}.lock"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
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

/// 将 macOS 风格的 locale 字符串转换为 BCP 47 语言标签。
///
/// 处理以下格式：
/// - `zh-Hans-CN` → `zh-CN`（Apple 语言描述符，移除脚本变体）
/// - `zh-Hans` → `zh`（无地区时仅保留语言）
/// - `ja_JP.UTF-8` → `ja-JP`（Unix locale 风格）
/// - `en-US` → `en-US`（已是 BCP 47）
/// - `C` / `POSIX` → `en-US`
///
/// # 参数
/// - `locale`: macOS locale 字符串
///
/// # 返回值
/// BCP 47 格式的语言标签
fn macos_locale_to_bcp47(locale: &str) -> LanguageTag {
    if locale == "C" || locale == "POSIX" {
        return LanguageTag::new("en-US");
    }

    // 移除编码后缀
    let without_encoding = locale.split('.').next().unwrap_or(locale);

    // 分割为片段
    let parts: Vec<&str> = without_encoding.split(['_', '-']).collect();

    if parts.is_empty() {
        return LanguageTag::new("en-US");
    }

    // Apple 语言标签可能包含脚本变体（如 Hans、Hant、Latn）
    // BCP 47 格式：language[-script][-region]
    // 对于引擎的用途，language-region 即可
    let language = parts[0];

    // 查找地区码（2 个大写字母）
    let region_str = parts
        .iter()
        .skip(1)
        .find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_uppercase()));

    match region_str {
        Some(region) => LanguageTag::new(&format!("{language}-{region}")),
        None => LanguageTag::new(language),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_locale_to_bcp47_apple_format() {
        let tag = macos_locale_to_bcp47("zh-Hans-CN");
        assert_eq!(tag.as_str(), "zh-CN");
    }

    #[test]
    fn test_macos_locale_to_bcp47_apple_no_region() {
        let tag = macos_locale_to_bcp47("zh-Hans");
        assert_eq!(tag.as_str(), "zh");
    }

    #[test]
    fn test_macos_locale_to_bcp47_unix_format() {
        let tag = macos_locale_to_bcp47("ja_JP.UTF-8");
        assert_eq!(tag.as_str(), "ja-JP");
    }

    #[test]
    fn test_macos_locale_to_bcp47_already_bcp47() {
        let tag = macos_locale_to_bcp47("en-US");
        assert_eq!(tag.as_str(), "en-US");
    }

    #[test]
    fn test_macos_locale_to_bcp47_c_locale() {
        let tag = macos_locale_to_bcp47("C");
        assert_eq!(tag.as_str(), "en-US");
    }

    #[test]
    fn test_normalize_path_on_macos() {
        let platform = MacOSPlatform;
        let result = platform.normalize_path(OsStr::new("/Users/test/Documents"));
        assert_eq!(result, PathBuf::from("/Users/test/Documents"));
    }
}
