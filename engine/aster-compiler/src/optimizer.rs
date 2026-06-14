//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/src/optimizer.rs
//! 功能概述：IR 优化器 — 包含 4 个优化 Pass：常量折叠、死标签消除、跳转合并、窥孔优化。
//!           在 IR 生成之后、字节码编码之前执行，减少指令数量并提升执行效率。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - crate::ir::IrInstruction（被优化的 IR 指令序列）
//!
//! ## 优化 Pass 顺序
//!
//! 1. **常量折叠** — 编译期求值，减少运算指令
//! 2. **死标签消除** — 移除不可达代码块
//! 3. **跳转合并** — 缩短跳转链
//! 4. **窥孔优化** — 局部指令模式匹配
//!
//! ## 设计说明
//!
//! 每个 Pass 独立实现，可单独开关。Pass 间有顺序依赖（如常量折叠后可能产生新的
//! 死代码，但当前不重复执行其他 Pass）。
//!
//! 所有优化不改变程序语义（AC05），优化后指令数 ≤ 优化前（AC04）。

use std::collections::{HashMap, HashSet};

use crate::ir::IrInstruction;

// ============================================================================
// ConstValue — 常量值追踪
// ============================================================================

/// 编译期已知的常量值，用于常量折叠 Pass。
///
/// 寄存器可能持有以下类型的常量值：
/// - Int：整数
/// - Float：浮点数
/// - Bool：布尔值
/// - Str：字符串（存储 pool_idx 而非字符串本身，因为字符串在常量池中）
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ConstValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    /// 字符串常量的池索引（当前未在折叠中使用，预留）
    Str(u16),
}

// ============================================================================
// OptimizeStats — 优化统计
// ============================================================================

/// 优化统计 — 记录一次 optimize() 调用中各 Pass 的应用次数。
///
/// 用于调试优化效果和向用户展示优化收益。
#[derive(Debug, Clone, Default)]
pub struct OptimizeStats {
    /// 优化前 IR 指令数
    pub instructions_before: usize,
    /// 优化后 IR 指令数
    pub instructions_after: usize,
    /// 常量折叠次数
    pub folds: usize,
    /// 消除的死标签数（对应代码块）
    pub dead_labels: usize,
    /// 跳转合并次数
    pub jumps_threaded: usize,
    /// 窥孔优化应用次数
    pub peephole_applied: usize,
}

// ============================================================================
// Optimizer — 优化器
// ============================================================================

/// IR 优化器 — 对 IrInstruction 序列应用 4 个优化 Pass。
///
/// # 使用示例
/// ```
/// use aster_compiler::ir::IrInstruction;
/// use aster_compiler::Optimizer;
///
/// let mut ir = vec![
///     IrInstruction::PushInt { reg: 0, value: 2 },
///     IrInstruction::PushInt { reg: 1, value: 3 },
///     IrInstruction::Add { dst: 2, left: 0, right: 1 },
/// ];
///
/// let optimizer = Optimizer::new();
/// let stats = optimizer.optimize(&mut ir);
/// assert!(stats.folds > 0); // 2+3 被折叠为 5
/// ```
#[derive(Debug, Default)]
pub struct Optimizer {}

impl Optimizer {
    /// 创建一个新的优化器实例。
    pub fn new() -> Self {
        Optimizer {}
    }

    /// 对 IR 指令序列执行全部 4 个优化 Pass。
    ///
    /// Pass 执行顺序：常量折叠 → 死标签消除 → 跳转合并 → 窥孔优化。
    ///
    /// # 参数
    /// - `ir`：可变引用，指向待优化的 IR 指令序列
    ///
    /// # 返回值
    /// - `OptimizeStats`：各 Pass 的应用次数和前后指令数对比
    pub fn optimize(&self, ir: &mut Vec<IrInstruction>) -> OptimizeStats {
        let instructions_before = ir.len();

        let folds = self.constant_folding(ir);
        let dead_labels = self.dead_label_elimination(ir);
        let jumps_threaded = self.jump_threading(ir);
        let peephole_applied = self.peephole(ir);

        let instructions_after = ir.len();

        OptimizeStats {
            instructions_before,
            instructions_after,
            folds,
            dead_labels,
            jumps_threaded,
            peephole_applied,
        }
    }

    // ========================================================================
    // Pass 1: 常量折叠
    // ========================================================================

    /// 常量折叠 — 编译期求值算术/比较/逻辑/一元运算（重写版本，修复 borrow 冲突）。
    fn constant_folding(&self, ir: &mut Vec<IrInstruction>) -> usize {
        let mut folds: usize = 0;
        let mut constants: HashMap<u8, ConstValue> = HashMap::new();
        // 收集待应用的替换：(index, new_instruction)
        let mut replacements: HashMap<usize, IrInstruction> = HashMap::new();
        // 标记待删除的指令索引
        let mut to_remove: HashSet<usize> = HashSet::new();

        for (i, inst) in ir.iter().enumerate() {
            match inst {
                IrInstruction::PushInt { reg, value } => {
                    constants.insert(*reg, ConstValue::Int(*value));
                }
                IrInstruction::PushFloat { reg, value } => {
                    constants.insert(*reg, ConstValue::Float(*value));
                }
                IrInstruction::PushBool { reg, value } => {
                    constants.insert(*reg, ConstValue::Bool(*value));
                }
                IrInstruction::PushStr { reg, str_idx } => {
                    constants.insert(*reg, ConstValue::Str(*str_idx));
                }
                IrInstruction::LoadVar { dst, .. } => {
                    constants.remove(dst);
                }
                _ => {}
            }

            // 尝试折叠二元/一元运算
            let folded = self.try_fold(i, inst, &constants, &mut to_remove, ir);
            if let Some(new_inst) = folded {
                replacements.insert(i, new_inst.0);
                folds += 1;
                // 更新常量映射
                if let Some((reg, value)) = new_inst.1 {
                    constants.insert(reg, value);
                }
            }
        }

        // 应用替换
        for (&idx, new_inst) in &replacements {
            ir[idx] = new_inst.clone();
        }

        // 移除标记为删除的指令
        let mut new_ir: Vec<IrInstruction> = Vec::with_capacity(ir.len() - to_remove.len());
        for (i, inst) in ir.iter().enumerate() {
            if !to_remove.contains(&i) {
                new_ir.push(inst.clone());
            }
        }
        *ir = new_ir;

        folds
    }

    /// 尝试折叠单条指令。返回 Some((新指令, Some((寄存器, 新常量值)))) 或 None。
    fn try_fold(
        &self,
        idx: usize,
        inst: &IrInstruction,
        constants: &HashMap<u8, ConstValue>,
        to_remove: &mut HashSet<usize>,
        ir: &[IrInstruction],
    ) -> Option<(IrInstruction, Option<(u8, ConstValue)>)> {
        match inst {
            IrInstruction::Add { dst, left, right } => {
                if let Some(result) = self.fold_binary_int(*left, *right, constants) {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushInt {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Int(result))),
                    ));
                }
                if let Some(result) = self.fold_binary_float(*left, *right, constants) {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushFloat {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Float(result))),
                    ));
                }
            }
            IrInstruction::Sub { dst, left, right } => {
                if let Some(result) =
                    self.fold_binary_int_op(*left, *right, constants, |a, b| a - b)
                {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushInt {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Int(result))),
                    ));
                }
                if let Some(result) =
                    self.fold_binary_float_op(*left, *right, constants, |a, b| a - b)
                {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushFloat {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Float(result))),
                    ));
                }
            }
            IrInstruction::Mul { dst, left, right } => {
                if let Some(result) =
                    self.fold_binary_int_op(*left, *right, constants, |a, b| a * b)
                {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushInt {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Int(result))),
                    ));
                }
                if let Some(result) =
                    self.fold_binary_float_op(*left, *right, constants, |a, b| a * b)
                {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushFloat {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Float(result))),
                    ));
                }
            }
            IrInstruction::Div { dst, left, right } => {
                if let Some(result) = self.fold_binary_float_op(*left, *right, constants, |a, b| {
                    if b == 0.0 { f64::NAN } else { a / b }
                }) {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushFloat {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Float(result))),
                    ));
                }
            }
            IrInstruction::Eq { dst, left, right }
            | IrInstruction::Neq { dst, left, right }
            | IrInstruction::Lt { dst, left, right }
            | IrInstruction::Gt { dst, left, right }
            | IrInstruction::Le { dst, left, right }
            | IrInstruction::Ge { dst, left, right } => {
                if let Some(result) = self.fold_comparison(inst, *left, *right, constants) {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushBool {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Bool(result))),
                    ));
                }
            }
            IrInstruction::And { dst, left, right } => {
                if let Some(result) = self.fold_binary_bool(*left, *right, constants, |a, b| a && b)
                {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushBool {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Bool(result))),
                    ));
                }
            }
            IrInstruction::Or { dst, left, right } => {
                if let Some(result) = self.fold_binary_bool(*left, *right, constants, |a, b| a || b)
                {
                    self.mark_operand_removal(to_remove, idx, *left, *right, ir);
                    return Some((
                        IrInstruction::PushBool {
                            reg: *dst,
                            value: result,
                        },
                        Some((*dst, ConstValue::Bool(result))),
                    ));
                }
            }
            IrInstruction::Not { dst, src } => {
                if let Some(ConstValue::Bool(v)) = constants.get(src) {
                    return Some((
                        IrInstruction::PushBool {
                            reg: *dst,
                            value: !v,
                        },
                        Some((*dst, ConstValue::Bool(!v))),
                    ));
                }
            }
            IrInstruction::Neg { dst, src } => {
                if let Some(ConstValue::Int(v)) = constants.get(src) {
                    return Some((
                        IrInstruction::PushInt {
                            reg: *dst,
                            value: -v,
                        },
                        Some((*dst, ConstValue::Int(-v))),
                    ));
                }
                if let Some(ConstValue::Float(v)) = constants.get(src) {
                    return Some((
                        IrInstruction::PushFloat {
                            reg: *dst,
                            value: -v,
                        },
                        Some((*dst, ConstValue::Float(-v))),
                    ));
                }
            }
            _ => {}
        }
        None
    }

    /// 标记被折叠的操作数 Push* 指令为待删除。
    fn mark_operand_removal(
        &self,
        to_remove: &mut HashSet<usize>,
        current_idx: usize,
        left_reg: u8,
        right_reg: u8,
        ir: &[IrInstruction],
    ) {
        // 向前扫描最近的对 left_reg 和 right_reg 的 Push* 指令
        let mut found_left = false;
        let mut found_right = false;
        for j in (0..current_idx).rev() {
            if found_left && found_right {
                break;
            }
            if !found_left && self.is_push_to_reg(&ir[j], left_reg) {
                to_remove.insert(j);
                found_left = true;
            }
            if !found_right && self.is_push_to_reg(&ir[j], right_reg) {
                to_remove.insert(j);
                found_right = true;
            }
        }
    }

    /// 判断指令是否是对指定寄存器的 Push* 操作。
    fn is_push_to_reg(&self, inst: &IrInstruction, reg: u8) -> bool {
        match inst {
            IrInstruction::PushInt { reg: r, .. }
            | IrInstruction::PushFloat { reg: r, .. }
            | IrInstruction::PushBool { reg: r, .. }
            | IrInstruction::PushStr { reg: r, .. }
            | IrInstruction::LoadVar { dst: r, .. } => *r == reg,
            _ => false,
        }
    }

    /// 尝试对整型二元运算进行常量折叠。
    fn fold_binary_int(
        &self,
        left: u8,
        right: u8,
        constants: &HashMap<u8, ConstValue>,
    ) -> Option<i64> {
        self.fold_binary_int_op(left, right, constants, |a, b| a + b)
    }

    /// 尝试对整型二元运算（自定义操作）进行常量折叠。
    fn fold_binary_int_op<F>(
        &self,
        left: u8,
        right: u8,
        constants: &HashMap<u8, ConstValue>,
        op: F,
    ) -> Option<i64>
    where
        F: Fn(i64, i64) -> i64,
    {
        if let Some(ConstValue::Int(a)) = constants.get(&left)
            && let Some(ConstValue::Int(b)) = constants.get(&right)
        {
            return Some(op(*a, *b));
        }
        None
    }

    /// 尝试对浮点二元运算进行常量折叠。
    fn fold_binary_float(
        &self,
        left: u8,
        right: u8,
        constants: &HashMap<u8, ConstValue>,
    ) -> Option<f64> {
        self.fold_binary_float_op(left, right, constants, |a, b| a + b)
    }

    /// 尝试对浮点二元运算（自定义操作）进行常量折叠。
    fn fold_binary_float_op<F>(
        &self,
        left: u8,
        right: u8,
        constants: &HashMap<u8, ConstValue>,
        op: F,
    ) -> Option<f64>
    where
        F: Fn(f64, f64) -> f64,
    {
        // 尝试 Int + Int → 提升为 Float
        let a = match constants.get(&left) {
            Some(ConstValue::Int(v)) => Some(*v as f64),
            Some(ConstValue::Float(v)) => Some(*v),
            _ => None,
        };
        let b = match constants.get(&right) {
            Some(ConstValue::Int(v)) => Some(*v as f64),
            Some(ConstValue::Float(v)) => Some(*v),
            _ => None,
        };
        if let (Some(a), Some(b)) = (a, b) {
            return Some(op(a, b));
        }
        None
    }

    /// 尝试对布尔二元运算进行常量折叠。
    fn fold_binary_bool<F>(
        &self,
        left: u8,
        right: u8,
        constants: &HashMap<u8, ConstValue>,
        op: F,
    ) -> Option<bool>
    where
        F: Fn(bool, bool) -> bool,
    {
        if let Some(ConstValue::Bool(a)) = constants.get(&left)
            && let Some(ConstValue::Bool(b)) = constants.get(&right)
        {
            return Some(op(*a, *b));
        }
        None
    }

    /// 尝试对比较运算进行常量折叠。
    fn fold_comparison(
        &self,
        inst: &IrInstruction,
        left: u8,
        right: u8,
        constants: &HashMap<u8, ConstValue>,
    ) -> Option<bool> {
        // 提取比较操作的类型
        let cmp_fn: fn(f64, f64) -> bool = match inst {
            IrInstruction::Eq { .. } => |a, b| (a - b).abs() < f64::EPSILON,
            IrInstruction::Neq { .. } => |a, b| (a - b).abs() >= f64::EPSILON,
            IrInstruction::Lt { .. } => |a, b| a < b,
            IrInstruction::Gt { .. } => |a, b| a > b,
            IrInstruction::Le { .. } => |a, b| a <= b,
            IrInstruction::Ge { .. } => |a, b| a >= b,
            _ => return None,
        };

        let a = match constants.get(&left) {
            Some(ConstValue::Int(v)) => Some(*v as f64),
            Some(ConstValue::Float(v)) => Some(*v),
            _ => None,
        };
        let b = match constants.get(&right) {
            Some(ConstValue::Int(v)) => Some(*v as f64),
            Some(ConstValue::Float(v)) => Some(*v),
            _ => None,
        };
        if let (Some(a), Some(b)) = (a, b) {
            return Some(cmp_fn(a, b));
        }
        None
    }

    // ========================================================================
    // Pass 2: 死标签消除
    // ========================================================================

    /// 死标签消除 — 移除不可达的 Label 及其后续指令块。
    ///
    /// # 算法
    ///
    /// 1. 收集所有被跳转指令引用的标签名
    /// 2. 从指令 0 开始线性扫描，跟踪 fall-through 可达性
    /// 3. 若 Label 不可达（既未被引用也无法 fall-through 到达），
    ///    删除该 Label 及后续指令直到下一个 Label/End
    fn dead_label_elimination(&self, ir: &mut Vec<IrInstruction>) -> usize {
        if ir.is_empty() {
            return 0;
        }

        // 步骤 1：构建 label_name → instruction_index 映射
        let label_map = self.build_label_map(ir);

        // 步骤 2：收集所有被引用的标签
        let referenced_labels = self.collect_referenced_labels(ir);

        // 步骤 3：标记可达指令
        let reachable = self.compute_reachable(ir, &label_map, &referenced_labels);

        // 步骤 4：移除不可达指令块
        let mut new_ir: Vec<IrInstruction> = Vec::with_capacity(ir.len());
        let mut in_dead_block = false;
        let mut dead_labels_removed: usize = 0;

        for (i, inst) in ir.iter().enumerate() {
            if let IrInstruction::Label { name } = inst {
                // 判断此 Label 是否应该保留：
                // 1. 可达 → 保留
                // 2. 不可达但是用户定义标签（非 @ 前缀）→ 保守保留
                //    （可能是跨场景 Goto 目标 或 存档引用点）
                // 3. 不可达的自动生成标签（@ 前缀）→ 移除
                if reachable.contains(&i) || !name.starts_with('@') {
                    in_dead_block = false;
                    new_ir.push(inst.clone());
                } else {
                    in_dead_block = true;
                    dead_labels_removed += 1;
                }
            } else if in_dead_block {
                // 跳过死块内的指令
                if matches!(inst, IrInstruction::Label { .. } | IrInstruction::End) {
                    // 遇到下一个 Label 或 End 时结束死块
                    if reachable.contains(&i) {
                        in_dead_block = false;
                        new_ir.push(inst.clone());
                    }
                }
            } else {
                new_ir.push(inst.clone());
            }
        }

        *ir = new_ir;
        dead_labels_removed
    }

    /// 构建标签名 → 指令索引映射
    fn build_label_map(&self, ir: &[IrInstruction]) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for (i, inst) in ir.iter().enumerate() {
            if let IrInstruction::Label { name } = inst {
                map.insert(name.clone(), i);
            }
        }
        map
    }

    /// 收集所有被跳转/调用指令引用的标签名
    fn collect_referenced_labels(&self, ir: &[IrInstruction]) -> HashSet<String> {
        let mut refs = HashSet::new();
        for inst in ir {
            match inst {
                IrInstruction::Jump { target }
                | IrInstruction::JumpIf { target, .. }
                | IrInstruction::JumpIfFlag { target, .. }
                | IrInstruction::Call { target } => {
                    refs.insert(target.clone());
                }
                _ => {}
            }
        }
        refs
    }

    /// 计算可达指令索引集合
    fn compute_reachable(
        &self,
        ir: &[IrInstruction],
        label_map: &HashMap<String, usize>,
        _referenced_labels: &HashSet<String>,
    ) -> HashSet<usize> {
        let mut reachable = HashSet::new();
        if ir.is_empty() {
            return reachable;
        }

        // 深度优先搜索可达指令
        let mut stack: Vec<usize> = vec![0]; // 从第一条指令开始
        let mut visited = HashSet::new();

        while let Some(idx) = stack.pop() {
            if idx >= ir.len() || visited.contains(&idx) {
                continue;
            }
            visited.insert(idx);
            reachable.insert(idx);

            let inst = &ir[idx];

            match inst {
                IrInstruction::Jump { target } => {
                    if let Some(&target_idx) = label_map.get(target) {
                        stack.push(target_idx);
                    }
                    // Jump 不 fall-through（无条件跳转）
                }
                IrInstruction::JumpIf { target, .. } | IrInstruction::JumpIfFlag { target, .. } => {
                    if let Some(&target_idx) = label_map.get(target) {
                        stack.push(target_idx);
                    }
                    // 条件跳转会 fall-through
                    stack.push(idx + 1);
                }
                IrInstruction::Call { target } => {
                    if let Some(&target_idx) = label_map.get(target) {
                        stack.push(target_idx);
                    }
                    // Call 之后会返回，所以会 fall-through
                    stack.push(idx + 1);
                }
                IrInstruction::Return | IrInstruction::End => {
                    // 不 fall-through
                }
                _ => {
                    // 其他指令默认 fall-through
                    stack.push(idx + 1);
                }
            }
        }

        reachable
    }

    // ========================================================================
    // Pass 3: 跳转合并
    // ========================================================================

    /// 跳转合并 — 缩短跳转链。
    ///
    /// 检测模式：
    /// - `Jump L1; ...; Label L1; Jump L2` → `Jump L2`
    /// - `JumpIf(r, L1); ...; Label L1; Jump L2` → `JumpIf(r, L2)`
    ///
    /// 迭代执行直到无变化（固定点）。
    fn jump_threading(&self, ir: &mut [IrInstruction]) -> usize {
        let mut total_threaded: usize = 0;

        loop {
            let label_map = self.build_label_map(ir);
            let mut threaded: usize = 0;

            for i in 0..ir.len() {
                let result = match &ir[i] {
                    IrInstruction::Jump { target } => self
                        .label_leads_to_jump(target, &label_map, ir)
                        .map(|nested_target| IrInstruction::Jump {
                            target: nested_target,
                        }),
                    IrInstruction::JumpIf { reg, target } => self
                        .label_leads_to_jump(target, &label_map, ir)
                        .map(|nested_target| IrInstruction::JumpIf {
                            reg: *reg,
                            target: nested_target,
                        }),
                    _ => None,
                };

                if let Some(new_inst) = result {
                    ir[i] = new_inst;
                    threaded += 1;
                }
            }

            total_threaded += threaded;
            if threaded == 0 {
                break; // 固定点
            }
        }

        total_threaded
    }

    /// 检查标签后的第一条有效指令是否是 Jump。
    ///
    /// 如果是，返回该 Jump 的目标标签名（跳转链的下一个节点）；
    /// 否则返回 None。
    fn label_leads_to_jump(
        &self,
        label_name: &str,
        label_map: &HashMap<String, usize>,
        ir: &[IrInstruction],
    ) -> Option<String> {
        let &label_idx = label_map.get(label_name)?;

        // 从 label 的下一条指令开始扫描
        for inst in ir.iter().skip(label_idx + 1) {
            match inst {
                // 跳过其他 Label 指令
                IrInstruction::Label { .. } => continue,
                // 找到 Jump → 返回其目标
                IrInstruction::Jump { target } => return Some(target.clone()),
                // 其他指令 → 标签后不是直接的 Jump，停止
                _ => return None,
            }
        }

        None
    }

    // ========================================================================
    // Pass 4: 窥孔优化
    // ========================================================================

    /// 窥孔优化 — 滑动窗口扫描相邻指令，应用局部模式匹配。
    ///
    /// 模式：
    /// 1. `PushBool(r, true)` + `JumpIf(r, L)` → `Jump L`
    /// 2. `PushBool(r, false)` / `PushInt(r, 0)` + `JumpIf(r, L)` → 删除两者
    /// 3. 同一寄存器连续 Push* → 删除前一条
    fn peephole(&self, ir: &mut Vec<IrInstruction>) -> usize {
        let mut applied: usize = 0;
        let mut i: usize = 0;

        while i + 1 < ir.len() {
            let matched = match_peephole_pattern(&ir[i], &ir[i + 1]);

            match matched {
                PeepholeResult::ReplaceFirst(new_inst) => {
                    ir[i] = new_inst;
                    ir.remove(i + 1);
                    applied += 1;
                    // 不递增 i，检查新指令和后续指令的模式
                }
                PeepholeResult::RemoveBoth => {
                    ir.remove(i); // 删除第一条，第二条现在移到 i
                    ir.remove(i); // 删除原第二条
                    applied += 1;
                    // 不递增 i，原地检查新指令
                }
                PeepholeResult::None => {
                    i += 1;
                }
            }
        }

        applied
    }
}

/// 窥孔模式匹配结果
enum PeepholeResult {
    /// 无匹配
    None,
    /// 用新指令替换第一条，删除第二条
    ReplaceFirst(IrInstruction),
    /// 删除两条指令
    RemoveBoth,
}

fn match_peephole_pattern(first: &IrInstruction, second: &IrInstruction) -> PeepholeResult {
    // 模式 1+2：PushBool/PushInt + JumpIf → 优化
    if let IrInstruction::JumpIf { reg, target } = second {
        match first {
            IrInstruction::PushBool {
                reg: r,
                value: true,
            } if r == reg => {
                // PushBool(r, true) + JumpIf(r, L) → Jump L
                return PeepholeResult::ReplaceFirst(IrInstruction::Jump {
                    target: target.clone(),
                });
            }
            IrInstruction::PushBool {
                reg: r,
                value: false,
            } if r == reg => {
                // PushBool(r, false) + JumpIf(r, L) → 删除两者（死代码）
                return PeepholeResult::RemoveBoth;
            }
            IrInstruction::PushInt { reg: r, value: 0 } if r == reg => {
                // PushInt(r, 0) + JumpIf(r, L) → 删除两者（0=false，永远不跳）
                return PeepholeResult::RemoveBoth;
            }
            _ => {}
        }
    }

    // 模式 3：同一寄存器连续 Push* → 删除前一条
    match (first, second) {
        (
            IrInstruction::PushInt { reg: r1, .. }
            | IrInstruction::PushFloat { reg: r1, .. }
            | IrInstruction::PushStr { reg: r1, .. }
            | IrInstruction::PushBool { reg: r1, .. },
            IrInstruction::PushInt { reg: r2, .. }
            | IrInstruction::PushFloat { reg: r2, .. }
            | IrInstruction::PushStr { reg: r2, .. }
            | IrInstruction::PushBool { reg: r2, .. },
        ) if r1 == r2 => {
            return PeepholeResult::ReplaceFirst(second.clone());
        }
        _ => {}
    }

    PeepholeResult::None
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use IrInstruction::*;

    // ─── 辅助函数 ────────────────────────────────────────────────────────

    fn push_int(reg: u8, v: i64) -> IrInstruction {
        PushInt { reg, value: v }
    }
    fn push_float(reg: u8, v: f64) -> IrInstruction {
        PushFloat { reg, value: v }
    }
    fn push_bool(reg: u8, v: bool) -> IrInstruction {
        PushBool { reg, value: v }
    }
    fn add(dst: u8, l: u8, r: u8) -> IrInstruction {
        Add {
            dst,
            left: l,
            right: r,
        }
    }
    fn jump(target: &str) -> IrInstruction {
        Jump {
            target: target.into(),
        }
    }
    fn jump_if(reg: u8, target: &str) -> IrInstruction {
        JumpIf {
            reg,
            target: target.into(),
        }
    }
    fn label(name: &str) -> IrInstruction {
        Label { name: name.into() }
    }
    fn end_inst() -> IrInstruction {
        End
    }

    // ─── AC01: 常量折叠 ─────────────────────────────────────────────────

    /// AC01 — `$x = 2 + 3` 折叠为单个 PushInt(5)。
    #[test]
    fn ac01_constant_folding_int_add() {
        let mut ir = vec![push_int(0, 2), push_int(1, 3), add(2, 0, 1)];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);

        assert!(stats.folds > 0, "应触发常量折叠");
        // 验证 Add 已被替换为 PushInt
        let has_add = ir.iter().any(|inst| matches!(inst, Add { .. }));
        assert!(!has_add, "Add 指令应已被折叠");
        // 验证结果包含 PushInt(reg, 5)
        let has_result = ir
            .iter()
            .any(|inst| matches!(inst, PushInt { value: 5, .. }));
        assert!(has_result, "应有 PushInt(5) 作为折叠结果");
    }

    /// AC01 补充 — 浮点常量折叠。
    #[test]
    fn ac01_constant_folding_float() {
        let mut ir = vec![
            push_float(0, 1.5),
            push_float(1, 2.5),
            IrInstruction::Mul {
                dst: 2,
                left: 0,
                right: 1,
            },
        ];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);

        assert!(stats.folds > 0);
        let has_mul = ir.iter().any(|inst| matches!(inst, Mul { .. }));
        assert!(!has_mul, "Mul 应已被折叠");
        let has_result = ir.iter().any(
            |inst| matches!(inst, PushFloat { value, .. } if (value - 3.75).abs() < f64::EPSILON),
        );
        assert!(has_result, "应有 PushFloat(3.75)");
    }

    /// AC01 补充 — 布尔常量折叠（And）。
    #[test]
    fn ac01_constant_folding_bool_and() {
        let mut ir = vec![
            push_bool(0, true),
            push_bool(1, false),
            IrInstruction::And {
                dst: 2,
                left: 0,
                right: 1,
            },
        ];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);

        assert!(stats.folds > 0);
        let has_and = ir.iter().any(|inst| matches!(inst, And { .. }));
        assert!(!has_and);
        let has_result = ir
            .iter()
            .any(|inst| matches!(inst, PushBool { value: false, .. }));
        assert!(has_result, "true && false = false");
    }

    /// AC01 补充 — 比较运算折叠。
    #[test]
    fn ac01_constant_folding_comparison() {
        let mut ir = vec![
            push_int(0, 10),
            push_int(1, 5),
            IrInstruction::Gt {
                dst: 2,
                left: 0,
                right: 1,
            },
        ];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);

        assert!(stats.folds > 0);
        let has_gt = ir.iter().any(|inst| matches!(inst, Gt { .. }));
        assert!(!has_gt);
        let has_result = ir
            .iter()
            .any(|inst| matches!(inst, PushBool { value: true, .. }));
        assert!(has_result, "10 > 5 = true");
    }

    /// AC01 补充 — 一元取负折叠。
    #[test]
    fn ac01_constant_folding_neg() {
        let mut ir = vec![push_int(0, 42), IrInstruction::Neg { dst: 1, src: 0 }];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);

        assert!(stats.folds > 0);
        let has_result = ir
            .iter()
            .any(|inst| matches!(inst, PushInt { value: -42, .. }));
        assert!(has_result, "-42");
    }

    /// 验证非常量运算不被错误折叠。
    #[test]
    fn non_constant_not_folded() {
        let mut ir = vec![
            PushInt { reg: 0, value: 2 },
            IrInstruction::LoadVar {
                dst: 1,
                name_idx: 0,
            },
            Add {
                dst: 2,
                left: 0,
                right: 1,
            },
        ];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);
        // 不应折叠，因为 r1 来自 LoadVar，非编译期常量
        assert_eq!(stats.folds, 0, "含 LoadVar 的表达式不应被折叠");
    }

    // ─── AC02: 死标签消除 ─────────────────────────────────────────────

    /// AC02 — 不可达自动生成标签（@前缀）及其后续代码被移除。
    /// 用户定义标签总是保留（可能是跨场景 Goto 目标）。
    #[test]
    fn ac02_dead_label_elimination() {
        let mut ir = vec![
            jump("end"),
            label("@dead"), // 自动生成标签，不可达 → 应移除
            push_int(0, 1), // 不可达代码
            push_int(1, 2),
            label("end"), // 用户标签，保留
            end_inst(),
        ];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);

        assert!(stats.dead_labels > 0, "应消除至少一个死标签");
        // 验证 @dead 标签不再存在
        let has_dead_label = ir
            .iter()
            .any(|inst| matches!(inst, Label { name } if name == "@dead"));
        assert!(!has_dead_label, "@dead 标签应被移除");
        // 验证 end 标签仍在
        let has_end_label = ir
            .iter()
            .any(|inst| matches!(inst, Label { name } if name == "end"));
        assert!(has_end_label, "end 标签应保留");
    }

    /// AC02 补充 — 无条件跳转前的 Label 不可达？不，只有未引用+不可达的才移除。
    #[test]
    fn ac02_referenced_label_preserved() {
        let mut ir = vec![
            jump_if(0, "target"),
            jump("after"),
            label("target"), // 被 JumpIf 引用，应保留
            push_int(1, 99),
            label("after"),
            end_inst(),
        ];
        let optimizer = Optimizer::new();
        optimizer.optimize(&mut ir);

        let has_target = ir
            .iter()
            .any(|inst| matches!(inst, Label { name } if name == "target"));
        assert!(has_target, "被引用的标签应保留");
    }

    // ─── AC03: 跳转合并 ─────────────────────────────────────────────────

    /// AC03 — Jump L1; Label L1; Jump L2 → 合并为 Jump L2。
    #[test]
    fn ac03_jump_threading() {
        let mut ir = vec![jump("L1"), label("L1"), jump("L2"), label("L2"), end_inst()];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);

        assert!(stats.jumps_threaded > 0, "应触发跳转合并");
        // 第一条 Jump 的目标应该是 L2（被合并）
        if let Jump { target } = &ir[0] {
            assert_eq!(target, "L2", "Jump L1 应被合并为 Jump L2");
        } else {
            panic!("第一条指令应为 Jump");
        }
    }

    /// AC03 补充 — JumpIf 到无条件跳转的合并。
    #[test]
    fn ac03_jump_if_threading() {
        let mut ir = vec![
            jump_if(0, "L1"),
            label("L1"),
            jump("L2"),
            label("L2"),
            end_inst(),
        ];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);
        assert!(stats.jumps_threaded > 0);

        if let JumpIf { target, .. } = &ir[0] {
            assert_eq!(target, "L2", "JumpIf L1 应合并为 JumpIf L2");
        }
    }

    // ─── AC04: 优化不增加指令数 ────────────────────────────────────────

    /// AC04 — 验证优化后指令数 ≤ 优化前（非退化保证）。
    #[test]
    fn ac04_no_instruction_increase() {
        // 构造一个可优化的 IR：含有常量折叠 + 死标签 + 跳转合并
        let mut ir = vec![
            push_int(0, 3),
            push_int(1, 4),
            add(2, 0, 1), // 可折叠为 7
            jump("skip"),
            label("dead"), // 死标签
            push_int(0, 99),
            label("skip"),
            jump("final_target"),
            label("final_target"),
            end_inst(),
        ];
        let before = ir.len();
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);

        assert!(
            stats.instructions_after <= stats.instructions_before,
            "优化后不应增加指令数: {} → {}",
            before,
            ir.len()
        );
        assert!(
            stats.folds > 0 || stats.dead_labels > 0 || stats.jumps_threaded > 0,
            "应至少触发一种优化"
        );
    }

    // ─── AC05: 语义保留 ─────────────────────────────────────────────────

    /// AC05 — 优化后 IR 结构完整性检查。
    ///
    /// 验证：(1) 所有跳转目标标签存在；(2) End 指令存在；
    /// (3) 跳转目标偏移合理。
    #[test]
    fn ac05_semantic_preservation() {
        let mut ir = vec![
            jump("start"),
            label("unused"), // 死标签
            push_int(0, 1),
            label("start"),
            push_int(1, 2),
            push_int(2, 3),
            add(3, 1, 2), // 可折叠
            push_bool(4, true),
            jump_if(4, "end_label"),
            push_int(5, 0), // 死代码？
            label("end_label"),
            end_inst(),
        ];
        let optimizer = Optimizer::new();
        let _stats = optimizer.optimize(&mut ir);

        // 验证 1：所有跳转目标存在
        let labels: HashSet<String> = ir
            .iter()
            .filter_map(|inst| {
                if let Label { name } = inst {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        for inst in ir.iter() {
            match inst {
                Jump { target } | JumpIf { target, .. } | Call { target, .. } => {
                    assert!(
                        labels.contains(target),
                        "跳转目标 '{}' 应在 IR 中存在",
                        target
                    );
                }
                _ => {}
            }
        }

        // 验证 2：End 指令存在（场景正确终止）
        let has_end = ir.iter().any(|inst| matches!(inst, End));
        assert!(has_end, "优化后 IR 应以 End 结尾");

        // 验证 3：编译期折叠的常量值正确
        let has_folded = ir
            .iter()
            .any(|inst| matches!(inst, PushInt { value: 5, .. }));
        assert!(has_folded, "2+3 应折叠为 PushInt(5)");
    }

    // ─── OptimizeStats 基础测试 ───────────────────────────────────────

    /// 验证空 IR 优化不 panic。
    #[test]
    fn optimize_empty_ir() {
        let mut ir: Vec<IrInstruction> = vec![];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);
        assert_eq!(stats.instructions_before, 0);
        assert_eq!(stats.instructions_after, 0);
    }

    /// 验证仅含 End 的 IR 优化后不变。
    #[test]
    fn optimize_end_only() {
        let mut ir = vec![end_inst()];
        let optimizer = Optimizer::new();
        let stats = optimizer.optimize(&mut ir);
        assert_eq!(stats.instructions_before, 1);
        assert_eq!(stats.instructions_after, 1);
    }
}
