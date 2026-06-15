//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/expr.rs
//! 功能概述：表达式类型定义 — `Expr`（表达式 AST 节点，7 种变体）、
//!           `BinaryOp`（二元运算符枚举，12 种）和 `UnaryOp`（一元运算符枚举，2 种）。
//!           这些类型在 parser→compiler 管线中共享：
//!           - `aster-parser`：解析 .aster 脚本中的表达式，产出 `Expr` 树
//!           - `aster-compiler`：将 `Expr` 树编译为字节码指令序列
//!           不直接出现在运行时（VM 执行的是字节码，不是 Expr 树）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//!
//! 对应文档：Architecture.md §4.2（核心类型清单）
//!           任务：PH1-T03 — 实现 aster-core 资源与变量类型
//!
//! ## 类型说明
//!
//! `Expr` 是 .aster 脚本中所有"值"的统一表示。无论是静态字符串字面量、
//! 变量引用还是复杂表达式（`$a + $b * $c`），最终都表示为 `Expr` 树。
//! SceneNode 中所有可能出现表达式的位置（资产路径、文本内容、数值参数、
//! 跳转目标等）统一使用 `Expr` 类型，确保 parser 产出后 compiler 可直接消费，
//! 无需二次解析。

use serde::{Deserialize, Serialize};

/// 二元运算符枚举 — 涵盖算术、比较、逻辑三类运算。
///
/// 仅用于 `Expr::BinaryOp` 变体。所有运算符均派生 `Copy + Clone`，
/// 以便在 `Expr` 树中高效传递。
///
/// # 运算符分类
///
/// | 类别 | 运算符 | 说明 |
/// |------|--------|------|
/// | 算术 | `Add` / `Sub` / `Mul` / `Div` | 加减乘除 |
/// | 比较 | `Eq` / `Neq` / `Lt` / `Gt` / `Le` / `Ge` | 等于/不等于/小于/大于/≤/≥ |
/// | 逻辑 | `And` / `Or` | 逻辑与/或 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinaryOp {
    // ── 算术运算符 ──
    /// 加法 `+`
    Add,
    /// 减法 `-`
    Sub,
    /// 乘法 `*`
    Mul,
    /// 除法 `/`
    Div,

    // ── 比较运算符 ──
    /// 等于 `==`
    Eq,
    /// 不等于 `!=`
    Neq,
    /// 小于 `<`
    Lt,
    /// 大于 `>`
    Gt,
    /// 小于等于 `<=`
    Le,
    /// 大于等于 `>=`
    Ge,

    // ── 逻辑运算符 ──
    /// 逻辑与 `and`
    And,
    /// 逻辑或 `or`
    Or,
}

/// 一元运算符枚举 — 仅用于 `Expr::UnaryOp` 变体。
///
/// 与 `BinaryOp` 分离确保类型安全：`Expr::UnaryOp` 只能接受 `Not` 或 `Neg`，
/// 不可能误传入算术/比较/逻辑二元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOp {
    /// 逻辑非 `not`
    Not,
    /// 算术取负 `-`（一元）
    Neg,
}

/// 表达式 AST 节点 — 表示 .aster 脚本中的一个表达式树。
///
/// 支持 7 种节点类型：
///
/// | Variant | .aster 语法示例 | 说明 |
/// |---------|----------------|------|
/// | `StringLiteral` | `"你好"` | 双引号字符串 |
/// | `IntLiteral` | `42` | 整数 |
/// | `FloatLiteral` | `3.14` | 浮点数 |
/// | `BoolLiteral` | `true` / `false` | 布尔值 |
/// | `Variable` | `$score` | 变量引用 |
/// | `BinaryOp` | `$a + $b` | 二元运算 |
/// | `UnaryOp` | `not $flag` | 一元运算 |
///
/// # 使用场景
///
/// - **SceneNode 字段**：所有可能出现表达式的 AST 字段（资产路径、文本、
///   数值参数、跳转目标等）统一使用 `Expr` 类型
/// - **解析阶段**：`aster-parser` 将脚本中的表达式文本解析为 `Expr` 树
/// - **编译阶段**：`aster-compiler` 将 `Expr` 树编译为字节码（如 `$a + $b`
///   编译为 `PushVar a, PushVar b, Add` 三条指令）
/// - **运行时**：VM 不直接使用 `Expr`，而是执行编译后的字节码
///
/// # 序列化
///
/// 派生 `Serialize + Deserialize`，支持 JSON/TOML 等格式。
///
/// # 示例
/// ```
/// use aster_core::{Expr, BinaryOp};
///
/// // 表示表达式：$score + 10
/// let expr = Expr::BinaryOp(
///     Box::new(Expr::Variable("score".into())),
///     BinaryOp::Add,
///     Box::new(Expr::IntLiteral(10)),
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// 字符串字面量 — 如 `"你好世界"`
    StringLiteral(String),

    /// 整数字面量 — 如 `42`
    IntLiteral(i64),

    /// 浮点数字面量 — 如 `3.14`
    FloatLiteral(f64),

    /// 布尔字面量 — `true` 或 `false`
    BoolLiteral(bool),

    /// 变量引用 — 如 `$score`、`$player_name`
    /// 存储变量名（不含 `$` 前缀）
    Variable(String),

    /// 二元运算 — 如 `$a + $b`、`$score >= 100`
    /// 包含左操作数、二元运算符、右操作数
    BinaryOp(Box<Expr>, BinaryOp, Box<Expr>),

    /// 一元运算 — 如 `not $flag`、`-$value`
    /// 包含一元运算符和操作数
    UnaryOp(UnaryOp, Box<Expr>),
}

impl Expr {
    /// 创建字符串字面量的便捷构造函数。
    pub fn string_literal(s: impl Into<String>) -> Self {
        Expr::StringLiteral(s.into())
    }

    /// 创建整数字面量的便捷构造函数。
    pub fn int_literal(v: i64) -> Self {
        Expr::IntLiteral(v)
    }

    /// 创建浮点数字面量的便捷构造函数。
    pub fn float_literal(v: f64) -> Self {
        Expr::FloatLiteral(v)
    }

    /// 创建布尔字面量的便捷构造函数。
    pub fn bool_literal(v: bool) -> Self {
        Expr::BoolLiteral(v)
    }

    /// 创建变量引用的便捷构造函数。
    ///
    /// # 参数
    /// - `name`：变量名（不含 `$` 前缀）
    pub fn variable(name: impl Into<String>) -> Self {
        Expr::Variable(name.into())
    }

    /// 创建二元运算的便捷构造函数。
    ///
    /// # 参数
    /// - `left`：左操作数
    /// - `op`：二元运算符
    /// - `right`：右操作数
    pub fn binary_op(left: Expr, op: BinaryOp, right: Expr) -> Self {
        Expr::BinaryOp(Box::new(left), op, Box::new(right))
    }

    /// 创建一元运算的便捷构造函数。
    ///
    /// # 参数
    /// - `op`：一元运算符（`UnaryOp::Not` 或 `UnaryOp::Neg`）
    /// - `operand`：操作数
    pub fn unary_op(op: UnaryOp, operand: Expr) -> Self {
        Expr::UnaryOp(op, Box::new(operand))
    }

    /// 如果表达式是整数字面量，返回其值。
    ///
    /// 帮助编译器在常量折叠阶段快速提取字面量值，
    /// 也用于 `Position::to_coords()` 等方法中提取静态坐标。
    ///
    /// # 返回值
    /// - `Some(i64)`：表达式为 `IntLiteral`
    /// - `None`：表达式为非字面量（变量引用、运算等）
    pub fn as_int_literal(&self) -> Option<i64> {
        match self {
            Expr::IntLiteral(v) => Some(*v),
            _ => None,
        }
    }

    /// 如果表达式是浮点数字面量（或可无损转换的整数字面量），返回其 `f64` 值。
    ///
    /// # 返回值
    /// - `Some(f64)`：表达式为 `FloatLiteral` 或 `IntLiteral`
    /// - `None`：表达式为非字面量
    pub fn as_float_literal(&self) -> Option<f64> {
        match self {
            Expr::FloatLiteral(v) => Some(*v),
            Expr::IntLiteral(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// 如果表达式是字符串字面量，返回其引用。
    pub fn as_string_literal(&self) -> Option<&str> {
        match self {
            Expr::StringLiteral(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// 如果表达式是布尔字面量，返回其值。
    pub fn as_bool_literal(&self) -> Option<bool> {
        match self {
            Expr::BoolLiteral(v) => Some(*v),
            _ => None,
        }
    }

    /// 如果表达式是变量引用，返回变量名。
    pub fn as_variable(&self) -> Option<&str> {
        match self {
            Expr::Variable(name) => Some(name.as_str()),
            _ => None,
        }
    }
}

// ─── 辅助：Expr 字面量默认值 ────────────────────────────────────────────────

/// 返回 Expr 浮点字面量 `1.0`。
///
/// 用于 SceneNode 中需要数值默认值的字段的 serde `#[serde(default = ...)]`。
pub fn default_expr_one() -> Expr {
    Expr::FloatLiteral(1.0)
}

/// 返回 Expr 布尔字面量 `true`。
pub fn default_expr_true() -> Expr {
    Expr::BoolLiteral(true)
}

/// 返回 Expr 整数字面量 `0`。
pub fn default_expr_zero() -> Expr {
    Expr::IntLiteral(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── BinaryOp / UnaryOp 基础测试 ───────────────────────────────────────

    /// 验证 BinaryOp 所有 12 种运算符的 serde round-trip。
    #[test]
    fn binary_op_serde_roundtrip() {
        let ops = vec![
            BinaryOp::Add,
            BinaryOp::Sub,
            BinaryOp::Mul,
            BinaryOp::Div,
            BinaryOp::Eq,
            BinaryOp::Neq,
            BinaryOp::Lt,
            BinaryOp::Gt,
            BinaryOp::Le,
            BinaryOp::Ge,
            BinaryOp::And,
            BinaryOp::Or,
        ];

        for op in &ops {
            let json =
                serde_json::to_string(op).unwrap_or_else(|_| panic!("{op:?} JSON 序列化失败"));
            let restored: BinaryOp =
                serde_json::from_str(&json).unwrap_or_else(|_| panic!("{op:?} JSON 反序列化失败"));
            assert_eq!(&restored, op);
        }
    }

    /// 验证 UnaryOp 2 种运算符的 serde round-trip。
    #[test]
    fn unary_op_serde_roundtrip() {
        let ops = vec![UnaryOp::Not, UnaryOp::Neg];

        for op in &ops {
            let json =
                serde_json::to_string(op).unwrap_or_else(|_| panic!("{op:?} JSON 序列化失败"));
            let restored: UnaryOp =
                serde_json::from_str(&json).unwrap_or_else(|_| panic!("{op:?} JSON 反序列化失败"));
            assert_eq!(&restored, op);
        }
    }

    /// 验证 BinaryOp 的 Copy 语义。
    #[test]
    fn binary_op_copy_semantics() {
        let op = BinaryOp::Add;
        let copied = op; // Copy，非 move
        assert_eq!(op, BinaryOp::Add); // 原变量仍可用
        assert_eq!(copied, BinaryOp::Add);
    }

    /// 验证 UnaryOp 的 Copy 语义。
    #[test]
    fn unary_op_copy_semantics() {
        let op = UnaryOp::Not;
        let copied = op;
        assert_eq!(op, UnaryOp::Not);
        assert_eq!(copied, UnaryOp::Not);
    }

    // ─── Expr 构造与 serde 测试 ────────────────────────────────────────────

    /// AC07 — `Expr` 的构造和 serde round-trip。
    #[test]
    fn expr_construction_and_serde_roundtrip() {
        // 构造：$score + 10 > 5
        let expr = Expr::binary_op(
            Expr::binary_op(
                Expr::variable("score"),
                BinaryOp::Add,
                Expr::int_literal(10),
            ),
            BinaryOp::Gt,
            Expr::int_literal(5),
        );

        // 模式匹配验证结构
        match &expr {
            Expr::BinaryOp(left, op, right) => {
                assert_eq!(*op, BinaryOp::Gt);
                match left.as_ref() {
                    Expr::BinaryOp(inner_left, inner_op, inner_right) => {
                        assert_eq!(*inner_op, BinaryOp::Add);
                        match inner_left.as_ref() {
                            Expr::Variable(name) => assert_eq!(name, "score"),
                            _ => panic!("期望 Variable"),
                        }
                        match inner_right.as_ref() {
                            Expr::IntLiteral(n) => assert_eq!(*n, 10),
                            _ => panic!("期望 IntLiteral"),
                        }
                    }
                    _ => panic!("期望 BinaryOp"),
                }
                match right.as_ref() {
                    Expr::IntLiteral(n) => assert_eq!(*n, 5),
                    _ => panic!("期望 IntLiteral"),
                }
            }
            _ => panic!("期望 BinaryOp"),
        }

        // serde round-trip
        let json = serde_json::to_string(&expr).expect("JSON 序列化失败");
        let restored: Expr = serde_json::from_str(&json).expect("JSON 反序列化失败");
        assert_eq!(restored, expr);
    }

    /// 验证所有 Expr variant 的构造函数和 serde。
    #[test]
    fn expr_all_variants() {
        let variants: Vec<Expr> = vec![
            Expr::string_literal("你好"),
            Expr::int_literal(42),
            Expr::float_literal(std::f64::consts::PI),
            Expr::bool_literal(true),
            Expr::variable("score"),
            Expr::binary_op(Expr::int_literal(1), BinaryOp::Add, Expr::int_literal(2)),
            Expr::unary_op(UnaryOp::Not, Expr::bool_literal(false)),
        ];

        for v in &variants {
            let json = serde_json::to_string(v).expect("JSON 序列化失败");
            let restored: Expr = serde_json::from_str(&json).expect("JSON 反序列化失败");
            assert_eq!(&restored, v);
        }
    }

    /// 验证深度嵌套的 Expr 树序列化成功（serde_json 默认递归限制约 128 层）。
    #[test]
    fn deeply_nested_expr() {
        // 构造 20 层左结合加法：((...(1 + 2) + 3) + ... + 20)
        let mut expr = Expr::int_literal(1);
        for i in 2..=20 {
            expr = Expr::binary_op(expr, BinaryOp::Add, Expr::int_literal(i));
        }

        let json = serde_json::to_string(&expr).expect("JSON 序列化失败");
        let restored: Expr = serde_json::from_str(&json).expect("JSON 反序列化失败");
        assert_eq!(restored, expr);
    }

    /// 验证一元取负表达式：-$value
    #[test]
    fn unary_neg_expression() {
        let expr = Expr::unary_op(UnaryOp::Neg, Expr::variable("value"));
        match &expr {
            Expr::UnaryOp(op, operand) => {
                assert_eq!(*op, UnaryOp::Neg);
                assert_eq!(**operand, Expr::Variable("value".into()));
            }
            _ => panic!("期望 UnaryOp"),
        }
    }

    /// 验证一元逻辑非：not $flag
    #[test]
    fn unary_not_expression() {
        let expr = Expr::unary_op(UnaryOp::Not, Expr::variable("flag"));
        match &expr {
            Expr::UnaryOp(op, operand) => {
                assert_eq!(*op, UnaryOp::Not);
                assert_eq!(**operand, Expr::Variable("flag".into()));
            }
            _ => panic!("期望 UnaryOp"),
        }
    }

    // ─── Expr 辅助方法测试 ─────────────────────────────────────────────────

    /// 验证 as_int_literal() 提取整数字面量。
    #[test]
    fn as_int_literal_extraction() {
        assert_eq!(Expr::int_literal(42).as_int_literal(), Some(42));
        assert_eq!(
            Expr::float_literal(std::f64::consts::PI).as_int_literal(),
            None
        );
        assert_eq!(Expr::variable("x").as_int_literal(), None);
        assert_eq!(
            Expr::binary_op(Expr::int_literal(1), BinaryOp::Add, Expr::int_literal(2))
                .as_int_literal(),
            None
        );
    }

    /// 验证 as_float_literal() 提取浮点字面量（含 IntLiteral 自动转换）。
    #[test]
    fn as_float_literal_extraction() {
        assert!(
            (Expr::float_literal(std::f64::consts::PI)
                .as_float_literal()
                .unwrap()
                - std::f64::consts::PI)
                .abs()
                < f64::EPSILON
        );
        assert!((Expr::int_literal(42).as_float_literal().unwrap() - 42.0).abs() < f64::EPSILON);
        assert_eq!(Expr::string_literal("3.14").as_float_literal(), None);
        assert_eq!(Expr::variable("x").as_float_literal(), None);
    }

    /// 验证 as_string_literal() 提取字符串字面量。
    #[test]
    fn as_string_literal_extraction() {
        assert_eq!(
            Expr::string_literal("hello").as_string_literal(),
            Some("hello")
        );
        assert_eq!(Expr::int_literal(1).as_string_literal(), None);
        assert_eq!(Expr::variable("x").as_string_literal(), None);
    }

    /// 验证 as_bool_literal() 提取布尔字面量。
    #[test]
    fn as_bool_literal_extraction() {
        assert_eq!(Expr::bool_literal(true).as_bool_literal(), Some(true));
        assert_eq!(Expr::bool_literal(false).as_bool_literal(), Some(false));
        assert_eq!(Expr::int_literal(0).as_bool_literal(), None);
    }

    /// 验证 as_variable() 提取变量名。
    #[test]
    fn as_variable_extraction() {
        assert_eq!(Expr::variable("score").as_variable(), Some("score"));
        assert_eq!(Expr::string_literal("score").as_variable(), None);
    }

    /// 验证 default 辅助函数。
    #[test]
    fn default_expr_helpers() {
        assert_eq!(default_expr_one(), Expr::FloatLiteral(1.0));
        assert_eq!(default_expr_true(), Expr::BoolLiteral(true));
        assert_eq!(default_expr_zero(), Expr::IntLiteral(0));
    }

    // ─── Expr::FloatLiteral(NaN) 行为文档化 ────────────────────────────

    /// 验证 Expr::FloatLiteral(NaN) 的 PartialEq 行为。
    ///
    /// # 设计说明
    ///
    /// `Expr` 的 `PartialEq` 由 derive 宏自动实现，依赖各字段的 `PartialEq`。
    /// `f64::NAN != f64::NAN` 是 IEEE 754 标准行为，因此：
    /// `Expr::FloatLiteral(f64::NAN) != Expr::FloatLiteral(f64::NAN)`
    ///
    /// 这与 `Value::Float(NaN)` 的 total_cmp 语义（NaN==NaN）**不同**：
    /// - `Value` 是运行时值类型，需要 HashMap key 一致性
    /// - `Expr` 是编译期 AST 节点，不会用作 HashMap key
    ///
    /// 此测试显式记录这一有意为之的行为差异。
    #[test]
    fn expr_float_nan_not_equal_to_self() {
        let nan1 = Expr::FloatLiteral(f64::NAN);
        let nan2 = Expr::FloatLiteral(f64::NAN);
        // IEEE 754: NaN != NaN
        assert_ne!(
            nan1, nan2,
            "Expr::FloatLiteral(NaN) 遵循 IEEE 754 语义，NaN≠NaN"
        );
    }

    /// 验证普通 Expr 的等价比较不受 NaN 行为影响。
    #[test]
    fn expr_normal_values_equality() {
        assert_eq!(Expr::int_literal(1), Expr::int_literal(1));
        assert_eq!(Expr::string_literal("a"), Expr::string_literal("a"));
        assert_ne!(Expr::int_literal(1), Expr::int_literal(2));
    }
}
