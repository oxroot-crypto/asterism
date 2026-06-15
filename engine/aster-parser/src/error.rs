//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-parser/src/error.rs
//! 功能概述：解析器错误类型定义 — `ParseError` 结构体携带源码位置（行/列/偏移，
//!           1-based，与 Monaco Editor 一致）、中文错误描述、修复建议和出错行源码。
//!           所有错误信息使用中文，面向创作者友好。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - std::fmt（Display/Debug 实现）

use std::fmt;

/// 解析错误 — 携带位置信息和中文描述的结构化错误类型。
///
/// 用于将 pest 内部错误转换为面向创作者的友好格式。
/// 所有位置信息为 1-based，与 Monaco Editor 的 line/column 体系一致。
///
/// # 字段说明
///
/// - `location`: `(line, column, offset)` — 三值均为 1-based
///   - `line`: 行号（从 1 开始）
///   - `column`: 列号（从 1 开始，即第几个字符）
///   - `offset`: 从文件开始算起的字符偏移量（从 1 开始）
/// - `message`: 中文错误描述（如 `"第3行第12列：未闭合的字符串字面量"`）
/// - `hint`: 可选的修复建议（如 `"是否漏掉了右引号 \" ？"`）
/// - `context`: 出错行的源代码文本（方便快速定位）
///
/// # 示例
///
/// ```
/// use aster_parser::ParseError;
///
/// let err = ParseError::new(
///     (3, 12, 56),
///     "未闭合的字符串字面量".to_string(),
///     Some("是否漏掉了右引号 \" ？".to_string()),
///     "    sayori \"今天天气真好。".to_string(),
/// );
///
/// assert_eq!(err.line(), 3);
/// assert_eq!(err.column(), 12);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// 源码位置：(行号, 列号, 字符偏移)，均为 1-based
    pub location: (usize, usize, usize),

    /// 中文错误描述
    pub message: String,

    /// 可选的修复建议
    pub hint: Option<String>,

    /// 出错行的源代码文本
    pub context: String,
}

impl ParseError {
    /// 创建新的解析错误。
    ///
    /// # 参数
    ///
    /// - `location`: `(line, column, offset)` — 1-based 位置
    /// - `message`: 中文错误描述
    /// - `hint`: 可选的修复建议
    /// - `context`: 出错行的源代码文本
    pub fn new(
        location: (usize, usize, usize),
        message: String,
        hint: Option<String>,
        context: String,
    ) -> Self {
        Self {
            location,
            message,
            hint,
            context,
        }
    }

    /// 返回行号（1-based）。
    pub fn line(&self) -> usize {
        self.location.0
    }

    /// 返回到号（1-based）。
    pub fn column(&self) -> usize {
        self.location.1
    }

    /// 返回字符偏移量（1-based）。
    pub fn offset(&self) -> usize {
        self.location.2
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "第{}行第{}列：{}",
            self.location.0, self.location.1, self.message
        )?;

        if let Some(ref hint) = self.hint {
            write!(f, "\n  💡 提示：{}", hint)?;
        }

        write!(f, "\n  {} | {}", self.location.0, self.context)?;

        // 在出错位置下方显示 `^` 指示符
        if self.location.1 > 0 {
            let indent = " ".repeat(self.location.0.to_string().len());
            let arrow_pad = " ".repeat(self.location.1.saturating_sub(1));
            write!(f, "\n  {indent} | {arrow_pad}^")?;
        }

        Ok(())
    }
}

impl std::error::Error for ParseError {}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_display_format() {
        let err = ParseError::new(
            (3, 12, 56),
            "未闭合的字符串字面量".to_string(),
            Some("是否漏掉了右引号 \" ？".to_string()),
            "    sayori \"今天天气真好。".to_string(),
        );

        let display = format!("{}", err);
        // 验证包含关键信息
        assert!(display.contains("第3行第12列"));
        assert!(display.contains("未闭合的字符串字面量"));
        assert!(display.contains("提示"));
        assert!(display.contains("sayori"));
        // ^ 指示符在列 12（前面有 11 个空格 + 空格缩进）
        assert!(display.contains("^"));
    }

    #[test]
    fn test_parse_error_no_hint() {
        let err = ParseError::new(
            (1, 1, 1),
            "未知的语法错误".to_string(),
            None,
            "".to_string(),
        );

        let display = format!("{}", err);
        assert!(!display.contains("提示"));
    }

    #[test]
    fn test_parse_error_line_column_accessors() {
        let err = ParseError::new((10, 5, 100), "测试".to_string(), None, "test".to_string());

        assert_eq!(err.line(), 10);
        assert_eq!(err.column(), 5);
        assert_eq!(err.offset(), 100);
    }
}
