//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/src/error.rs
//! 功能概述：编译错误类型 — 定义 `CompileError` 结构体，携带源码位置、中文错误描述、
//!           和可选的修复建议。用于编译器在语义检查阶段报告错误。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - std::fmt（Display 实现）

use std::fmt;

/// 编译错误 — 语义分析阶段检测到的错误。
///
/// 与 `aster_parser::ParseError`（语法错误）互补：
/// - `ParseError`：脚本语法层面的错误（如缺少引号、缩进错误），pest 解析器捕获
/// - `CompileError`：语义层面的错误（如跳转到未定义标签、变量未声明），编译器捕获
///
/// # 设计说明
///
/// v0.1 阶段 SceneNode 不携带源码位置信息，因此 `line`/`column` 可能为 0
/// （表示位置未知）。`message` 中尽可能包含足够的上下文（节点类型、标签名等）
/// 帮助创作者定位问题。
///
/// # 示例
/// ```
/// use aster_compiler::CompileError;
///
/// let err = CompileError {
///     message: "跳转目标标签 'bad_end' 未定义".into(),
///     line: 0,
///     column: 0,
///     hint: Some("请检查标签名拼写，或在当前场景中添加 label \"bad_end\"".into()),
/// };
///
/// assert!(err.to_string().contains("bad_end"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    /// 中文错误描述（如 "跳转目标标签 'xxx' 未定义"）
    pub message: String,

    /// 出错位置行号（1-based），0 表示位置未知
    pub line: usize,

    /// 出错位置列号（1-based），0 表示位置未知
    pub column: usize,

    /// 可选的修复建议（中文），帮助创作者快速修复
    pub hint: Option<String>,
}

impl CompileError {
    /// 创建一个新的编译错误。
    ///
    /// # 参数
    /// - `message`：中文错误描述
    /// - `line`：行号（1-based，0 表示未知）
    /// - `column`：列号（1-based，0 表示未知）
    /// - `hint`：可选的修复建议
    pub fn new(
        message: impl Into<String>,
        line: usize,
        column: usize,
        hint: Option<impl Into<String>>,
    ) -> Self {
        CompileError {
            message: message.into(),
            line,
            column,
            hint: hint.map(|h| h.into()),
        }
    }

    /// 创建位置未知的编译错误（SceneNode 无 span 时的兜底方案）。
    ///
    /// # 参数
    /// - `message`：中文错误描述
    /// - `hint`：可选的修复建议
    pub fn without_position(message: impl Into<String>, hint: Option<impl Into<String>>) -> Self {
        Self::new(message, 0, 0, hint)
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 && self.column > 0 {
            write!(f, "第{}行第{}列：{}", self.line, self.column, self.message)?;
        } else if self.line > 0 {
            write!(f, "第{}行：{}", self.line, self.message)?;
        } else {
            write!(f, "{}", self.message)?;
        }

        if let Some(ref hint) = self.hint {
            write!(f, "\n  💡 修复建议：{}", hint)?;
        }

        Ok(())
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 CompileError 的 Display 格式。
    #[test]
    fn compile_error_display_format() {
        let err = CompileError {
            message: "跳转目标标签 'bad_end' 未定义".into(),
            line: 42,
            column: 5,
            hint: Some("请检查标签名拼写".into()),
        };

        let display = err.to_string();
        assert!(display.contains("第42行第5列"));
        assert!(display.contains("bad_end"));
        assert!(display.contains("💡 修复建议"));
        assert!(display.contains("请检查标签名拼写"));
    }

    /// 验证无位置的 CompileError 的 Display 格式。
    #[test]
    fn compile_error_display_without_position() {
        let err = CompileError::without_position(
            "变量 'score' 未声明",
            Some("请使用 set $score = 0 声明变量"),
        );

        let display = err.to_string();
        assert!(display.contains("score"));
        assert!(display.contains("💡 修复建议"));
        // 不应包含"第x行"格式
        assert!(!display.contains("第"));
    }

    /// 验证 CompileError::new 构造函数。
    #[test]
    fn compile_error_constructor() {
        let err = CompileError::new("测试错误", 10, 20, Some("测试建议"));
        assert_eq!(err.message, "测试错误");
        assert_eq!(err.line, 10);
        assert_eq!(err.column, 20);
        assert_eq!(err.hint, Some("测试建议".into()));
    }

    /// 验证 CompileError::without_position 构造函数。
    #[test]
    fn compile_error_without_position() {
        let err = CompileError::without_position("测试", None::<&str>);
        assert_eq!(err.line, 0);
        assert_eq!(err.column, 0);
        assert_eq!(err.hint, None);
    }
}
