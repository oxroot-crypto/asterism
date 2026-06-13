//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-parser/src/parser.rs
//! 功能概述：pest 解析器入口 — 基于 `grammar.pest` 语法文件，通过 `#[derive(Parser)]`
//!           自动生成 PEG 解析器。对外提供 `parse_script()` 函数，输入 .aster 源码，
//!           输出 pest token 流（`pest::Pairs`）或结构化的 `ParseError` 列表。
//!           PH1-T05 接入 AstBuilder 后将改为返回 `Result<aster_core::Scene, Vec<ParseError>>`。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - pest（PEG 解析器运行时）
//! - pest_derive（编译期语法处理宏 `#[derive(Parser)]`）
//! - crate::error::ParseError（结构化错误类型）
//!
//! 解析流程：
//! ```text
//! .aster 源码 → AsterParser::parse(Rule::script, source)
//!             → Ok(pest::Pairs) → AstBuilder::build() → aster_core::Scene
//!             → Err → pest Error → Vec<ParseError>
//! ```

use pest::Parser;
use pest_derive::Parser;

use aster_core::Scene;

use crate::builder::AstBuilder;
use crate::error::ParseError;

/// pest 解析器 — 由 `grammar.pest` 文件在编译期自动生成。
///
/// `#[derive(Parser)]` 宏读取 `grammar.pest` 并生成：
/// - `Rule` 枚举（每个语法规则对应一个 variant）
/// - `AsterParser` 结构体（实现 `pest::Parser` trait）
///
/// 语法文件路径相对于 `src/` 目录。
#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct AsterParser;

/// 解析 .aster 脚本源码，返回编译就绪的 `Scene` AST。
///
/// 这是 parser 模块的唯一公共入口。
///
/// # 解析流程
/// 1. pest 解析：.aster 源码 → pest token 流
/// 2. AST 构建：pest token 流 → `aster_core::Scene`
///
/// # 参数
/// - `source`: .aster 脚本的完整源码文本（&str）
///
/// # 返回值
/// - `Ok(Scene)`: 解析成功，返回可直接供编译器消费的完整 Scene AST。
///   - 即使输入为空文件（仅含空白/注释），也返回 Ok（Scene 的 nodes 为空）
/// - `Err(Vec<ParseError>)`: 解析失败，返回一个或多个结构化错误
///   - 每个错误携带行号、列号、中文描述和修复建议
///
/// # 性能
/// - 10,000 行脚本解析 < 100ms（对应 NFR-PERF-011）
///
/// # 示例
/// ```rust,no_run
/// use aster_parser::parse_script;
///
/// let source = r#"scene "test" {
///     narration "Hello!"
/// }"#;
///
/// match parse_script(source) {
///     Ok(scene) => println!("解析成功，场景 '{}' 包含 {} 个节点", scene.id, scene.nodes.len()),
///     Err(errors) => {
///         for e in &errors {
///             eprintln!("{}", e);
///         }
///     }
/// }
/// ```
pub fn parse_script(source: &str) -> Result<Scene, Vec<ParseError>> {
    // 空输入（仅含空白/注释）视为合法：不产生 token 对，但也不报错
    // pest 的 SOI ~ EOI 对纯空白输入会解析为空 token 流，这是预期行为
    let pairs = AsterParser::parse(Rule::script, source).map_err(|pest_error| {
        // 将单个 pest 错误转换为我们的 ParseError 格式
        // 注：pest 的 PEG 解析在遇到第一个语法错误时即停止，
        // 因此每次解析失败只产生一个 pest Error
        let (line, col) = match &pest_error.line_col {
            pest::error::LineColLocation::Pos((line, col)) => (*line, *col),
            pest::error::LineColLocation::Span((line, col), _) => (*line, *col),
        };

        // 提取上下文：出错行（或附近）的源码
        let context = extract_error_context(source, line);

        let message = format_pest_error_message(&pest_error);

        let hint = generate_hint(&pest_error);

        vec![ParseError::new((line, col, 0), message, hint, context)]
    })?;

    // PH1-T05: 通过 AstBuilder 将 pest token 流转换为 Scene AST
    AstBuilder::build(pairs, source)
}

/// 从源码中提取出错行的文本（用于错误展示的 context 字段）。
///
/// # 参数
/// - `source`: 完整源码
/// - `line`: 1-based 行号
///
/// # 返回值
/// 出错行的文本（去除尾部换行符），如果行号超出范围则返回空字符串
fn extract_error_context(source: &str, line: usize) -> String {
    if line == 0 {
        return String::new();
    }
    source
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .to_string()
}

/// 将 pest 内部错误格式化为面向创作者的友好中文描述。
///
/// pest 的错误消息为英文，且技术性强（如 "expected X but found Y"），
/// 需要转换为创作者能理解的中文错误信息。
fn format_pest_error_message(error: &pest::error::Error<Rule>) -> String {
    // pest 的 Display 实现提供了格式化的错误消息
    // 我们对其进行中文增强
    let raw = format!("{}", error);

    // 常见模式转换
    if raw.contains("expected") {
        // 提取 "expected X" 的具体内容
        format!("语法错误：{}", raw.lines().next().unwrap_or(&raw))
    } else {
        format!("解析错误：{}", raw.lines().next().unwrap_or(&raw))
    }
}

/// 根据错误类型生成修复提示。
///
/// 识别常见错误模式并提供对应的修复建议。
fn generate_hint(error: &pest::error::Error<Rule>) -> Option<String> {
    let raw = format!("{}", error);

    if raw.contains("expected") {
        if raw.contains("\"") && !raw.contains("\\\"") {
            // 可能是字符串字面量相关问题
            Some("检查所有字符串是否用双引号 \" 正确包裹，并且字符串内容不跨行。".into())
        } else if raw.contains("{") || raw.contains("}") {
            Some("检查大括号 { } 是否成对出现，以及块内语句的缩进是否正确。".into())
        } else if raw.contains("$") {
            Some("变量引用格式为 $变量名，确认 $ 前缀后紧跟有效标识符。".into())
        } else {
            Some("检查关键字拼写是否正确，以及语法格式是否符合 .aster DSL 规范。".into())
        }
    } else {
        None
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── AC04: 空文件解析不 panic ──

    #[test]
    fn test_parse_empty_string_returns_ok() {
        // 空字符串应返回 Ok（无 token，但也不是错误）
        let result = parse_script("");
        assert!(result.is_ok(), "空文件解析不应报错");
    }

    #[test]
    fn test_parse_whitespace_only_returns_ok() {
        // 仅含空白和注释的文件
        let result = parse_script("  \n  \n  -- just a comment\n");
        assert!(result.is_ok(), "仅含空白/注释的文件不应报错");
    }

    // ── AC02: 非法语法返回错误含行号 ──

    #[test]
    fn test_unclosed_string_returns_error_with_line() {
        let source = "scene \"test\" {\n    sayori \"hello\n}";
        let result = parse_script(source);
        assert!(result.is_err(), "未闭合字符串应返回错误");

        let errors = result.unwrap_err();
        assert!(!errors.is_empty(), "错误列表不应为空");

        let first_error = &errors[0];
        assert!(
            first_error.line() > 0,
            "错误应包含有效行号，实际行号: {}",
            first_error.line()
        );
        assert!(!first_error.message.is_empty(), "错误应有描述消息");
    }

    // ── AC03: 注释被正确忽略 ──

    #[test]
    fn test_comments_are_ignored() {
        // 包含注释的合法脚本应成功解析
        let source = "scene \"test\" {\n    -- 这是一个注释\n    narration \"你好\"\n}";
        let result = parse_script(source);
        assert!(
            result.is_ok(),
            "包含注释的合法脚本应成功解析，但返回: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_multiple_comments_are_ignored() {
        let source = "-- 文件头注释\nscene \"test\" {\n    -- 行内注释\n    narration \"Hello\"\n    -- 行尾注释\n}";
        let result = parse_script(source);
        assert!(result.is_ok(), "多行注释不应影响解析");
    }

    // ── 基本语法解析 ──

    #[test]
    fn test_parse_minimal_scene() {
        let source = "scene \"minimal\" {\n}";
        let result = parse_script(source);
        assert!(result.is_ok(), "最小场景应成功解析");
    }

    #[test]
    fn test_parse_scene_with_description() {
        let source = "scene \"test\" {\n    description: \"这是一个测试场景\"\n    narration \"你好世界\"\n}";
        let result = parse_script(source);
        assert!(
            result.is_ok(),
            "含 description 的场景应成功解析，错误: {:?}",
            result.err()
        );
    }

    // ── 各 SceneNode 变体解析 ──

    #[test]
    fn test_parse_bg_stmt() {
        let source = "scene \"test\" {\n    bg \"bg_classroom\"\n}";
        assert!(parse_script(source).is_ok(), "bg 语句应成功解析");
    }

    #[test]
    fn test_parse_bg_with_transition() {
        let source = "scene \"test\" {\n    bg \"bg_sakura\" with fade(1000)\n}";
        assert!(parse_script(source).is_ok(), "bg + fade 转场应成功解析");
    }

    #[test]
    fn test_parse_bg_with_dissolve() {
        let source = "scene \"test\" {\n    bg \"bg_park\" with dissolve(1200)\n}";
        assert!(parse_script(source).is_ok(), "bg + dissolve 转场应成功解析");
    }

    #[test]
    fn test_parse_music_stmt() {
        let source = "scene \"test\" {\n    music \"bgm_test\"\n}";
        assert!(parse_script(source).is_ok(), "music 语句应成功解析");
    }

    #[test]
    fn test_parse_music_with_options() {
        let source = "scene \"test\" {\n    music \"bgm_title\" fade_in: 1.0 looping: false\n}";
        assert!(
            parse_script(source).is_ok(),
            "music + fade_in + looping 应成功解析"
        );
    }

    #[test]
    fn test_parse_stop_music() {
        let source = "scene \"test\" {\n    stop_music\n}";
        assert!(parse_script(source).is_ok(), "stop_music 应成功解析");
    }

    #[test]
    fn test_parse_stop_music_with_fade() {
        let source = "scene \"test\" {\n    stop_music fade_out: 1.5\n}";
        assert!(
            parse_script(source).is_ok(),
            "stop_music + fade_out 应成功解析"
        );
    }

    #[test]
    fn test_parse_se_stmt() {
        let source = "scene \"test\" {\n    se \"se_birds\"\n}";
        assert!(parse_script(source).is_ok(), "se 语句应成功解析");
    }

    #[test]
    fn test_parse_se_with_fade() {
        let source = "scene \"test\" {\n    se \"se_birds\" fade_in: 0.5\n}";
        assert!(parse_script(source).is_ok(), "se + fade_in 应成功解析");
    }

    #[test]
    fn test_parse_show_char_preset_positions() {
        for pos in &["left", "center", "right"] {
            let source = format!("scene \"test\" {{\n    show sayori at {}\n}}", pos);
            assert!(parse_script(&source).is_ok(), "show at {} 应成功解析", pos);
        }
    }

    #[test]
    fn test_parse_show_char_custom_position() {
        let source = "scene \"test\" {\n    show sayori at (0.25, 0.5)\n}";
        assert!(parse_script(source).is_ok(), "show + 自定义坐标应成功解析");
    }

    #[test]
    fn test_parse_show_char_with_emotion_and_transition() {
        let source =
            "scene \"test\" {\n    show sayori at center emotion: \"happy\" with fade(0.8)\n}";
        assert!(
            parse_script(source).is_ok(),
            "show + at + emotion + transition 应成功解析"
        );
    }

    #[test]
    fn test_parse_show_char_variable() {
        let source = "scene \"test\" {\n    show $char at center\n}";
        assert!(
            parse_script(source).is_ok(),
            "show + 变量 char_id 应成功解析"
        );
    }

    #[test]
    fn test_parse_emotion_stmt() {
        let source = "scene \"test\" {\n    emotion sayori \"smile\"\n}";
        assert!(parse_script(source).is_ok(), "emotion 语句应成功解析");
    }

    #[test]
    fn test_parse_emotion_with_transition() {
        let source = "scene \"test\" {\n    emotion sayori \"surprise\" with dissolve(0.3)\n}";
        assert!(
            parse_script(source).is_ok(),
            "emotion + transition 应成功解析"
        );
    }

    #[test]
    fn test_parse_move_char() {
        let source = "scene \"test\" {\n    move sayori to left with slide(left, 0.8)\n}";
        assert!(parse_script(source).is_ok(), "move 语句应成功解析");
    }

    #[test]
    fn test_parse_move_char_with_emotion() {
        let source = "scene \"test\" {\n    move sayori to center emotion: \"embarrassed\" with dissolve(0.5)\n}";
        assert!(parse_script(source).is_ok(), "move + emotion 应成功解析");
    }

    #[test]
    fn test_parse_hide_char() {
        let source = "scene \"test\" {\n    hide akane\n}";
        assert!(parse_script(source).is_ok(), "hide 语句应成功解析");
    }

    #[test]
    fn test_parse_hide_char_with_transition() {
        let source = "scene \"test\" {\n    hide sayori with fade(500)\n}";
        assert!(parse_script(source).is_ok(), "hide + transition 应成功解析");
    }

    #[test]
    fn test_parse_sprite_stmt() {
        let source = "scene \"test\" {\n    sprite \"ui/icon.png\" at (0.9, 0.05)\n}";
        assert!(parse_script(source).is_ok(), "sprite 语句应成功解析");
    }

    #[test]
    fn test_parse_sprite_with_options() {
        let source = "scene \"test\" {\n    sprite \"ui/icon.png\" at (0.5, 0.5) scale: 0.5 alpha: 0.7 with fade(1.0)\n}";
        assert!(
            parse_script(source).is_ok(),
            "sprite + scale + alpha + transition 应成功解析"
        );
    }

    #[test]
    fn test_parse_hide_sprite() {
        let source = "scene \"test\" {\n    usprite \"ui/icon.png\"\n}";
        assert!(parse_script(source).is_ok(), "usprite 应成功解析");
    }

    #[test]
    fn test_parse_hide_sprite_with_transition() {
        let source = "scene \"test\" {\n    usprite \"ui/icon.png\" with fade(500)\n}";
        assert!(
            parse_script(source).is_ok(),
            "usprite + transition 应成功解析"
        );
    }

    #[test]
    fn test_parse_dialogue() {
        let source = "scene \"test\" {\n    sayori \"你好\"\n}";
        assert!(parse_script(source).is_ok(), "dialogue 语句应成功解析");
    }

    #[test]
    fn test_parse_dialogue_with_voice() {
        let source = "scene \"test\" {\n    sayori \"你好\" voice: $voice_id\n}";
        assert!(parse_script(source).is_ok(), "dialogue + voice 应成功解析");
    }

    #[test]
    fn test_parse_dialogue_with_expression() {
        let source = "scene \"test\" {\n    akane \"好感度: \" + $affection\n}";
        assert!(
            parse_script(source).is_ok(),
            "dialogue + 字符串拼接应成功解析"
        );
    }

    #[test]
    fn test_parse_narration() {
        let source = "scene \"test\" {\n    narration \"春天来了\"\n}";
        assert!(parse_script(source).is_ok(), "narration 语句应成功解析");
    }

    #[test]
    fn test_parse_narration_with_expression() {
        let source = "scene \"test\" {\n    narration $greeting + \"，你好\"\n}";
        assert!(parse_script(source).is_ok(), "narration + 表达式应成功解析");
    }

    #[test]
    fn test_parse_narration_not_mistaken_for_dialogue() {
        // narration 关键字不应被当作说话者名字解析为 dialogue
        let source = "scene \"test\" {\n    narration \"这是旁白\"\n}";
        let result = parse_script(source);
        assert!(
            result.is_ok(),
            "narration 应被正确识别为旁白而非对话，错误: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_menu() {
        let source = "scene \"test\" {\n    menu \"选择：\" {\n        \"选项A\" {\n            jump \"label_a\"\n        }\n        \"选项B\" {\n            jump \"label_b\"\n        }\n    }\n}";
        assert!(parse_script(source).is_ok(), "menu 语句应成功解析");
    }

    #[test]
    fn test_parse_menu_with_conditions() {
        let source = "scene \"test\" {\n    menu \"选择：\" {\n        \"普通选项\" {\n            jump \"normal\"\n        }\n        \"隐藏选项\" if $affection >= 5 {\n            jump \"secret\"\n        }\n    }\n}";
        assert!(parse_script(source).is_ok(), "menu + 条件选项应成功解析");
    }

    #[test]
    fn test_parse_if_branch() {
        let source =
            "scene \"test\" {\n    if $score >= 100 {\n        narration \"完美！\"\n    }\n}";
        assert!(parse_script(source).is_ok(), "if 分支应成功解析");
    }

    #[test]
    fn test_parse_if_elif_else() {
        let source = "scene \"test\" {\n    if $score >= 100 {\n        narration \"完美\"\n    } elif $score > 50 {\n        narration \"不错\"\n    } else {\n        narration \"加油\"\n    }\n}";
        assert!(parse_script(source).is_ok(), "if/elif/else 应成功解析");
    }

    #[test]
    fn test_parse_nested_if() {
        let source = "scene \"test\" {\n    if $a > 0 {\n        narration \"外层\"\n        if $b > 0 {\n            narration \"内层\"\n        }\n    }\n}";
        assert!(parse_script(source).is_ok(), "嵌套 if 应成功解析");
    }

    #[test]
    fn test_parse_jump() {
        let source = "scene \"test\" {\n    jump \"some_label\"\n}";
        assert!(parse_script(source).is_ok(), "jump 语句应成功解析");
    }

    #[test]
    fn test_parse_goto() {
        let source = "scene \"test\" {\n    goto \"chapter1/scene\"\n}";
        assert!(parse_script(source).is_ok(), "goto 语句应成功解析");
    }

    #[test]
    fn test_parse_goto_with_label() {
        let source = "scene \"test\" {\n    goto \"chapter1/scene\" label: \"main_flow\"\n}";
        assert!(parse_script(source).is_ok(), "goto + label 应成功解析");
    }

    #[test]
    fn test_parse_call() {
        let source = "scene \"test\" {\n    call \"subroutine_name\"\n}";
        assert!(parse_script(source).is_ok(), "call 语句应成功解析");
    }

    #[test]
    fn test_parse_return() {
        let source = "scene \"test\" {\n    return\n}";
        assert!(parse_script(source).is_ok(), "return 语句应成功解析");
    }

    #[test]
    fn test_parse_label_with_identifier() {
        let source = "scene \"test\" {\n    label branch_demo\n}";
        assert!(
            parse_script(source).is_ok(),
            "label + identifier 应成功解析"
        );
    }

    #[test]
    fn test_parse_label_with_string() {
        let source = "scene \"test\" {\n    label \"branch_demo\"\n}";
        assert!(parse_script(source).is_ok(), "label + string 应成功解析");
    }

    #[test]
    fn test_parse_assignment_int() {
        let source = "scene \"test\" {\n    $score = 0\n}";
        assert!(parse_script(source).is_ok(), "变量赋值（整型）应成功解析");
    }

    #[test]
    fn test_parse_assignment_expr() {
        let source = "scene \"test\" {\n    $score = $score + 10\n}";
        assert!(parse_script(source).is_ok(), "变量赋值（表达式）应成功解析");
    }

    #[test]
    fn test_parse_assignment_comparison() {
        let source = "scene \"test\" {\n    $is_high = $affection >= 5\n}";
        assert!(
            parse_script(source).is_ok(),
            "变量赋值（比较结果）应成功解析"
        );
    }

    #[test]
    fn test_parse_assignment_logic() {
        let source = "scene \"test\" {\n    $both = $a >= 3 and $b >= 3\n}";
        assert!(
            parse_script(source).is_ok(),
            "变量赋值（逻辑运算）应成功解析"
        );
    }

    #[test]
    fn test_parse_set_flag() {
        let source = "scene \"test\" {\n    set %met_akane\n}";
        assert!(parse_script(source).is_ok(), "set flag 应成功解析");
    }

    #[test]
    fn test_parse_unset_flag() {
        let source = "scene \"test\" {\n    unset %warning\n}";
        assert!(parse_script(source).is_ok(), "unset flag 应成功解析");
    }

    #[test]
    fn test_parse_toggle_flag() {
        let source = "scene \"test\" {\n    toggle %auto_mode\n}";
        assert!(parse_script(source).is_ok(), "toggle flag 应成功解析");
    }

    #[test]
    fn test_parse_effect() {
        let source = "scene \"test\" {\n    effect \"shake\" intensity: 0.8 duration: 500\n}";
        assert!(parse_script(source).is_ok(), "effect 语句应成功解析");
    }

    #[test]
    fn test_parse_effect_with_expr_params() {
        let source = "scene \"test\" {\n    effect \"flash\" color: \"#FFFFFF\" duration: $base_delay * 0.6\n}";
        assert!(
            parse_script(source).is_ok(),
            "effect + 表达式参数应成功解析"
        );
    }

    #[test]
    fn test_parse_wait_int() {
        let source = "scene \"test\" {\n    wait 800\n}";
        assert!(parse_script(source).is_ok(), "wait + int 应成功解析");
    }

    #[test]
    fn test_parse_wait_float() {
        let source = "scene \"test\" {\n    wait 1.5\n}";
        assert!(parse_script(source).is_ok(), "wait + float 应成功解析");
    }

    #[test]
    fn test_parse_wait_expr() {
        let source = "scene \"test\" {\n    wait $base_delay * 3\n}";
        assert!(parse_script(source).is_ok(), "wait + 表达式应成功解析");
    }

    // ── 错误消息测试 ──

    #[test]
    fn test_parse_error_has_chinese_message() {
        let source = "garbage text that cannot be parsed";
        let result = parse_script(source);
        assert!(result.is_err(), "非法输入应返回错误");

        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
        // 验证错误消息非空且至少包含中文或英文描述
        assert!(!errors[0].message.is_empty());
    }

    #[test]
    fn test_parse_error_has_context() {
        let source = "scene \"test\" {\n    invalid_syntax!!!!\n}";
        let result = parse_script(source);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
        // context 应包含出错行源码
        assert!(!errors[0].context.is_empty());
    }

    #[test]
    fn test_parse_error_display_format() {
        let err = ParseError::new(
            (5, 10, 50),
            "测试错误".to_string(),
            Some("试试这样修复".to_string()),
            "    bad code here".to_string(),
        );
        let display = format!("{}", err);
        assert!(display.contains("第5行第10列"));
        assert!(display.contains("测试错误"));
        assert!(display.contains("试试这样修复"));
    }

    // ── 表达式解析 ──

    #[test]
    fn test_parse_string_literal_in_expr() {
        let source = "scene \"test\" {\n    bg \"classroom\"\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_variable_ref_in_expr() {
        let source = "scene \"test\" {\n    bg $bg_var\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_arithmetic_expr() {
        let source = "scene \"test\" {\n    $score = $a + $b * $c\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_comparison_expr() {
        let source = "scene \"test\" {\n    $result = $a >= $b\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_logic_expr() {
        let source = "scene \"test\" {\n    $result = $a >= 3 and $b >= 3\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_unary_not() {
        let source =
            "scene \"test\" {\n    if not %flag {\n        narration \"flag not set\"\n    }\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_unary_neg() {
        let source = "scene \"test\" {\n    $debt = -500\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_parenthesized_expr() {
        let source = "scene \"test\" {\n    $adjusted = ($score - 5) * 3\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_string_concat() {
        let source = "scene \"test\" {\n    $greeting = \"你好，\" + $player_name\n}";
        assert!(parse_script(source).is_ok());
    }

    #[test]
    fn test_parse_custom_position_with_expr() {
        let source = "scene \"test\" {\n    show sayori at ($x + 0.1, 0.5)\n}";
        assert!(
            parse_script(source).is_ok(),
            "自定义坐标 + 表达式应成功解析"
        );
    }

    #[test]
    fn test_parse_transition_with_expr() {
        let source = "scene \"test\" {\n    bg \"bg\" with fade($fade_speed * 1000)\n}";
        assert!(
            parse_script(source).is_ok(),
            "转场 + 表达式 duration 应成功解析"
        );
    }
}
