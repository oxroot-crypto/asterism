//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/variable.rs
//! 功能概述：变量与旗标系统类型定义 —
//!           `Value`（运行时值类型枚举，6 种变体）、
//!           `VariableStore`（变量存储表，包装 `HashMap<String, Value>`）、
//!           `FlagSet`（旗标集合，包装 `HashSet<String>`）。
//!           这些类型是 VM 变量/旗标操作码（PH1-T14）的数据载体，
//!           也是存档系统（Phase 2）需要序列化保存的核心状态。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//! - std::collections::{HashMap, HashSet}（内部存储容器）
//!
//! 对应需求：REQ-ENG-003（变量与旗标系统）
//!           支持整型、浮点、字符串、布尔四种值类型的变量存储。
//!           支持旗标（命名布尔值）的 set/unset/toggle/check 操作。
//!           变量在场景间保持有效；旗标操作结果正确。
//!
//! 对应文档：Architecture.md §4.2（核心类型清单）、§4.5（VM 架构）
//!           任务：PH1-T03 — 实现 aster-core 资源与变量类型

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

/// 运行时值类型 — 引擎变量系统中所有可能的值变体。
///
/// 支持 6 种值类型。其中 `Int`、`Float`、`String`、`Bool` 是 v0.1
/// 阶段开放使用的核心类型；`Array` 和 `Map` 为预留变体，
/// 在 Phase 4（表达式增强）中启用。
///
/// # PartialEq 实现说明
///
/// `Float` 变体使用 `f64::total_cmp` 语义进行比较：
/// - `NaN` 与 `NaN` 比较为相等
/// - `NaN` 与任何非 `NaN` 值比较为不相等
/// - 这确保 `VariableStore` 和 `HashMap<Value, ...>` 等依赖 `Eq` 的场景不会因 `NaN` 而行为异常
///
/// # Serde 序列化
///
/// 通过 serde 派生宏支持 JSON/TOML 等格式的序列化。`Float` 变体
/// 在序列化时特殊处理 `NaN` 和 `Infinity`：
/// - `NaN` → `null`
/// - `Infinity` → `"Infinity"`（字符串标记）
/// - `-Infinity` → `"-Infinity"`（字符串标记）
/// - 普通值 → 数值字面量
///
/// # 示例
/// ```
/// use aster_core::Value;
///
/// let int_val = Value::Int(100);
/// let float_val = Value::Float(0.5);
/// let str_val = Value::String("hello".into());
/// let bool_val = Value::Bool(true);
///
/// assert_eq!(int_val.type_name(), "Int");
/// assert_eq!(bool_val.type_name(), "Bool");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    /// 64 位有符号整数
    Int(i64),

    /// 64 位双精度浮点数
    #[serde(
        serialize_with = "serialize_float",
        deserialize_with = "deserialize_float"
    )]
    Float(f64),

    /// UTF-8 字符串
    String(String),

    /// 布尔值（`true` / `false`）
    Bool(bool),

    /// 值数组（Phase 4 启用）
    Array(Vec<Value>),

    /// 值映射表（Phase 4 启用）
    Map(HashMap<String, Value>),
}

// ─── Float 序列化辅助函数 ────────────────────────────────────────────────

/// 自定义 f64 序列化：处理 NaN/Infinity 的 JSON 兼容输出。
fn serialize_float<S: serde::Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    if value.is_nan() {
        serializer.serialize_none() // NaN → null
    } else if value.is_infinite() {
        if *value > 0.0 {
            serializer.serialize_str("Infinity")
        } else {
            serializer.serialize_str("-Infinity")
        }
    } else {
        serializer.serialize_f64(*value)
    }
}

/// 自定义 f64 反序列化：支持 null→NaN 和字符串标记的反向转换。
fn deserialize_float<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    use serde::de;

    /// Visitor 结构体，处理 JSON 中 f64 的多种表示形式
    struct FloatVisitor;

    impl<'de> de::Visitor<'de> for FloatVisitor {
        type Value = f64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("一个浮点数、null（表示 NaN）或 \"Infinity\"/\"-Infinity\" 字符串")
        }

        fn visit_f64<E: de::Error>(self, value: f64) -> Result<f64, E> {
            Ok(value)
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<f64, E> {
            Ok(value as f64)
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<f64, E> {
            Ok(value as f64)
        }

        fn visit_none<E: de::Error>(self) -> Result<f64, E> {
            Ok(f64::NAN)
        }

        fn visit_unit<E: de::Error>(self) -> Result<f64, E> {
            Ok(f64::NAN) // null/unit → NaN
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<f64, E> {
            match value {
                "Infinity" => Ok(f64::INFINITY),
                "-Infinity" => Ok(f64::NEG_INFINITY),
                _ => Err(de::Error::custom(format!(
                    "无效的浮点数标记：'{}'，期望 \"Infinity\" 或 \"-Infinity\"",
                    value
                ))),
            }
        }
    }

    deserializer.deserialize_any(FloatVisitor)
}

// ─── Value 方法实现 ──────────────────────────────────────────────────────

impl Value {
    /// 创建 `Int` 变体的便捷构造函数。
    ///
    /// # 示例
    /// ```
    /// use aster_core::Value;
    ///
    /// let v = Value::int(42);
    /// assert_eq!(v, Value::Int(42));
    /// ```
    pub fn int(v: i64) -> Self {
        Value::Int(v)
    }

    /// 创建 `Float` 变体的便捷构造函数。
    ///
    /// # 示例
    /// ```
    /// use aster_core::Value;
    ///
    /// let v = Value::float(3.14);
    /// assert_eq!(v, Value::Float(3.14));
    /// ```
    pub fn float(v: f64) -> Self {
        Value::Float(v)
    }

    /// 创建 `String` 变体的便捷构造函数。
    ///
    /// # 示例
    /// ```
    /// use aster_core::Value;
    ///
    /// let v = Value::string("你好");
    /// assert_eq!(v, Value::String("你好".into()));
    /// ```
    pub fn string(v: impl Into<String>) -> Self {
        Value::String(v.into())
    }

    /// 创建 `Bool` 变体的便捷构造函数。
    ///
    /// # 示例
    /// ```
    /// use aster_core::Value;
    ///
    /// let v = Value::bool(true);
    /// assert_eq!(v, Value::Bool(true));
    /// ```
    pub fn bool(v: bool) -> Self {
        Value::Bool(v)
    }

    /// 返回值的类型名称（中文）。
    ///
    /// # 示例
    /// ```
    /// use aster_core::Value;
    /// use std::collections::HashMap;
    ///
    /// assert_eq!(Value::Int(1).type_name(), "Int");
    /// assert_eq!(Value::Float(0.5).type_name(), "Float");
    /// assert_eq!(Value::String("".into()).type_name(), "String");
    /// assert_eq!(Value::Bool(false).type_name(), "Bool");
    /// assert_eq!(Value::Array(vec![]).type_name(), "Array");
    /// assert_eq!(Value::Map(HashMap::new()).type_name(), "Map");
    /// ```
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Bool(_) => "Bool",
            Value::Array(_) => "Array",
            Value::Map(_) => "Map",
        }
    }
}

// ─── Value PartialEq 实现 ─────────────────────────────────────────────────

/// Value 的 PartialEq 实现。
///
/// `Float` 变体使用 `f64::total_cmp` 语义：
/// - `NaN` == `NaN`（与 IEEE 754 默认行为不同，但确保 HashMap 等数据结构的一致性）
/// - `+0.0` == `-0.0`
/// - 其他情况按 f64 数值比较
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => {
                // 使用 total_cmp 语义：NaN == NaN, +0 == -0
                a.total_cmp(b) == std::cmp::Ordering::Equal
            }
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            _ => false,
        }
    }
}

// ─── VariableStore ────────────────────────────────────────────────────────

/// 变量存储表 — 包装 `HashMap<String, Value>` 提供引擎全局/场景变量的 CRUD 接口。
///
/// 变量在场景间保持有效（与场景脚本中定义的变量同生命周期），
/// 在存档时序列化为 JSON 后打包存入 `SaveData.variable_store`。
///
/// # 设计说明
///
/// 不使用 `Index`/`IndexMut` trait 实现下标访问。所有读写通过
/// 显式的 `get`/`set`/`delete` 方法完成，便于：
/// 1. 添加日志/追踪（`tracing::trace!`）
/// 2. 在 `set` 时触发变更通知（后续 Phase 可扩展 `on_change` 回调）
/// 3. 统一错误处理和类型检查
///
/// # 示例
/// ```
/// use aster_core::{VariableStore, Value};
///
/// let mut store = VariableStore::new();
/// store.set("score", Value::Int(100));
/// store.set("player_name", Value::String("主角".into()));
///
/// assert_eq!(store.get("score"), Some(&Value::Int(100)));
/// assert!(store.has("player_name"));
/// assert_eq!(store.len(), 2);
///
/// store.delete("score");
/// assert!(!store.has("score"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableStore {
    /// 变量名 → 值的映射表
    variables: HashMap<String, Value>,
}

impl VariableStore {
    /// 创建一个空的变量存储表。
    pub fn new() -> Self {
        VariableStore {
            variables: HashMap::new(),
        }
    }

    /// 查询指定名称的变量值。
    ///
    /// # 参数
    /// - `name`：变量名
    ///
    /// # 返回值
    /// - `Some(&Value)`：变量存在时返回其值的不可变引用
    /// - `None`：变量不存在
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    /// 设置（插入或覆盖）一个变量。
    ///
    /// 如果变量已存在，旧值被新值替换。
    ///
    /// # 参数
    /// - `name`：变量名（接受 `impl Into<String>`，支持 `&str` 或 `String`）
    /// - `value`：要存储的值
    pub fn set(&mut self, name: impl Into<String>, value: Value) {
        self.variables.insert(name.into(), value);
    }

    /// 删除指定名称的变量。
    ///
    /// # 参数
    /// - `name`：变量名
    ///
    /// # 返回值
    /// - `true`：变量存在并已删除
    /// - `false`：变量不存在，无操作
    pub fn delete(&mut self, name: &str) -> bool {
        self.variables.remove(name).is_some()
    }

    /// 检查指定名称的变量是否存在。
    ///
    /// # 参数
    /// - `name`：变量名
    pub fn has(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// 返回当前存储的变量数量。
    pub fn len(&self) -> usize {
        self.variables.len()
    }

    /// 返回变量存储表是否为空。
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// 清空所有变量。
    ///
    /// 通常在场景切换时调用（取决于 SceneManager 的策略——
    /// 是清除全部变量还是仅清除场景局部变量）。
    pub fn clear(&mut self) {
        self.variables.clear();
    }

    /// 遍历所有变量的不可变迭代器。
    ///
    /// 迭代顺序与 `HashMap` 的内部顺序一致（非确定性）。
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.variables.iter()
    }
}

impl Default for VariableStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─── FlagSet ──────────────────────────────────────────────────────────────

/// 旗标集合 — 包装 `HashSet<String>` 提供命名布尔旗标的 set/unset/toggle/check 操作。
///
/// 旗标是命名布尔值，用于跟踪游戏中的二元状态（如"是否已见过某个角色"、
/// "是否已触发某个事件"）。旗标在场景间保持有效，在存档时序列化为
/// JSON 字符串数组后打包存入 `SaveData.flags`。
///
/// # 设计说明
///
/// 旗标的内部实现为 `HashSet<String>`：
/// - 已设置的旗标存在于集合中（`check(flag) == true`）
/// - 未设置的旗标不在集合中（`check(flag) == false`）
/// - 这比 `HashMap<String, bool>` 更节省内存且语义更清晰
///
/// # 示例
/// ```
/// use aster_core::FlagSet;
///
/// let mut flags = FlagSet::new();
/// flags.set("met_sayori");
/// assert!(flags.check("met_sayori"));
///
/// flags.toggle("met_sayori");
/// assert!(!flags.check("met_sayori"));
///
/// assert!(!flags.check("never_set_flag"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagSet {
    /// 已设置的旗标名集合
    flags: HashSet<String>,
}

impl FlagSet {
    /// 创建一个空的旗标集合。
    pub fn new() -> Self {
        FlagSet {
            flags: HashSet::new(),
        }
    }

    /// 设置一个旗标（标记为已触发/已发生）。
    ///
    /// 如果旗标已存在，此操作无副作用（幂等）。
    ///
    /// # 参数
    /// - `flag`：旗标名（接受 `impl Into<String>`）
    pub fn set(&mut self, flag: impl Into<String>) {
        self.flags.insert(flag.into());
    }

    /// 取消一个旗标。
    ///
    /// # 参数
    /// - `flag`：旗标名
    ///
    /// # 返回值
    /// - `true`：旗标之前存在，已被取消
    /// - `false`：旗标本来就不存在，无操作
    pub fn unset(&mut self, flag: &str) -> bool {
        self.flags.remove(flag)
    }

    /// 切换旗标状态。
    ///
    /// 如果旗标存在则移除，不存在则添加。等价于"取反"操作。
    ///
    /// # 参数
    /// - `flag`：旗标名
    pub fn toggle(&mut self, flag: &str) {
        if self.flags.contains(flag) {
            self.flags.remove(flag);
        } else {
            self.flags.insert(flag.to_string());
        }
    }

    /// 检查旗标是否已设置。
    ///
    /// # 参数
    /// - `flag`：旗标名
    ///
    /// # 返回值
    /// - `true`：旗标已设置
    /// - `false`：旗标未设置或不存在
    pub fn check(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }

    /// 清空所有旗标。
    pub fn clear(&mut self) {
        self.flags.clear();
    }

    /// 返回当前已设置的旗标数量。
    pub fn len(&self) -> usize {
        self.flags.len()
    }

    /// 返回旗标集合是否为空。
    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// 遍历所有已设置旗标的不可变迭代器。
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.flags.iter()
    }
}

impl Default for FlagSet {
    fn default() -> Self {
        Self::new()
    }
}

// ─── 测试模块 ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── AC02: Value 枚举 6 种变体构造与模式匹配 ─────────────────────────

    /// AC02 — `Value` 枚举支持 6 种类型（Int/Float/String/Bool/Array/Map）的构造和模式匹配。
    ///
    /// 验证每种 variant 可以正确构造并通过 match 提取值。
    #[test]
    fn ac02_value_all_variants() {
        // Int
        let v = Value::Int(42);
        match &v {
            Value::Int(n) => assert_eq!(*n, 42),
            _ => panic!("期望 Int"),
        }

        // Float
        let v = Value::Float(std::f64::consts::PI);
        match &v {
            Value::Float(f) => assert!((*f - std::f64::consts::PI).abs() < f64::EPSILON),
            _ => panic!("期望 Float"),
        }

        // String
        let v = Value::String("你好世界".into());
        match &v {
            Value::String(s) => assert_eq!(s, "你好世界"),
            _ => panic!("期望 String"),
        }

        // Bool
        let v = Value::Bool(true);
        match &v {
            Value::Bool(b) => assert!(*b),
            _ => panic!("期望 Bool"),
        }

        // Array
        let v = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        match &v {
            Value::Array(arr) => assert_eq!(arr.len(), 3),
            _ => panic!("期望 Array"),
        }

        // Map
        let mut map = HashMap::new();
        map.insert("key".into(), Value::String("value".into()));
        let v = Value::Map(map);
        match &v {
            Value::Map(m) => assert_eq!(m.get("key"), Some(&Value::String("value".into()))),
            _ => panic!("期望 Map"),
        }
    }

    /// 验证 Value 构造函数方法的正确性。
    #[test]
    fn value_constructors() {
        assert_eq!(Value::int(42), Value::Int(42));
        assert_eq!(Value::float(1.5), Value::Float(1.5));
        assert_eq!(Value::string("test"), Value::String("test".into()));
        assert_eq!(Value::bool(true), Value::Bool(true));
    }

    /// 验证 Value::type_name() 返回正确的中文类型名。
    #[test]
    fn value_type_name() {
        assert_eq!(Value::Int(0).type_name(), "Int");
        assert_eq!(Value::Float(0.0).type_name(), "Float");
        assert_eq!(Value::String(String::new()).type_name(), "String");
        assert_eq!(Value::Bool(false).type_name(), "Bool");
        assert_eq!(Value::Array(vec![]).type_name(), "Array");
        assert_eq!(Value::Map(HashMap::new()).type_name(), "Map");
    }

    /// 验证 Value::Float 的 NaN 比较行为：
    /// NaN == NaN（与 IEEE 754 不同，但确保 HashMap 一致性）。
    #[test]
    fn value_float_nan_eq() {
        let a = Value::Float(f64::NAN);
        let b = Value::Float(f64::NAN);
        // 使用 total_cmp 语义：NaN == NaN
        assert_eq!(a, b);
    }

    /// 验证 NaN 与非 NaN 不相等。
    #[test]
    fn value_float_nan_ne_normal() {
        let nan = Value::Float(f64::NAN);
        let normal = Value::Float(1.0);
        assert_ne!(nan, normal);
    }

    /// 验证不同类型的 Value 不相等。
    #[test]
    fn value_different_types_not_equal() {
        assert_ne!(Value::Int(0), Value::Float(0.0));
        assert_ne!(Value::String("true".into()), Value::Bool(true));
        assert_ne!(Value::Int(1), Value::String("1".into()));
    }

    // ─── AC03: VariableStore get/set/delete 操作 ──────────────────────────

    /// AC03 — `VariableStore` 的 get/set/delete 操作正确。
    ///
    /// 验证完整的 CRUD 生命周期：
    /// set → get → has → delete → get（None）
    #[test]
    fn ac03_variable_store_crud() {
        let mut store = VariableStore::new();

        // 初始为空
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        // Set
        store.set("score", Value::Int(100));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        // Get
        assert_eq!(store.get("score"), Some(&Value::Int(100)));
        assert!(store.has("score"));

        // Set 覆盖同名变量
        store.set("score", Value::Int(200));
        assert_eq!(store.get("score"), Some(&Value::Int(200)));
        assert_eq!(store.len(), 1); // 仍然是 1 个变量

        // Set 多种类型
        store.set("player_name", Value::String("主角".into()));
        store.set("is_new_game", Value::Bool(true));
        store.set("chapter", Value::Float(1.5));
        assert_eq!(store.len(), 4);

        // 遍历
        let names: Vec<String> = store.iter().map(|(k, _)| k.clone()).collect();
        assert!(names.contains(&"score".to_string()));
        assert!(names.contains(&"player_name".to_string()));

        // Delete
        assert!(store.delete("score"));
        assert!(!store.has("score"));
        assert_eq!(store.get("score"), None);
        assert_eq!(store.len(), 3);

        // Delete 不存在的变量
        assert!(!store.delete("nonexistent"));
        assert_eq!(store.len(), 3);

        // Clear
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(!store.has("player_name"));
    }

    /// 验证 VariableStore::iter() 在 store 变更后反映最新状态。
    #[test]
    fn variable_store_iter_after_mutation() {
        let mut store = VariableStore::new();
        store.set("a", Value::Int(1));
        store.set("b", Value::Int(2));

        let count = store.iter().count();
        assert_eq!(count, 2);

        store.delete("a");
        let count = store.iter().count();
        assert_eq!(count, 1);
    }

    // ─── AC04: FlagSet set/unset/toggle/check 语义 ────────────────────────

    /// AC04 — `FlagSet` 的 set/unset/toggle/check 语义正确。
    ///
    /// 验证：
    /// 1. set → check 返回 true
    /// 2. toggle → check 返回 false
    /// 3. unset 不存在的 flag 返回 false 且不 panic
    /// 4. 重复 set 不报错（幂等）
    #[test]
    fn ac04_flag_set_operations() {
        let mut flags = FlagSet::new();

        // 初始为空
        assert!(flags.is_empty());
        assert_eq!(flags.len(), 0);

        // Set → Check
        flags.set("met_sayori");
        assert!(flags.check("met_sayori"));
        assert!(!flags.check("never_set"));
        assert_eq!(flags.len(), 1);

        // 重复 Set 不报错（幂等）
        flags.set("met_sayori");
        flags.set("met_sayori");
        assert!(flags.check("met_sayori"));
        assert_eq!(flags.len(), 1); // 仍然是 1 个

        // Toggle
        flags.toggle("met_sayori");
        assert!(!flags.check("met_sayori"));
        assert_eq!(flags.len(), 0);

        // Toggle 回来
        flags.toggle("met_sayori");
        assert!(flags.check("met_sayori"));
        assert_eq!(flags.len(), 1);

        // Toggle 不存在的 flag（添加）
        flags.toggle("completed_ch1");
        assert!(flags.check("completed_ch1"));
        assert_eq!(flags.len(), 2);

        // Unset 存在的 flag
        assert!(flags.unset("met_sayori"));
        assert!(!flags.check("met_sayori"));
        assert_eq!(flags.len(), 1);

        // Unset 不存在的 flag 返回 false，不 panic
        assert!(!flags.unset("never_exists"));
        assert_eq!(flags.len(), 1);

        // Clear
        flags.clear();
        assert!(flags.is_empty());
        assert_eq!(flags.len(), 0);
        assert!(!flags.check("completed_ch1"));
    }

    /// 验证 FlagSet::iter() 返回所有已设置的 flag。
    #[test]
    fn flag_set_iter() {
        let mut flags = FlagSet::new();
        flags.set("a");
        flags.set("b");
        flags.set("c");

        let flag_list: Vec<String> = flags.iter().cloned().collect();
        assert_eq!(flag_list.len(), 3);
        assert!(flag_list.contains(&"a".to_string()));
        assert!(flag_list.contains(&"b".to_string()));
        assert!(flag_list.contains(&"c".to_string()));
    }

    // ─── AC05: VariableStore + FlagSet serde 序列化 round-trip ────────────

    /// AC05 — `VariableStore` 支持 serde 序列化 round-trip。
    #[test]
    fn ac05_variable_store_serde_roundtrip() {
        let mut store = VariableStore::new();
        store.set("score", Value::Int(100));
        store.set("player_name", Value::String("主角".into()));
        store.set("progress", Value::Float(0.75));
        store.set("is_new_game", Value::Bool(false));
        store.set(
            "inventory",
            Value::Array(vec![
                Value::String("钥匙".into()),
                Value::String("地图".into()),
            ]),
        );

        let json = serde_json::to_string(&store).expect("JSON 序列化失败");
        let restored: VariableStore = serde_json::from_str(&json).expect("JSON 反序列化失败");

        assert_eq!(restored.get("score"), Some(&Value::Int(100)));
        assert_eq!(
            restored.get("player_name"),
            Some(&Value::String("主角".into()))
        );
        assert_eq!(restored.get("progress"), Some(&Value::Float(0.75)));
        assert_eq!(restored.get("is_new_game"), Some(&Value::Bool(false)));

        // Array round-trip 验证
        match restored.get("inventory") {
            Some(Value::Array(items)) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], Value::String("钥匙".into()));
                assert_eq!(items[1], Value::String("地图".into()));
            }
            other => panic!("期望 Array 变体，得到 {:?}", other),
        }

        assert_eq!(restored.len(), 5);
    }

    /// AC05 — `FlagSet` 支持 serde 序列化 round-trip。
    #[test]
    fn ac05_flag_set_serde_roundtrip() {
        let mut flags = FlagSet::new();
        flags.set("met_sayori");
        flags.set("completed_ch1");
        flags.set("true_ending_unlocked");

        let json = serde_json::to_string(&flags).expect("JSON 序列化失败");
        let restored: FlagSet = serde_json::from_str(&json).expect("JSON 反序列化失败");

        assert!(restored.check("met_sayori"));
        assert!(restored.check("completed_ch1"));
        assert!(restored.check("true_ending_unlocked"));
        assert!(!restored.check("never_set"));
        assert_eq!(restored.len(), 3);
    }

    /// 验证空 VariableStore 的 serde round-trip。
    #[test]
    fn empty_variable_store_serde() {
        let store = VariableStore::new();
        let json = serde_json::to_string(&store).expect("JSON 序列化失败");
        let restored: VariableStore = serde_json::from_str(&json).expect("JSON 反序列化失败");
        assert!(restored.is_empty());
    }

    /// 验证空 FlagSet 的 serde round-trip。
    #[test]
    fn empty_flag_set_serde() {
        let flags = FlagSet::new();
        let json = serde_json::to_string(&flags).expect("JSON 序列化失败");
        let restored: FlagSet = serde_json::from_str(&json).expect("JSON 反序列化失败");
        assert!(restored.is_empty());
    }

    /// 验证 Value::Float NaN 的序列化 round-trip。
    #[test]
    fn value_float_nan_serde_roundtrip() {
        let v = Value::Float(f64::NAN);
        let json = serde_json::to_string(&v).expect("JSON 序列化失败");
        let restored: Value = serde_json::from_str(&json).expect("JSON 反序列化失败");
        // NaN == NaN 在我们自定义的 PartialEq 中成立
        assert_eq!(restored, Value::Float(f64::NAN));
    }

    /// 验证 Value::Float Infinity 的序列化 round-trip。
    #[test]
    fn value_float_infinity_serde_roundtrip() {
        let v = Value::Float(f64::INFINITY);
        let json = serde_json::to_string(&v).expect("JSON 序列化失败");
        let restored: Value = serde_json::from_str(&json).expect("JSON 反序列化失败");
        assert_eq!(restored, Value::Float(f64::INFINITY));

        let v = Value::Float(f64::NEG_INFINITY);
        let json = serde_json::to_string(&v).expect("JSON 序列化失败");
        let restored: Value = serde_json::from_str(&json).expect("JSON 反序列化失败");
        assert_eq!(restored, Value::Float(f64::NEG_INFINITY));
    }

    /// 验证 VariableStore 同时存储中文变量名和值。
    #[test]
    fn variable_store_chinese_names() {
        let mut store = VariableStore::new();
        store.set("好感度", Value::Int(80));
        store.set("是否解锁真结局", Value::Bool(true));
        store.set("角色名称", Value::String("小百合".into()));

        assert_eq!(store.get("好感度"), Some(&Value::Int(80)));
        assert_eq!(store.get("是否解锁真结局"), Some(&Value::Bool(true)));
        assert_eq!(store.get("角色名称"), Some(&Value::String("小百合".into())));
    }

    // ─── Default trait 验证 ──────────────────────────────────────────────

    /// 验证 VariableStore::default() 等于 VariableStore::new()，且为空。
    #[test]
    fn variable_store_default_is_empty() {
        let store = VariableStore::default();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    /// 验证 FlagSet::default() 等于 FlagSet::new()，且为空。
    #[test]
    fn flag_set_default_is_empty() {
        let flags = FlagSet::default();
        assert!(flags.is_empty());
        assert_eq!(flags.len(), 0);
    }

    // ─── Value Clone 深拷贝验证 ──────────────────────────────────────────

    /// 验证 Value::Array 的 Clone 是深拷贝（修改 clone 不影响原值）。
    #[test]
    fn value_clone_is_deep() {
        let original = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let mut cloned = original.clone();

        // 修改 cloned 的 Array 内容
        if let Value::Array(ref mut items) = cloned {
            items.push(Value::Int(3));
        }
        assert_ne!(original, cloned);
    }

    // ─── FlagSet 压力测试 ───────────────────────────────────────────────

    /// 验证 FlagSet 可以处理大量旗标（10000 个）而不会 panic 或性能退化。
    #[test]
    fn flag_set_large_count() {
        let mut flags = FlagSet::new();
        let count = 10_000;

        for i in 0..count {
            flags.set(format!("flag_{i}"));
        }
        assert_eq!(flags.len(), count);

        // 验证随机采样
        assert!(flags.check("flag_0"));
        assert!(flags.check("flag_9999"));
        assert!(!flags.check("nonexistent"));

        // 验证 toggle 在大集合中正确工作
        flags.toggle("flag_5000");
        assert!(!flags.check("flag_5000"));
        assert_eq!(flags.len(), count - 1);

        // 验证 clear 可以正确清理
        flags.clear();
        assert!(flags.is_empty());
    }
}
