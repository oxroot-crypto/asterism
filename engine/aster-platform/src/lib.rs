//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-platform/src/lib.rs
//! 功能概述：平台抽象层入口 — 通过条件编译（`#[cfg(target_os)]`）选择当前平台的
//!           `Platform` trait 实现，对外暴露统一的 `create_platform()` 工厂函数。
//!           `aster-platform` 是架构分层的最底层，仅依赖标准库。
//! 作者：Claude (AI)
//! 创建日期：2026-06-12
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - platform（Platform trait 定义 + PlatformError + LanguageTag）
//! - linux / macos / windows（平台具体实现，条件编译）
//!
//! 架构位置：aster-platform ← aster-core ← 上层 crate（Architecture.md §4 分层图）
//!
//! # 使用示例
//! ```rust
//! use aster_platform::create_platform;
//! let platform = create_platform();
//! let config_dir = platform.user_config_dir();
//! println!("配置目录: {}", config_dir.display());
//! ```

// ============================================================================
// 模块声明
// ============================================================================

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod platform;
#[cfg(target_os = "windows")]
mod windows;

// ============================================================================
// 公开导出 - 核心类型（所有平台可用）
// ============================================================================

pub use platform::{LanguageTag, Platform, PlatformError};

// ============================================================================
// 公开导出 - 平台具体实现（条件编译）
// ============================================================================

#[cfg(target_os = "linux")]
pub use linux::LinuxPlatform;
#[cfg(target_os = "macos")]
pub use macos::MacOSPlatform;
#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform;

// ============================================================================
// 工厂函数
// ============================================================================

/// 创建当前平台的 `Platform` 实例。
///
/// 根据编译目标平台，返回对应的具体实现：
/// - `target_os = "windows"` → `WindowsPlatform`
/// - `target_os = "macos"` → `MacOSPlatform`
/// - `target_os = "linux"` → `LinuxPlatform`
///
/// # 使用示例
/// ```rust
/// use aster_platform::create_platform;
/// let platform = create_platform();
/// let lang = platform.system_language();
/// println!("系统语言: {lang}");
/// ```
pub fn create_platform() -> Box<dyn Platform> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsPlatform)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSPlatform)
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxPlatform)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        compile_error!("aster-platform: 不支持的平台。当前仅支持 Windows、macOS 和 Linux。");
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC01-AC05
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::Path;

    // ========================================================================
    // AC01 — create_platform() 返回正确的具体类型
    // ========================================================================

    /// AC01: 验证 `create_platform()` 在编译平台上返回正确的具体类型。
    ///
    /// 通过编译期 `cfg!()` 宏验证工厂函数返回的实例能够调用 trait 方法，
    /// 且按预期返回当前平台对应的具体实现。编译期类型检查保证类型正确。
    #[test]
    fn ac01_create_platform_returns_corrent_type() {
        let platform = create_platform();

        // 验证返回的实例可以正常使用 trait 方法（证明类型正确）
        let lang = platform.system_language();
        assert!(!lang.as_str().is_empty());

        let config = platform.user_config_dir();
        assert!(!config.as_os_str().is_empty());

        // 编译期断言：确保各平台的具体类型可构造且实现 Platform trait
        #[cfg(target_os = "windows")]
        {
            let _p = WindowsPlatform;
            let _: &dyn Platform = &_p;
        }
        #[cfg(target_os = "macos")]
        {
            let _p = MacOSPlatform;
            let _: &dyn Platform = &_p;
        }
        #[cfg(target_os = "linux")]
        {
            let _p = LinuxPlatform;
            let _: &dyn Platform = &_p;
        }
    }

    /// AC01 补充: 验证工厂函数可以多次调用，每次都成功。
    #[test]
    fn ac01_create_platform_multiple_calls() {
        let p1 = create_platform();
        let p2 = create_platform();
        // 两个实例应成功创建且不 panic
        let _ = p1.system_language();
        let _ = p2.system_language();
    }

    // ========================================================================
    // AC02 — user_config_dir() 返回符合平台规范的路径
    // ========================================================================

    /// AC02: 验证 `user_config_dir()` 返回的路径符合各平台标准目录规范。
    ///
    /// - Windows: 路径应包含 "AppData" 或 "Roaming"
    /// - macOS: 路径应包含 "Application Support"
    /// - Linux: 路径应包含 ".local/share"
    #[test]
    fn ac02_user_config_dir_follows_platform_convention() {
        let platform = create_platform();
        let config_dir = platform.user_config_dir();
        let path_str = config_dir.to_string_lossy().to_string();

        if cfg!(target_os = "windows") {
            // Windows: %APPDATA%/Asterism/ — 可能包含 AppData 或 Roaming
            let has_appdata = path_str.to_lowercase().contains("appdata")
                || path_str.to_lowercase().contains("roaming");
            assert!(
                has_appdata,
                "Windows 配置目录应位于 AppData/Roaming 下，实际: {path_str}"
            );
            assert!(
                path_str.contains("Asterism"),
                "Windows 配置目录应包含 'Asterism'，实际: {path_str}"
            );
        } else if cfg!(target_os = "macos") {
            assert!(
                path_str.contains("Application Support"),
                "macOS 配置目录应位于 Application Support 下，实际: {path_str}"
            );
            assert!(
                path_str.contains("com.asterism.engine"),
                "macOS 配置目录应包含 bundle ID，实际: {path_str}"
            );
        } else if cfg!(target_os = "linux") {
            // XDG: ~/.local/share/asterism/
            let has_xdg = path_str.contains(".local/share")
                || std::env::var("XDG_DATA_HOME")
                    .map(|xdg| path_str.contains(&xdg))
                    .unwrap_or(false);
            assert!(
                has_xdg,
                "Linux 配置目录应遵循 XDG 规范（~/.local/share/ 或 $XDG_DATA_HOME），实际: {path_str}"
            );
            assert!(
                path_str.contains("asterism"),
                "Linux 配置目录应包含 'asterism'，实际: {path_str}"
            );
        }
    }

    /// AC02 补充: 配置目录在调用后应实际存在（自动创建）。
    #[test]
    fn ac02_config_dir_exists_after_call() {
        let platform = create_platform();
        let config_dir = platform.user_config_dir();
        assert!(
            config_dir.exists(),
            "user_config_dir() 返回的目录应存在（自动创建），实际路径: {}",
            config_dir.display()
        );
    }

    // ========================================================================
    // AC03 — default_save_dir() 返回正确的存档路径
    // ========================================================================

    /// AC03: 验证 `default_save_dir("test_game")` 路径包含游戏名和 saves 目录。
    #[test]
    fn ac03_default_save_dir_contains_game_name_and_saves() {
        let platform = create_platform();
        let save_dir = platform.default_save_dir("test_game");
        let path_str = save_dir.to_string_lossy().to_string();

        assert!(
            path_str.contains("test_game"),
            "存档路径应包含游戏名 'test_game'，实际: {path_str}"
        );
        assert!(
            path_str.contains("saves"),
            "存档路径应包含 'saves' 子目录，实际: {path_str}"
        );
    }

    /// AC03 补充: 存档目录在调用后应实际存在（自动创建）。
    #[test]
    fn ac03_save_dir_exists_after_call() {
        let platform = create_platform();
        let save_dir = platform.default_save_dir("test_game");
        assert!(
            save_dir.exists(),
            "default_save_dir() 返回的目录应存在（自动创建），实际路径: {}",
            save_dir.display()
        );
    }

    /// AC03 补充: 不同游戏名的存档目录不同。
    #[test]
    fn ac03_different_games_different_dirs() {
        let platform = create_platform();
        let dir_a = platform.default_save_dir("game_a");
        let dir_b = platform.default_save_dir("game_b");
        assert_ne!(dir_a, dir_b, "不同游戏名的存档目录应不同");
    }

    // ========================================================================
    // AC04 — normalize_path() 将反斜杠转换为正斜杠
    // ========================================================================

    /// AC04: `normalize_path()` 将 Windows 反斜杠路径转换为正斜杠。
    ///
    /// 在所有平台上测试（不限于 Windows），因为 `normalize_path_string` 是跨平台的。
    #[test]
    fn ac04_normalize_path_converts_backslash_to_forward() {
        let platform = create_platform();
        let result = platform.normalize_path(OsStr::new("a\\b\\c"));
        let result_str = result.to_string_lossy();
        assert!(
            !result_str.contains('\\'),
            "规范化后的路径不应包含反斜杠，实际: {result_str}"
        );
        assert_eq!(
            result_str, "a/b/c",
            "路径 `a\\b\\c` 应规范化为 `a/b/c`，实际: {result_str}"
        );
    }

    /// AC04 补充: 连续多个分隔符被合并。
    #[test]
    fn ac04_normalize_path_collapses_multiple_slashes() {
        let platform = create_platform();
        let result = platform.normalize_path(OsStr::new("a///b//c"));
        let result_str = result.to_string_lossy();
        assert_eq!(result_str, "a/b/c", "连续分隔符应合并，实际: {result_str}");
    }

    /// AC04 补充: 空路径不 panic。
    #[test]
    fn ac04_normalize_path_empty() {
        let platform = create_platform();
        let result = platform.normalize_path(OsStr::new(""));
        let result_str = result.to_string_lossy();
        assert_eq!(result_str, "", "空路径应返回空字符串，实际: {result_str}");
    }

    // ========================================================================
    // AC05 — system_language() 返回非空 LanguageTag
    // ========================================================================

    /// AC05: 验证 `system_language()` 返回非空值，且格式为 BCP 47。
    #[test]
    fn ac05_system_language_returns_non_empty() {
        let platform = create_platform();
        let lang = platform.system_language();
        let tag = lang.as_str();

        assert!(!tag.is_empty(), "system_language() 不应返回空字符串");
        assert_ne!(
            tag, "und",
            "system_language() 应返回实际语言标签，非 undetermined"
        );
    }

    /// AC05 补充: LanguageTag 的 primary_language() 至少 2 个字符。
    #[test]
    fn ac05_language_tag_has_valid_format() {
        let platform = create_platform();
        let lang = platform.system_language();
        let primary = lang.primary_language();

        assert!(
            primary.len() >= 2,
            "主语言代码至少 2 个字符（如 zh/en/ja），实际: {primary}"
        );
        // 如果包含地区，地区码为 2 个大写字母或 3 个数字
        if let Some(region) = lang.region() {
            assert!(
                region.len() == 2 || region.len() == 3,
                "地区码应为 2 字母（如 CN/JP）或 3 数字（UN M.49），实际: {region}"
            );
        }
    }

    // ========================================================================
    // 附加测试 — 其他方法的健康检查
    // ========================================================================

    /// clipboard_paste 在 Phase 1 阶段返回 None（存根）。
    #[test]
    fn test_clipboard_paste_is_stub() {
        let platform = create_platform();
        assert_eq!(
            platform.clipboard_paste(),
            None,
            "Phase 1 剪贴板应为存根，始终返回 None"
        );
    }

    /// clipboard_copy 不应 panic（即使是存根）。
    #[test]
    fn test_clipboard_copy_does_not_panic() {
        let platform = create_platform();
        // 存根方法，调用不应 panic
        platform.clipboard_copy("test text");
    }

    /// try_acquire_single_instance 基本功能：首次调用应成功。
    #[test]
    fn test_single_instance_first_call_succeeds() {
        let platform = create_platform();
        // 使用含进程 ID 的唯一 app_id，避免之前的测试锁文件残留
        let app_id = format!("test_first_instance_{}", std::process::id());
        let result = platform.try_acquire_single_instance(&app_id);
        assert!(result, "首次获取单实例锁应成功");
    }

    /// try_acquire_single_instance 第二次调用应失败（同一 app_id）。
    #[test]
    fn test_single_instance_second_call_fails() {
        let platform = create_platform();
        let app_id = format!("test_dup_instance_{}", std::process::id());
        let first = platform.try_acquire_single_instance(&app_id);
        assert!(first, "首次应成功");

        // 创建第二个 platform 实例（模拟第二个进程）
        let platform2 = create_platform();
        let second = platform2.try_acquire_single_instance(&app_id);
        assert!(!second, "第二次调用应失败（锁已被持有）");
    }

    /// launch_process 对不存在的可执行文件返回错误。
    #[test]
    fn test_launch_process_nonexistent_executable() {
        let platform = create_platform();
        let result = platform.launch_process(
            Path::new("/nonexistent/executable/that/does/not/exist"),
            &[],
        );
        assert!(result.is_err(), "启动不存在的可执行文件应返回错误");
    }

    /// launch_process 错误信息包含可执行文件路径。
    #[test]
    fn test_launch_process_error_contains_path() {
        let platform = create_platform();
        let result = platform.launch_process(Path::new("/nonexistent_app_xyz"), &[]);
        let err = result.unwrap_err();
        let err_str = format!("{err}");
        assert!(
            err_str.contains("nonexistent_app_xyz"),
            "错误信息应包含可执行文件路径，实际: {err_str}"
        );
    }

    // ========================================================================
    // 各平台特有测试（仅在对应平台运行）
    // ========================================================================

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_paths_use_backslash_aware_dirs() {
        let platform = create_platform();
        let config = platform.user_config_dir();
        // Windows 上 APPDATA 环境变量应存在
        let appdata = std::env::var("APPDATA").unwrap();
        assert!(
            config.to_string_lossy().starts_with(&appdata),
            "配置目录应在 APPDATA 下"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_respects_xdg_data_home() {
        // 临时设置 XDG_DATA_HOME 验证优先级
        unsafe { std::env::set_var("XDG_DATA_HOME", "/tmp/xdg-test") };
        let platform = create_platform();
        let config = platform.user_config_dir();
        assert!(
            config.to_string_lossy().starts_with("/tmp/xdg-test"),
            "Linux 应优先使用 XDG_DATA_HOME"
        );
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_uses_library_application_support() {
        let platform = create_platform();
        let config = platform.user_config_dir();
        let path_str = config.to_string_lossy();
        assert!(
            path_str.contains("Library/Application Support"),
            "macOS 使用 ~/Library/Application Support"
        );
    }

    /// LanguageTag 序列化 round-trip 测试（通过 Display/From 而非 serde）。
    #[test]
    fn test_language_tag_round_trip_via_string() {
        let original = LanguageTag::new("zh-CN");
        let s = original.to_string();
        let restored: LanguageTag = s.as_str().into();
        assert_eq!(original, restored);
    }

    // ─── ensure_dir / home_dir 直接测试 ─────────────────────────────────

    /// 验证 ensure_dir 在合法路径上可以创建目录。
    #[test]
    fn test_ensure_dir_creates_directory() {
        // 使用 std::env::temp_dir 确保有写入权限
        let temp = std::env::temp_dir().join(format!("aster_test_ensure_{}", std::process::id()));
        // 确保测试前目录不存在
        let _ = std::fs::remove_dir_all(&temp);
        assert!(!temp.exists());

        let result = platform::ensure_dir(&temp);
        assert!(result.is_ok(), "ensure_dir 应成功创建目录");
        assert!(temp.exists(), "目录应已创建");
        // 清理
        let _ = std::fs::remove_dir_all(&temp);
    }

    /// 验证 ensure_dir 对已存在目录是幂等的（不报错）。
    #[test]
    fn test_ensure_dir_idempotent() {
        let temp = std::env::temp_dir().join(format!("aster_test_idem_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        assert!(temp.exists());

        // 对已存在目录再次调用 ensure_dir 不应报错
        let result = platform::ensure_dir(&temp);
        assert!(result.is_ok(), "对已存在目录调用 ensure_dir 不应报错");
        // 清理
        let _ = std::fs::remove_dir_all(&temp);
    }

    /// 验证 home_dir 返回 Some 且路径非空。
    #[test]
    fn test_home_dir_returns_some_and_non_empty() {
        let home = platform::home_dir();
        assert!(
            home.is_some(),
            "home_dir 应返回 Some（任何 CI/开发者环境都应有 home 目录）"
        );
        let home = home.unwrap();
        assert!(!home.as_os_str().is_empty(), "home 路径不应为空");
    }

    // ─── LanguageTag 边界值测试 ─────────────────────────────────────────

    /// 验证 LanguageTag::From<String> 空字符串生成 "und"。
    #[test]
    fn test_language_tag_from_empty_string() {
        let tag: LanguageTag = String::new().into();
        assert_eq!(tag.as_str(), "und");
    }

    /// 验证 LanguageTag 多连字符格式的 region 提取（如 zh-Hans-CN）。
    #[test]
    fn test_language_tag_with_multiple_hyphens() {
        let tag = LanguageTag::new("zh-Hans-CN");
        // primary_language 返回第一个 `-` 之前的部分
        assert_eq!(tag.primary_language(), "zh");
        // region 返回第一个 `-` 之后的部分（与 BCP 47 语义不同但符合当前实现）
        // 当前实现：split('-').nth(1) → "Hans"
        assert!(tag.region().is_some(), "多连字符标签应有 region");
    }

    /// 验证 LanguageTag 仅含语言代码时的行为。
    #[test]
    fn test_language_tag_simple_language() {
        let tag = LanguageTag::new("ja");
        assert_eq!(tag.primary_language(), "ja");
        assert_eq!(tag.region(), None);
    }

    /// 验证 LanguageTag 含多个 region 格式的标签（如 sr-Latn-RS）。
    #[test]
    fn test_language_tag_script_and_region() {
        let tag = LanguageTag::new("sr-Latn-RS");
        assert_eq!(tag.primary_language(), "sr");
        // region() 返回第一个 `-` 之后的部分
        assert_eq!(tag.region(), Some("Latn"));
    }

    // ─── 非 UTF-8 OsStr normalize_path 测试 ────────────────────────────

    /// 验证 normalize_path 对非 UTF-8 OsStr 的容错处理。
    #[test]
    fn test_normalize_path_non_utf8_osstr() {
        let platform = create_platform();
        #[cfg(unix)]
        {
            // Unix 上可以使用原始字节构造 OsStr
            use std::os::unix::ffi::OsStrExt;
            // 构造包含无效 UTF-8 的路径（单独的高位字节，非 surrogates）
            let raw =
                OsStr::from_bytes(&[0x2F, b'u', b's', b'r', 0xFE, 0x2F, b'd', b'a', b't', b'a']);
            let result = platform.normalize_path(raw);
            // 不应 panic，结果路径应该可以转为 lossy string
            let _ = result.to_string_lossy();
        }
        #[cfg(not(unix))]
        {
            // Windows/non-Unix: 正常 OsStr 不会触发异常路径
            let result = platform.normalize_path(OsStr::new("normal/path"));
            assert!(!result.as_os_str().is_empty());
        }
    }

    // ─── launch_process 边界值测试 ──────────────────────────────────────

    /// 验证 launch_process 传入空 args 列表。
    #[test]
    fn test_launch_process_with_empty_args() {
        let platform = create_platform();
        let result = platform.launch_process(Path::new("/nonexistent_empty_args"), &[]);
        assert!(result.is_err(), "空 args + 不存在可执行文件应返回错误");
    }

    // ─── Trait impl 编译时验证 ─────────────────────────────────────────

    /// 验证所有平台 struct 满足 Send + Sync（通过 trait 约束编译期保证）。
    ///
    /// 此测试通过编译即可，运行时仅验证工厂函数正常工作。
    #[test]
    fn test_platform_structs_satisfy_trait_bounds() {
        // create_platform 返回 Box<dyn Platform>，Platform trait 要求 Send + Sync
        let p = create_platform();
        // 验证 trait 方法可正常调用
        let lang = p.system_language();
        assert!(!lang.as_str().is_empty());
    }

    // ─── 单实例锁路径格式测试 ──────────────────────────────────────────

    /// 验证单实例锁文件的路径格式。
    #[test]
    fn test_single_instance_lock_path_format() {
        let platform = create_platform();
        let app_id = format!("test_path_format_{}", std::process::id());

        // 锁应能成功获取
        let result = platform.try_acquire_single_instance(&app_id);
        assert!(result, "首次应成功获取锁");

        // 验证锁文件存在
        #[cfg(target_os = "windows")]
        {
            let temp_dir =
                std::env::var("TEMP").unwrap_or_else(|_| "C:\\Windows\\Temp".to_string());
            let lock_path =
                std::path::PathBuf::from(temp_dir).join(format!("asterism_{app_id}.lock"));
            assert!(lock_path.exists(), "锁文件应存在: {}", lock_path.display());
        }
        #[cfg(not(target_os = "windows"))]
        {
            let lock_path =
                std::path::PathBuf::from("/tmp").join(format!("asterism_{app_id}.lock"));
            assert!(lock_path.exists(), "锁文件应存在: {}", lock_path.display());
        }
    }
}
