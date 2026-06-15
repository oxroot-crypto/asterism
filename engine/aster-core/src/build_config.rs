//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/build_config.rs
//! 功能概述：项目构建配置类型 — 定义 `BuildConfig` / `CompileConfig` / `GlobPatterns` /
//!           `ArchiveConfig` 结构体，对应 `build.toml` 的四个 section。
//!           所有类型均派生 `Debug + Clone + Serialize + Deserialize + PartialEq`。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持，用于 TOML 读写）
//!
//! 对应文档：Architecture.md §5.2（build.toml 格式）

use serde::{Deserialize, Serialize};

/// 项目构建配置 — 对应 `build.toml` 的顶层结构。
///
/// 包含四个 section：
/// - `[compile]`：编译目标、优化开关、压缩选项
/// - `[include]`：构建时包含的资源 glob 模式
/// - `[exclude]`：构建时排除的资源 glob 模式
/// - `[archive]`：归档格式和加密选项
///
/// 当 `build.toml` 不存在时使用 `Default` 实现提供的合理默认值。
///
/// # 序列化
///
/// 通过 serde 派生支持 TOML 序列化/反序列化：
/// ```rust,no_run
/// # use aster_core::BuildConfig;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config: BuildConfig = toml::from_str(&std::fs::read_to_string("build.toml")?)?;
/// # Ok(())
/// # }
/// ```
///
/// # 示例
/// ```
/// use aster_core::BuildConfig;
/// let config = BuildConfig::default();
/// assert_eq!(config.compile.target, "asterbyte");
/// assert!(config.compile.optimize);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildConfig {
    /// 编译选项 — 对应 `[compile]` section
    #[serde(default)]
    pub compile: CompileConfig,

    /// 构建时包含的资源 glob 模式 — 对应 `[include]` section
    #[serde(default)]
    pub include: GlobPatterns,

    /// 构建时排除的资源 glob 模式 — 对应 `[exclude]` section
    #[serde(default)]
    pub exclude: GlobPatterns,

    /// 归档配置 — 对应 `[archive]` section
    #[serde(default)]
    pub archive: ArchiveConfig,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            compile: CompileConfig::default(),
            include: GlobPatterns::default(),
            exclude: GlobPatterns::default_exclude(),
            archive: ArchiveConfig::default(),
        }
    }
}

/// 编译配置 — 对应 `build.toml` 中的 `[compile]` section。
///
/// 控制脚本编译的目标格式、是否启用优化 Pass、是否压缩输出。
///
/// # 字段说明
///
/// | 字段 | 类型 | 默认值 | 说明 |
/// |------|------|--------|------|
/// | `target` | `String` | `"asterbyte"` | 编译目标：`"asterbyte"`（字节码）或 `"ast"`（调试用 AST） |
/// | `optimize` | `bool` | `true` | 是否启用 4 个优化 Pass（常量折叠/死代码消除/跳转合并/窥孔优化） |
/// | `minify` | `bool` | `true` | 是否去除字节码中的调试元数据（注释、原始行号映射等） |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompileConfig {
    /// 编译目标格式，默认 `"asterbyte"`
    #[serde(default = "default_compile_target")]
    pub target: String,

    /// 是否开启编译优化，默认 `true`
    #[serde(default = "default_true")]
    pub optimize: bool,

    /// 是否压缩输出，默认 `true`
    #[serde(default = "default_true")]
    pub minify: bool,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self {
            target: default_compile_target(),
            optimize: true,
            minify: true,
        }
    }
}

/// serde 默认值：编译目标为 `"asterbyte"`
fn default_compile_target() -> String {
    "asterbyte".to_string()
}

/// serde 默认值：布尔 `true`
const fn default_true() -> bool {
    true
}

/// Glob 模式列表 — 对应 `build.toml` 中的 `[include]` 和 `[exclude]` section。
///
/// 用于在构建时筛选资源文件。模式遵循 Unix shell 风格 glob 语法
///（与 `.gitignore` 兼容）。
///
/// # 示例
/// ```
/// use aster_core::GlobPatterns;
///
/// // 包含所有资源文件
/// let include = GlobPatterns {
///     patterns: vec![
///         "assets/**/*".into(),
///         "gui/**/*".into(),
///         "fonts/**/*".into(),
///     ],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobPatterns {
    /// glob 模式列表（相对于项目根目录）
    #[serde(default)]
    pub patterns: Vec<String>,
}

impl Default for GlobPatterns {
    /// 默认的构建包含模式：资源、GUI、字体目录下的所有文件
    fn default() -> Self {
        Self {
            patterns: vec![
                "assets/**/*".to_string(),
                "gui/**/*".to_string(),
                "fonts/**/*".to_string(),
            ],
        }
    }
}

impl GlobPatterns {
    /// 默认的构建排除模式：缓存、临时文件、备份文件
    fn default_exclude() -> Self {
        Self {
            patterns: vec![
                "**/.aster_cache/**".to_string(),
                "**/*.tmp".to_string(),
                "**/*.bak".to_string(),
            ],
        }
    }
}

/// 归档配置 — 对应 `build.toml` 中的 `[archive]` section。
///
/// 控制构建产物的打包格式和加密选项。
///
/// # 字段说明
///
/// | 字段 | 类型 | 默认值 | 说明 |
/// |------|------|--------|------|
/// | `format` | `String` | `"asterarchive"` | 归档格式：`"asterarchive"`（Asterism 专用格式）或 `"dir"`（目录） |
/// | `encrypt` | `bool` | `false` | 是否加密资源文件（AES-256-GCM） |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchiveConfig {
    /// 归档格式，默认 `"asterarchive"`
    #[serde(default = "default_archive_format")]
    pub format: String,

    /// 是否加密，默认 `false`
    #[serde(default)]
    pub encrypt: bool,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            format: default_archive_format(),
            encrypt: false,
        }
    }
}

/// serde 默认值：归档格式为 `"asterarchive"`
fn default_archive_format() -> String {
    "asterarchive".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 CompileConfig 的默认值
    #[test]
    fn compile_config_defaults() {
        let config = CompileConfig::default();
        assert_eq!(config.target, "asterbyte");
        assert!(config.optimize);
        assert!(config.minify);
    }

    /// 验证 CompileConfig 从 TOML 反序列化
    #[test]
    fn compile_config_toml_deserialize() {
        let toml_str = r#"
target = "ast"
optimize = false
minify = false
"#;
        let config: CompileConfig = toml::from_str(toml_str).expect("TOML 反序列化失败");
        assert_eq!(config.target, "ast");
        assert!(!config.optimize);
        assert!(!config.minify);
    }

    /// 验证 CompileConfig 缺失字段时使用默认值
    #[test]
    fn compile_config_partial_toml() {
        // 只提供 target，其他字段应使用默认值
        let toml_str = r#"target = "asterbyte""#;
        let config: CompileConfig = toml::from_str(toml_str).expect("TOML 反序列化失败");
        assert_eq!(config.target, "asterbyte");
        assert!(config.optimize); // 默认 true
        assert!(config.minify); // 默认 true
    }

    /// 验证 GlobPatterns 的默认值
    #[test]
    fn glob_patterns_default() {
        let patterns = GlobPatterns::default();
        assert_eq!(patterns.patterns.len(), 3);
        assert!(patterns.patterns.contains(&"assets/**/*".to_string()));
        assert!(patterns.patterns.contains(&"gui/**/*".to_string()));
        assert!(patterns.patterns.contains(&"fonts/**/*".to_string()));
    }

    /// 验证 GlobPatterns 排除默认值
    #[test]
    fn glob_patterns_default_exclude() {
        let patterns = GlobPatterns::default_exclude();
        assert!(!patterns.patterns.is_empty());
        assert!(patterns.patterns.contains(&"**/*.tmp".to_string()));
    }

    /// 验证 ArchiveConfig 的默认值
    #[test]
    fn archive_config_defaults() {
        let config = ArchiveConfig::default();
        assert_eq!(config.format, "asterarchive");
        assert!(!config.encrypt);
    }

    /// 验证 ArchiveConfig 从 TOML 反序列化
    #[test]
    fn archive_config_toml_deserialize() {
        let toml_str = r#"
format = "dir"
encrypt = true
"#;
        let config: ArchiveConfig = toml::from_str(toml_str).expect("TOML 反序列化失败");
        assert_eq!(config.format, "dir");
        assert!(config.encrypt);
    }

    /// 验证 BuildConfig 的完整默认值
    #[test]
    fn build_config_defaults() {
        let config = BuildConfig::default();
        assert_eq!(config.compile.target, "asterbyte");
        assert!(config.compile.optimize);
        assert!(config.compile.minify);
        assert_eq!(config.include.patterns.len(), 3);
        assert!(!config.archive.encrypt);
    }

    /// 验证 BuildConfig 从完整 TOML 反序列化
    #[test]
    fn build_config_full_toml_roundtrip() {
        let toml_str = r#"
[compile]
target = "asterbyte"
optimize = true
minify = true

[include]
patterns = ["assets/**/*", "gui/**/*", "fonts/**/*"]

[exclude]
patterns = ["**/.aster_cache/**", "**/*.tmp"]

[archive]
format = "asterarchive"
encrypt = false
"#;
        let config: BuildConfig = toml::from_str(toml_str).expect("TOML 反序列化失败");

        // 序列化回去
        let restored_toml = toml::to_string(&config).expect("TOML 序列化失败");
        let restored: BuildConfig = toml::from_str(&restored_toml).expect("TOML 再次反序列化失败");

        assert_eq!(config.compile.target, restored.compile.target);
        assert_eq!(config.compile.optimize, restored.compile.optimize);
        assert_eq!(config.include.patterns, restored.include.patterns);
        assert_eq!(config.archive.format, restored.archive.format);
    }

    /// 验证 BuildConfig 空 TOML 时使用全部默认值
    #[test]
    fn build_config_empty_toml_uses_defaults() {
        let config: BuildConfig = toml::from_str("").expect("空 TOML 反序列化失败");
        assert_eq!(config.compile.target, "asterbyte");
        assert!(config.compile.optimize);
        assert!(config.compile.minify);
        assert_eq!(config.include.patterns.len(), 3);
        assert_eq!(config.exclude.patterns.len(), 3);
        assert_eq!(config.archive.format, "asterarchive");
        assert!(!config.archive.encrypt);
    }

    /// 验证 BuildConfig 部分字段的 TOML 反序列化
    #[test]
    fn build_config_partial_toml() {
        let toml_str = r#"
[compile]
target = "ast"
"#;
        let config: BuildConfig = toml::from_str(toml_str).expect("TOML 反序列化失败");
        // 显式设置的字段
        assert_eq!(config.compile.target, "ast");
        // 未设置的字段使用默认值
        assert!(config.compile.optimize);
        assert!(config.compile.minify);
        assert_eq!(config.include.patterns.len(), 3);
        assert_eq!(config.archive.format, "asterarchive");
    }
}
