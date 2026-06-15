//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/error.rs
//! 功能概述：运行时错误类型 — 定义 `RuntimeError` 枚举，涵盖项目加载过程中的
//!            IO 错误、TOML 解析错误、文件缺失、场景引用无效等场景。
//!           使用 `thiserror` 派生宏统一实现 `Display` + `std::error::Error`。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - thiserror（错误派生宏）
//! - std::io（IO 错误）
//! - toml（TOML 解析错误）
//!
//! 对应文档：Phase-1-Tasks.md PH1-T15（GameLoader 错误处理）

use thiserror::Error;

/// 运行时错误 — 涵盖项目加载、场景管理的所有可恢复错误。
///
/// 使用 `thiserror` 派生宏自动生成 `Display` 和 `std::error::Error` 实现。
/// 包含 `#[from]` 属性支持 `?` 操作符无缝传播 IO 和 TOML 解析错误。
///
/// # 变体说明
///
/// | 变体 | 说明 |
/// |------|------|
/// | `Io` | 文件系统 IO 错误（读取失败、权限不足等） |
/// | `TomlParse` | TOML 格式解析错误 |
/// | `ProjectNotFound` | 项目根目录下未找到 `aster.toml` |
/// | `EntrySceneNotFound` | `aster.toml` 指定的入口场景在场景清单中不存在 |
/// | `CharacterParseError` | `.asterchar` 角色文件解析失败 |
///
/// # 示例
/// ```
/// use aster_runtime::RuntimeError;
///
/// let err = RuntimeError::ProjectNotFound("/bad/path".into());
/// assert!(err.to_string().contains("aster.toml"));
/// ```
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// IO 错误 — 文件读取失败或权限不足
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),

    /// TOML 解析错误 — `aster.toml` 或 `.asterchar` 文件内容格式错误
    #[error("TOML 解析错误：{0}")]
    TomlParse(#[from] toml::de::Error),

    /// 项目根目录验证失败 — 指定路径下不存在 `aster.toml` 文件
    #[error("未找到 aster.toml 文件，请确认项目根目录正确：{0}")]
    ProjectNotFound(String),

    /// 入口场景不在场景清单中 — `aster.toml` 中 `entry_scene` 指定的场景
    /// 未在 `scripts/` 目录下发现对应的 `.aster` 文件
    #[error(
        "入口场景 '{entry_scene}' 在场景清单中不存在，请确认 scripts/ 目录下存在对应的 .aster 文件"
    )]
    EntrySceneNotFound {
        /// `aster.toml` 中指定的入口场景 ID
        entry_scene: String,
    },

    /// 角色文件解析失败 — `.asterchar` 文件内容不符合预期格式
    #[error("角色文件解析失败：{path} — {message}")]
    CharacterParseError {
        /// 出错的 `.asterchar` 文件路径
        path: String,
        /// 具体错误描述
        message: String,
    },

    /// 场景未找到 — 指定的场景 ID 在 GameContext 中不存在
    ///
    /// 可能原因：场景未编译、场景 ID 拼写错误、或对应 `.aster` 文件不存在
    #[error("场景 '{scene_id}' 不存在或未编译，请确认场景 ID 正确且已在 build.toml 中注册")]
    SceneNotFound {
        /// 请求的场景标识符
        scene_id: String,
    },

    /// 当前无已加载场景 — 在 load_scene 前调用了 update/on_click 等需要场景上下文的方法
    #[error("当前没有已加载的场景，请先调用 load_scene() 加载场景")]
    SceneNotLoaded,

    /// 状态错误 — 在当前状态下不允许执行该操作
    #[error("操作状态错误：期望 {expected} 状态，当前为 {actual} 状态")]
    InvalidState {
        /// 期望的状态描述
        expected: String,
        /// 实际的状态描述
        actual: String,
    },

    /// 菜单选择索引越界 — select_choice 传入的索引超出可用选项范围
    #[error("菜单选择索引 {index} 超出范围（可用选项数：{max}）")]
    InvalidChoiceIndex {
        /// 请求的选择索引
        index: usize,
        /// 可用选项总数
        max: usize,
    },

    /// VM 执行错误 — 字节码损坏、无限循环、或运行时异常
    #[error("VM 执行错误：{message}")]
    VmError {
        /// 错误描述
        message: String,
    },

    /// 场景解析错误 — `.aster` 源码解析失败
    #[error("场景 '{scene_id}' 解析失败：\n{}", messages.join("\n"))]
    SceneParseError {
        /// 出错的场景 ID
        scene_id: String,
        /// 错误消息列表（每个 ParseError 一行）
        messages: Vec<String>,
    },

    /// 场景编译错误 — AST→字节码编译失败
    #[error("编译失败：\n{}", messages.join("\n"))]
    CompileError {
        /// 错误消息列表（每个 CompileError 一行）
        messages: Vec<String>,
    },

    /// 事件循环创建失败 — winit EventLoop 创建失败
    #[error("事件循环创建失败：{message}")]
    EventLoopError {
        /// 错误描述
        message: String,
    },
}
