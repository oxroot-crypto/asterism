//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/src/bytecode.rs
//! 功能概述：字节码定义 — 定义 `Opcode` 枚举（变长操作码，1 byte op + N bytes operands）、
//!           `CompiledScene` 结构体（指令序列 + 常量池 + 标签表）、
//!           IR→字节码 编码器 和 字节码→IR 解码器（用于测试 round-trip）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - crate::ir::{IrInstruction, PositionEncoding, ChoiceData, NONE_REG, NONE_POOL}
//! - serde（CompiledScene 序列化）
//! - bincode（二进制序列化格式）
//!
//! ## 字节码格式
//!
//! 每条指令 = 定长操作码 (1 byte u8) + 变长操作数 (各占 1/2/8 bytes)。
//! 汇编级指令长度见各 Opcode 的 `size()` 方法。

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::{ChoiceData, IrInstruction, NONE_POOL, NONE_REG, PositionEncoding};

// ============================================================================
// Opcode — 操作码枚举
// ============================================================================

/// 字节码操作码 — 1 byte 编码，对应 VM 执行的每一条指令类型。
///
/// 操作码布局遵循 Architecture.md §4.4 的字节码指令集规范：
/// - `0x01-0x07`：数据传送
/// - `0x08-0x15`：算术/比较/逻辑
/// - `0x16-0x17`：一元运算
/// - `0x20-0x29`：渲染与交互
/// - `0x30-0x35`：控制流
/// - `0x40-0x43`：变量/旗标
/// - `0x50-0x54`：媒体
/// - `0x60-0x61`：时序/跨场景
/// - `0xFF`：结束
///
/// 每条指令的字节长度由操作数决定，通过 `size()` 方法获取。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Opcode {
    // ── 数据传送 (0x01-0x07) ──
    /// PushStr — 字符串常量 → 寄存器 (4 bytes)
    PushStr = 0x01,
    /// PushInt — 整型立即数 → 寄存器 (10 bytes)
    PushInt = 0x02,
    /// PushFloat — 浮点立即数 → 寄存器 (10 bytes)
    PushFloat = 0x03,
    /// PushBool — 布尔立即数 → 寄存器 (3 bytes)
    PushBool = 0x04,
    /// LoadVar — 变量值 → 寄存器 (4 bytes)
    LoadVar = 0x05,
    /// StoreVar — 寄存器值 → 变量 (4 bytes)
    StoreVar = 0x06,
    /// CheckFlag — 旗标状态 → 寄存器 (4 bytes)
    CheckFlag = 0x07,

    // ── 算术运算 (0x08-0x0B) ──
    Add = 0x08,
    Sub = 0x09,
    Mul = 0x0A,
    Div = 0x0B,

    // ── 比较运算 (0x0C-0x11) ──
    Eq = 0x0C,
    Neq = 0x0D,
    Lt = 0x0E,
    Gt = 0x0F,
    Le = 0x10,
    Ge = 0x11,

    // ── 逻辑运算 (0x12-0x13) ──
    And = 0x12,
    Or = 0x13,

    // ── 一元运算 (0x14-0x15) ──
    Not = 0x14,
    Neg = 0x15,

    // ── 渲染指令 (0x20-0x28) ──
    Bg = 0x20,
    ShowChar = 0x21,
    ShowSprite = 0x22,
    MoveChar = 0x23,
    Emotion = 0x24,
    HideChar = 0x25,
    HideSprite = 0x26,
    Dialogue = 0x27,
    Narrate = 0x28,

    // ── 交互指令 (0x29) ──
    Menu = 0x29,

    // ── 控制流 (0x30-0x35) ──
    Jump = 0x30,
    JumpIf = 0x31,
    JumpIfFlag = 0x32,
    Call = 0x33,
    Return = 0x34,
    /// Label — 伪指令，仅用于标记位置，不产生字节码
    Label = 0x35,
    /// Goto — 跨场景跳转
    Goto = 0x36,

    // ── 变量/旗标 (0x40-0x43) ──
    SetVar = 0x40,
    SetFlag = 0x41,
    UnsetFlag = 0x42,
    ToggleFlag = 0x43,

    // ── 媒体 (0x50-0x54) ──
    PlayBgm = 0x50,
    StopBgm = 0x51,
    PlaySe = 0x52,
    PlayVoice = 0x53,
    Effect = 0x54,

    // ── 时序/跨场景 (0x60-0x61) ──
    Wait = 0x60,

    // ── 特殊 (0xFF) ──
    End = 0xFF,
}

impl Opcode {
    /// 从 u8 字节解析操作码。
    ///
    /// # 参数
    /// - `byte`：操作码字节值
    ///
    /// # 返回值
    /// - `Some(Opcode)`：合法的操作码
    /// - `None`：未定义的操作码（字节码版本不匹配或数据损坏）
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(Opcode::PushStr),
            0x02 => Some(Opcode::PushInt),
            0x03 => Some(Opcode::PushFloat),
            0x04 => Some(Opcode::PushBool),
            0x05 => Some(Opcode::LoadVar),
            0x06 => Some(Opcode::StoreVar),
            0x07 => Some(Opcode::CheckFlag),
            0x08 => Some(Opcode::Add),
            0x09 => Some(Opcode::Sub),
            0x0A => Some(Opcode::Mul),
            0x0B => Some(Opcode::Div),
            0x0C => Some(Opcode::Eq),
            0x0D => Some(Opcode::Neq),
            0x0E => Some(Opcode::Lt),
            0x0F => Some(Opcode::Gt),
            0x10 => Some(Opcode::Le),
            0x11 => Some(Opcode::Ge),
            0x12 => Some(Opcode::And),
            0x13 => Some(Opcode::Or),
            0x14 => Some(Opcode::Not),
            0x15 => Some(Opcode::Neg),
            0x20 => Some(Opcode::Bg),
            0x21 => Some(Opcode::ShowChar),
            0x22 => Some(Opcode::ShowSprite),
            0x23 => Some(Opcode::MoveChar),
            0x24 => Some(Opcode::Emotion),
            0x25 => Some(Opcode::HideChar),
            0x26 => Some(Opcode::HideSprite),
            0x27 => Some(Opcode::Dialogue),
            0x28 => Some(Opcode::Narrate),
            0x29 => Some(Opcode::Menu),
            0x30 => Some(Opcode::Jump),
            0x31 => Some(Opcode::JumpIf),
            0x32 => Some(Opcode::JumpIfFlag),
            0x33 => Some(Opcode::Call),
            0x34 => Some(Opcode::Return),
            0x35 => Some(Opcode::Label),
            0x36 => Some(Opcode::Goto),
            0x40 => Some(Opcode::SetVar),
            0x41 => Some(Opcode::SetFlag),
            0x42 => Some(Opcode::UnsetFlag),
            0x43 => Some(Opcode::ToggleFlag),
            0x50 => Some(Opcode::PlayBgm),
            0x51 => Some(Opcode::StopBgm),
            0x52 => Some(Opcode::PlaySe),
            0x53 => Some(Opcode::PlayVoice),
            0x54 => Some(Opcode::Effect),
            0x60 => Some(Opcode::Wait),
            0xFF => Some(Opcode::End),
            _ => None,
        }
    }
}

impl TryFrom<u8> for Opcode {
    type Error = String;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        Opcode::from_byte(byte).ok_or_else(|| format!("未定义的操作码: 0x{:02X}", byte))
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Opcode::PushStr => "PUSH_STR",
            Opcode::PushInt => "PUSH_INT",
            Opcode::PushFloat => "PUSH_FLOAT",
            Opcode::PushBool => "PUSH_BOOL",
            Opcode::LoadVar => "LOAD_VAR",
            Opcode::StoreVar => "STORE_VAR",
            Opcode::CheckFlag => "CHECK_FLAG",
            Opcode::Add => "ADD",
            Opcode::Sub => "SUB",
            Opcode::Mul => "MUL",
            Opcode::Div => "DIV",
            Opcode::Eq => "EQ",
            Opcode::Neq => "NEQ",
            Opcode::Lt => "LT",
            Opcode::Gt => "GT",
            Opcode::Le => "LE",
            Opcode::Ge => "GE",
            Opcode::And => "AND",
            Opcode::Or => "OR",
            Opcode::Not => "NOT",
            Opcode::Neg => "NEG",
            Opcode::Bg => "BG",
            Opcode::ShowChar => "SHOW_CHAR",
            Opcode::ShowSprite => "SHOW_SPRITE",
            Opcode::MoveChar => "MOVE_CHAR",
            Opcode::Emotion => "EMOTION",
            Opcode::HideChar => "HIDE_CHAR",
            Opcode::HideSprite => "HIDE_SPRITE",
            Opcode::Dialogue => "DIALOGUE",
            Opcode::Narrate => "NARRATE",
            Opcode::Menu => "MENU",
            Opcode::Jump => "JUMP",
            Opcode::JumpIf => "JUMP_IF",
            Opcode::JumpIfFlag => "JUMP_IF_FLAG",
            Opcode::Call => "CALL",
            Opcode::Return => "RETURN",
            Opcode::Label => "LABEL",
            Opcode::Goto => "GOTO",
            Opcode::SetVar => "SET_VAR",
            Opcode::SetFlag => "SET_FLAG",
            Opcode::UnsetFlag => "UNSET_FLAG",
            Opcode::ToggleFlag => "TOGGLE_FLAG",
            Opcode::PlayBgm => "PLAY_BGM",
            Opcode::StopBgm => "STOP_BGM",
            Opcode::PlaySe => "PLAY_SE",
            Opcode::PlayVoice => "PLAY_VOICE",
            Opcode::Effect => "EFFECT",
            Opcode::Wait => "WAIT",
            Opcode::End => "END",
        };
        write!(f, "{}", name)
    }
}

// ============================================================================
// 字节码解码辅助 — VM 侧公共 API（aster-vm::opcode 由此 re-export）
// ============================================================================

/// 返回指定操作码的字节码指令总长度（含 1 byte opcode + 变长操作数）。
///
/// Menu 和 Effect 为变长指令，此处返回 0（调用方需自行计算）。
/// Label 是伪指令，不产生字节码，返回 0。
pub fn instruction_size(opcode: Opcode) -> usize {
    match opcode {
        Opcode::PushStr => 4,
        Opcode::PushInt => 10,
        Opcode::PushFloat => 10,
        Opcode::PushBool => 3,
        Opcode::LoadVar => 4,
        Opcode::StoreVar => 4,
        Opcode::CheckFlag => 4,
        Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div => 4,
        Opcode::Eq | Opcode::Neq | Opcode::Lt | Opcode::Gt | Opcode::Le | Opcode::Ge => 4,
        Opcode::And | Opcode::Or => 4,
        Opcode::Not | Opcode::Neg => 3,
        Opcode::Bg => 6,
        Opcode::ShowChar => 11,
        Opcode::ShowSprite => 10,
        Opcode::MoveChar => 11,
        Opcode::Emotion => 8,
        Opcode::HideChar => 6,
        Opcode::HideSprite => 6,
        Opcode::Dialogue => 7,
        Opcode::Narrate => 3,
        Opcode::Menu => 0,
        Opcode::Jump => 3,
        Opcode::JumpIf => 4,
        Opcode::JumpIfFlag => 5,
        Opcode::Call => 0, // 变长: op(1) + target(2) + arg_count(1) + args(N)
        Opcode::Return => 1,
        Opcode::Label => 0,
        Opcode::Goto => 5,
        Opcode::SetVar => 4,
        Opcode::SetFlag | Opcode::UnsetFlag | Opcode::ToggleFlag => 3,
        Opcode::PlayBgm => 5,
        Opcode::StopBgm => 2,
        Opcode::PlaySe => 4,
        Opcode::PlayVoice => 3,
        Opcode::Effect => 0,
        Opcode::Wait => 2,
        Opcode::End => 1,
    }
}

/// Menu 指令字节长度。
pub fn menu_size(choice_count: usize) -> usize {
    4 + choice_count * 6
}

/// Call 指令字节长度。
/// 格式：op(1) + target_offset(2) + arg_count(1) + args(arg_count)
pub fn call_size(arg_count: usize) -> usize {
    4 + arg_count
}

/// Effect 指令字节长度。
pub fn effect_size(param_count: usize) -> usize {
    4 + param_count * 4
}

/// 从字节数组读取 little-endian u16（非推进式索引）。
#[inline]
pub fn read_u16(bytes: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([bytes[pos], bytes[pos + 1]])
}

/// 从字节数组读取 little-endian i64（非推进式索引）。
#[inline]
pub fn read_i64(bytes: &[u8], pos: usize) -> i64 {
    i64::from_le_bytes([
        bytes[pos],
        bytes[pos + 1],
        bytes[pos + 2],
        bytes[pos + 3],
        bytes[pos + 4],
        bytes[pos + 5],
        bytes[pos + 6],
        bytes[pos + 7],
    ])
}

/// 从字节数组读取 little-endian f64（非推进式索引）。
#[inline]
pub fn read_f64(bytes: &[u8], pos: usize) -> f64 {
    f64::from_le_bytes([
        bytes[pos],
        bytes[pos + 1],
        bytes[pos + 2],
        bytes[pos + 3],
        bytes[pos + 4],
        bytes[pos + 5],
        bytes[pos + 6],
        bytes[pos + 7],
    ])
}

// ============================================================================
// CompiledScene — 编译产物
// ============================================================================

/// 编译后的场景 — 包含可被 VM 直接执行的字节码。
///
/// # 结构说明
///
/// - `version`：字节码格式版本（当前 = 1），VM 用于拒绝不兼容的字节码
/// - `instructions`：字节码指令序列（变长编码，PC 按指令推进）
/// - `constant_pool`：所有字符串字面量（对话、名称、资源路径等）
/// - `label_table`：标签名 → 字节偏移（用于跳转解析）
///
/// # 序列化
///
/// 使用 `bincode` 序列化/反序列化为 `.asterbyte` 文件，供 VM 或 GameLauncher 直接加载。
///
/// # 示例
/// ```
/// use aster_compiler::{CompiledScene, Opcode};
/// use std::collections::HashMap;
///
/// let scene = CompiledScene {
///     version: 1,
///     instructions: vec![0xFF], // [END]
///     constant_pool: vec![],
///     label_table: HashMap::new(),
/// };
///
/// let bytes = bincode::serialize(&scene).expect("序列化成功");
/// let restored: CompiledScene = bincode::deserialize(&bytes).expect("反序列化成功");
/// assert_eq!(restored.version, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledScene {
    /// 字节码格式版本号（当前 = 1）
    pub version: u32,

    /// 字节码指令序列（变长编码，不含 Label 伪指令）
    pub instructions: Vec<u8>,

    /// 字符串常量池（按索引引用，0xFFFF = NONE）
    pub constant_pool: Vec<String>,

    /// 标签名 → 字节偏移映射表（仅含用户定义的 Label，不含内部标签）
    pub label_table: HashMap<String, usize>,
}

// ============================================================================
// 字节码编码器
// ============================================================================

/// 字节码编码/解码中的 helper：将 u16 以 little-endian 写入字节数组。
fn write_u16(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

/// 从字节数组读取 little-endian u16（推进式索引，内部用）。
fn read_u16_advance(bytes: &[u8], pos: &mut usize) -> u16 {
    let v = read_u16(bytes, *pos);
    *pos += 2;
    v
}

/// 将 i64 以 little-endian 写入字节数组。
fn write_i64(buf: &mut Vec<u8>, value: i64) {
    buf.extend_from_slice(&value.to_le_bytes());
}

/// 从字节数组读取 little-endian i64（推进式索引，内部用）。
fn read_i64_advance(bytes: &[u8], pos: &mut usize) -> i64 {
    let v = read_i64(bytes, *pos);
    *pos += 8;
    v
}

/// 将 f64 以 little-endian 写入字节数组。
fn write_f64(buf: &mut Vec<u8>, value: f64) {
    buf.extend_from_slice(&value.to_le_bytes());
}

/// 从字节数组读取 little-endian f64（推进式索引，内部用）。
fn read_f64_advance(bytes: &[u8], pos: &mut usize) -> f64 {
    let v = read_f64(bytes, *pos);
    *pos += 8;
    v
}

/// 将 IR 指令序列编码为字节码字节数组。
///
/// 这是整个编译管线的最后一步。输入：
/// - `ir_instructions`：已解析所有标签引用的 IR 指令序列
/// - `constant_pool`：字符串常量池
/// - `label_table`：标签名 → 字节偏移映射（会被填充）
///
/// # 编码流程
///
/// 1. **第一遍**：计算每条 IR 指令编码后的字节大小，确定指令偏移
/// 2. **第二遍**：记录所有 Label 位置到 label_table
/// 3. **第三遍**：逐条编码 IR 指令，将标签名解析为字节偏移
///
/// # 参数
/// - `ir_instructions`：IR 指令序列
/// - `constant_pool`：字符串常量池
/// - `label_table_out`：输出参数 — 填充用户定义标签的 名字→字节偏移 映射
///
/// # 返回值
/// - 字节码字节数组（不含 CompileScene 外层结构）
pub fn encode_instructions(
    ir_instructions: &[IrInstruction],
    _constant_pool: &[String],
    label_table_out: &mut HashMap<String, usize>,
) -> Result<Vec<u8>, String> {
    // Pass 1: 计算每条指令的字节大小和偏移
    let mut offsets: Vec<usize> = Vec::with_capacity(ir_instructions.len());
    let mut current_offset: usize = 0;

    for inst in ir_instructions {
        offsets.push(current_offset);
        match inst {
            IrInstruction::Label { .. } => {
                // Label 是伪指令，不占字节（VM 不会执行到它）
            }
            IrInstruction::Call { args, .. } => {
                // op(1) + target_offset(2) + arg_count(1) + args
                current_offset += 4 + args.len();
            }
            IrInstruction::Menu { choices, .. } => {
                // 头部：op(1) + prompt(2) + choice_count(1)
                current_offset += 4;
                // 每个 choice：text_idx(2) + target_offset(2) + cond_flag_idx(2)
                current_offset += choices.len() * 6;
            }
            IrInstruction::Effect { params, .. } => {
                // 头部：op(1) + type(2) + param_count(1)
                current_offset += 4;
                // 每个 param：key_idx(2) + value_reg(2)
                current_offset += params.len() * 4;
            }
            _ => {
                current_offset += ir_instruction_size(inst);
            }
        }
    }

    // Pass 2: 记录 Label 位置
    let mut label_map: HashMap<String, usize> = HashMap::new();
    for (i, inst) in ir_instructions.iter().enumerate() {
        if let IrInstruction::Label { name } = inst {
            label_map.insert(name.clone(), offsets[i]);
        }
    }

    // Pass 3: 编码指令，解析标签引用为字节偏移
    let mut bytes: Vec<u8> = Vec::with_capacity(current_offset);
    for inst in ir_instructions {
        match inst {
            IrInstruction::Label { .. } => {
                // Label 不产生字节码
            }
            IrInstruction::Menu {
                prompt_idx,
                choices,
            } => {
                bytes.push(Opcode::Menu as u8);
                write_u16(&mut bytes, *prompt_idx);
                bytes.push(choices.len() as u8);
                for choice in choices {
                    write_u16(&mut bytes, choice.text_idx);
                    // 解析 target 标签名为字节偏移
                    let target_offset = label_map.get(&choice.target).copied().unwrap_or(0);
                    write_u16(&mut bytes, target_offset as u16);
                    write_u16(&mut bytes, choice.condition_flag_idx);
                }
            }
            IrInstruction::Effect { type_idx, params } => {
                bytes.push(Opcode::Effect as u8);
                write_u16(&mut bytes, *type_idx);
                bytes.push(params.len() as u8);
                for (key_idx, value_reg) in params {
                    write_u16(&mut bytes, *key_idx);
                    write_u16(&mut bytes, *value_reg);
                }
            }
            _ => {
                encode_instruction(inst, &label_map, &mut bytes);
            }
        }
    }

    // 输出 Label 表（只输出用户定义的标签，跳过 @ 前缀的内部标签）
    *label_table_out = label_map
        .into_iter()
        .filter(|(name, _)| !name.starts_with('@'))
        .collect();

    Ok(bytes)
}

/// 返回单条 IR 指令编码后的字节长度（可变长指令除外）。
///
/// 用于 Pass 1 的偏移计算。
fn ir_instruction_size(inst: &IrInstruction) -> usize {
    match inst {
        IrInstruction::PushStr { .. } => 4,
        IrInstruction::PushInt { .. } => 10,
        IrInstruction::PushFloat { .. } => 10,
        IrInstruction::PushBool { .. } => 3,
        IrInstruction::LoadVar { .. } => 4,
        IrInstruction::StoreVar { .. } => 4,
        IrInstruction::CheckFlag { .. } => 4,
        IrInstruction::Add { .. }
        | IrInstruction::Sub { .. }
        | IrInstruction::Mul { .. }
        | IrInstruction::Div { .. }
        | IrInstruction::Eq { .. }
        | IrInstruction::Neq { .. }
        | IrInstruction::Lt { .. }
        | IrInstruction::Gt { .. }
        | IrInstruction::Le { .. }
        | IrInstruction::Ge { .. }
        | IrInstruction::And { .. }
        | IrInstruction::Or { .. } => 4,
        IrInstruction::Not { .. } | IrInstruction::Neg { .. } => 3,
        IrInstruction::Bg { .. } => 6,
        IrInstruction::ShowChar { .. } => 11,
        IrInstruction::ShowSprite { .. } => 10,
        IrInstruction::MoveChar { .. } => 11,
        IrInstruction::Emotion { .. } => 8,
        IrInstruction::HideChar { .. } => 6,
        IrInstruction::HideSprite { .. } => 6,
        IrInstruction::Dialogue { .. } => 7,
        IrInstruction::Narrate { .. } => 3,
        IrInstruction::Jump { .. } => 3,
        IrInstruction::Call { .. } => 0, // 变长
        IrInstruction::JumpIf { .. } => 4,
        IrInstruction::JumpIfFlag { .. } => 5,
        IrInstruction::Return => 1,
        IrInstruction::SetVar { .. } => 4,
        IrInstruction::SetFlag { .. }
        | IrInstruction::UnsetFlag { .. }
        | IrInstruction::ToggleFlag { .. } => 3,
        IrInstruction::PlayBgm { .. } => 5,
        IrInstruction::StopBgm { .. } => 2,
        IrInstruction::PlaySe { .. } => 4,
        IrInstruction::PlayVoice { .. } => 3,
        IrInstruction::Wait { .. } => 2,
        IrInstruction::Goto { .. } => 5,
        IrInstruction::End => 1,
        // 可变长指令在外部处理
        IrInstruction::Label { .. } | IrInstruction::Menu { .. } | IrInstruction::Effect { .. } => {
            0
        }
    }
}

/// 将单条 IR 指令编码为字节追加到 `buf`。
///
/// 跳转目标标签在 `label_map` 中解析为字节偏移。
fn encode_instruction(inst: &IrInstruction, label_map: &HashMap<String, usize>, buf: &mut Vec<u8>) {
    match inst {
        IrInstruction::PushStr { reg, str_idx } => {
            buf.push(Opcode::PushStr as u8);
            buf.push(*reg);
            write_u16(buf, *str_idx);
        }
        IrInstruction::PushInt { reg, value } => {
            buf.push(Opcode::PushInt as u8);
            buf.push(*reg);
            write_i64(buf, *value);
        }
        IrInstruction::PushFloat { reg, value } => {
            buf.push(Opcode::PushFloat as u8);
            buf.push(*reg);
            write_f64(buf, *value);
        }
        IrInstruction::PushBool { reg, value } => {
            buf.push(Opcode::PushBool as u8);
            buf.push(*reg);
            buf.push(*value as u8);
        }
        IrInstruction::LoadVar { dst, name_idx } => {
            buf.push(Opcode::LoadVar as u8);
            buf.push(*dst);
            write_u16(buf, *name_idx);
        }
        IrInstruction::StoreVar { name_idx, src } => {
            buf.push(Opcode::StoreVar as u8);
            write_u16(buf, *name_idx);
            buf.push(*src);
        }
        IrInstruction::CheckFlag { dst, flag_idx } => {
            buf.push(Opcode::CheckFlag as u8);
            buf.push(*dst);
            write_u16(buf, *flag_idx);
        }
        IrInstruction::Add { dst, left, right } => {
            buf.push(Opcode::Add as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Sub { dst, left, right } => {
            buf.push(Opcode::Sub as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Mul { dst, left, right } => {
            buf.push(Opcode::Mul as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Div { dst, left, right } => {
            buf.push(Opcode::Div as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Eq { dst, left, right } => {
            buf.push(Opcode::Eq as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Neq { dst, left, right } => {
            buf.push(Opcode::Neq as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Lt { dst, left, right } => {
            buf.push(Opcode::Lt as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Gt { dst, left, right } => {
            buf.push(Opcode::Gt as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Le { dst, left, right } => {
            buf.push(Opcode::Le as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Ge { dst, left, right } => {
            buf.push(Opcode::Ge as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::And { dst, left, right } => {
            buf.push(Opcode::And as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Or { dst, left, right } => {
            buf.push(Opcode::Or as u8);
            buf.push(*dst);
            buf.push(*left);
            buf.push(*right);
        }
        IrInstruction::Not { dst, src } => {
            buf.push(Opcode::Not as u8);
            buf.push(*dst);
            buf.push(*src);
        }
        IrInstruction::Neg { dst, src } => {
            buf.push(Opcode::Neg as u8);
            buf.push(*dst);
            buf.push(*src);
        }
        IrInstruction::Bg {
            asset_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            buf.push(Opcode::Bg as u8);
            write_u16(buf, *asset_idx);
            write_u16(buf, *trans_kind_idx);
            buf.push(*dur_reg);
        }
        IrInstruction::ShowChar {
            char_idx,
            pos,
            emotion_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            buf.push(Opcode::ShowChar as u8);
            write_u16(buf, *char_idx);
            buf.push(pos.to_byte());
            // Custom position: encode x_reg and y_reg
            match pos {
                PositionEncoding::Custom { x_reg, y_reg } => {
                    buf.push(*x_reg);
                    buf.push(*y_reg);
                }
                _ => {
                    buf.push(NONE_REG);
                    buf.push(NONE_REG);
                }
            }
            write_u16(buf, *emotion_idx);
            write_u16(buf, *trans_kind_idx);
            buf.push(*dur_reg);
        }
        IrInstruction::ShowSprite {
            asset_idx,
            x_reg,
            y_reg,
            scale_reg,
            alpha_reg,
            trans_kind_idx,
            dur_reg,
        } => {
            buf.push(Opcode::ShowSprite as u8);
            write_u16(buf, *asset_idx);
            buf.push(*x_reg);
            buf.push(*y_reg);
            buf.push(*scale_reg);
            buf.push(*alpha_reg);
            write_u16(buf, *trans_kind_idx);
            buf.push(*dur_reg);
        }
        IrInstruction::MoveChar {
            char_idx,
            pos,
            emotion_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            buf.push(Opcode::MoveChar as u8);
            write_u16(buf, *char_idx);
            buf.push(pos.to_byte());
            match pos {
                PositionEncoding::Custom { x_reg, y_reg } => {
                    buf.push(*x_reg);
                    buf.push(*y_reg);
                }
                _ => {
                    buf.push(NONE_REG);
                    buf.push(NONE_REG);
                }
            }
            write_u16(buf, *emotion_idx);
            write_u16(buf, *trans_kind_idx);
            buf.push(*dur_reg);
        }
        IrInstruction::Emotion {
            char_idx,
            emotion_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            buf.push(Opcode::Emotion as u8);
            write_u16(buf, *char_idx);
            write_u16(buf, *emotion_idx);
            write_u16(buf, *trans_kind_idx);
            buf.push(*dur_reg);
        }
        IrInstruction::HideChar {
            char_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            buf.push(Opcode::HideChar as u8);
            write_u16(buf, *char_idx);
            write_u16(buf, *trans_kind_idx);
            buf.push(*dur_reg);
        }
        IrInstruction::HideSprite {
            asset_idx,
            trans_kind_idx,
            dur_reg,
        } => {
            buf.push(Opcode::HideSprite as u8);
            write_u16(buf, *asset_idx);
            write_u16(buf, *trans_kind_idx);
            buf.push(*dur_reg);
        }
        IrInstruction::Dialogue {
            speaker_idx,
            text_idx,
            voice_idx,
        } => {
            buf.push(Opcode::Dialogue as u8);
            write_u16(buf, *speaker_idx);
            write_u16(buf, *text_idx);
            write_u16(buf, *voice_idx);
        }
        IrInstruction::Narrate { text_idx } => {
            buf.push(Opcode::Narrate as u8);
            write_u16(buf, *text_idx);
        }
        IrInstruction::Jump { target } => {
            buf.push(Opcode::Jump as u8);
            let offset = label_map.get(target).copied().unwrap_or(0);
            write_u16(buf, offset as u16);
        }
        IrInstruction::JumpIf { reg, target } => {
            buf.push(Opcode::JumpIf as u8);
            buf.push(*reg);
            let offset = label_map.get(target).copied().unwrap_or(0);
            write_u16(buf, offset as u16);
        }
        IrInstruction::JumpIfFlag { flag_idx, target } => {
            buf.push(Opcode::JumpIfFlag as u8);
            write_u16(buf, *flag_idx);
            let offset = label_map.get(target).copied().unwrap_or(0);
            write_u16(buf, offset as u16);
        }
        IrInstruction::Call { target, args } => {
            buf.push(Opcode::Call as u8);
            let offset = label_map.get(target).copied().unwrap_or(0);
            write_u16(buf, offset as u16);
            buf.push(args.len() as u8);
            for reg in args {
                buf.push(*reg);
            }
        }
        IrInstruction::Return => {
            buf.push(Opcode::Return as u8);
        }
        IrInstruction::Goto {
            scene_idx,
            label_idx,
        } => {
            buf.push(Opcode::Goto as u8);
            write_u16(buf, *scene_idx);
            write_u16(buf, *label_idx);
        }
        IrInstruction::SetVar {
            name_idx,
            value_reg,
        } => {
            buf.push(Opcode::SetVar as u8);
            write_u16(buf, *name_idx);
            buf.push(*value_reg);
        }
        IrInstruction::SetFlag { flag_idx } => {
            buf.push(Opcode::SetFlag as u8);
            write_u16(buf, *flag_idx);
        }
        IrInstruction::UnsetFlag { flag_idx } => {
            buf.push(Opcode::UnsetFlag as u8);
            write_u16(buf, *flag_idx);
        }
        IrInstruction::ToggleFlag { flag_idx } => {
            buf.push(Opcode::ToggleFlag as u8);
            write_u16(buf, *flag_idx);
        }
        IrInstruction::PlayBgm {
            asset_idx,
            fade_reg,
            looping,
        } => {
            buf.push(Opcode::PlayBgm as u8);
            write_u16(buf, *asset_idx);
            buf.push(*fade_reg);
            buf.push(*looping as u8);
        }
        IrInstruction::StopBgm { fade_reg } => {
            buf.push(Opcode::StopBgm as u8);
            buf.push(*fade_reg);
        }
        IrInstruction::PlaySe {
            asset_idx,
            fade_reg,
        } => {
            buf.push(Opcode::PlaySe as u8);
            write_u16(buf, *asset_idx);
            buf.push(*fade_reg);
        }
        IrInstruction::PlayVoice { asset_idx } => {
            buf.push(Opcode::PlayVoice as u8);
            write_u16(buf, *asset_idx);
        }
        IrInstruction::Wait { dur_reg } => {
            buf.push(Opcode::Wait as u8);
            buf.push(*dur_reg);
        }
        IrInstruction::End => {
            buf.push(Opcode::End as u8);
        }
        // 以下在外部特殊处理
        IrInstruction::Label { .. } | IrInstruction::Menu { .. } | IrInstruction::Effect { .. } => {
            // handled in encode_instructions
        }
    }
}

// ============================================================================
// 字节码解码器（用于测试 round-trip）
// ============================================================================

/// 从字节码字节数组解码回 IR 指令序列。
///
/// 用于测试 round-trip 验证，生产环境中 VM 直接执行字节码无需解码为 IR。
///
/// # 参数
/// - `bytes`：字节码字节数组
/// - `constant_pool`：字符串常量池（用于恢复字符串引用）
///
/// # 返回值
/// - `Ok(Vec<IrInstruction>)`：成功解码的 IR 指令序列
/// - `Err(String)`：解码失败（如遇到非法操作码）
pub fn decode_instructions(
    bytes: &[u8],
    _constant_pool: &[String],
) -> Result<Vec<IrInstruction>, String> {
    let mut instructions: Vec<IrInstruction> = Vec::new();
    let mut pos: usize = 0;

    while pos < bytes.len() {
        let op_byte = bytes[pos];
        let opcode = Opcode::try_from(op_byte)?;
        pos += 1;

        let inst = match opcode {
            Opcode::PushStr => {
                let reg = bytes[pos];
                pos += 1;
                let str_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::PushStr { reg, str_idx }
            }
            Opcode::PushInt => {
                let reg = bytes[pos];
                pos += 1;
                let value = read_i64_advance(bytes, &mut pos);
                IrInstruction::PushInt { reg, value }
            }
            Opcode::PushFloat => {
                let reg = bytes[pos];
                pos += 1;
                let value = read_f64_advance(bytes, &mut pos);
                IrInstruction::PushFloat { reg, value }
            }
            Opcode::PushBool => {
                let reg = bytes[pos];
                pos += 1;
                let value = bytes[pos] != 0;
                pos += 1;
                IrInstruction::PushBool { reg, value }
            }
            Opcode::LoadVar => {
                let dst = bytes[pos];
                pos += 1;
                let name_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::LoadVar { dst, name_idx }
            }
            Opcode::StoreVar => {
                let name_idx = read_u16_advance(bytes, &mut pos);
                let src = bytes[pos];
                pos += 1;
                IrInstruction::StoreVar { name_idx, src }
            }
            Opcode::CheckFlag => {
                let dst = bytes[pos];
                pos += 1;
                let flag_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::CheckFlag { dst, flag_idx }
            }
            Opcode::Add => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Add { dst, left, right }
            }
            Opcode::Sub => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Sub { dst, left, right }
            }
            Opcode::Mul => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Mul { dst, left, right }
            }
            Opcode::Div => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Div { dst, left, right }
            }
            Opcode::Eq => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Eq { dst, left, right }
            }
            Opcode::Neq => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Neq { dst, left, right }
            }
            Opcode::Lt => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Lt { dst, left, right }
            }
            Opcode::Gt => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Gt { dst, left, right }
            }
            Opcode::Le => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Le { dst, left, right }
            }
            Opcode::Ge => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Ge { dst, left, right }
            }
            Opcode::And => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::And { dst, left, right }
            }
            Opcode::Or => {
                let dst = bytes[pos];
                pos += 1;
                let left = bytes[pos];
                pos += 1;
                let right = bytes[pos];
                pos += 1;
                IrInstruction::Or { dst, left, right }
            }
            Opcode::Not => {
                let dst = bytes[pos];
                pos += 1;
                let src = bytes[pos];
                pos += 1;
                IrInstruction::Not { dst, src }
            }
            Opcode::Neg => {
                let dst = bytes[pos];
                pos += 1;
                let src = bytes[pos];
                pos += 1;
                IrInstruction::Neg { dst, src }
            }
            Opcode::Bg => {
                let asset_idx = read_u16_advance(bytes, &mut pos);
                let trans_kind_idx = read_u16_advance(bytes, &mut pos);
                let dur_reg = bytes[pos];
                pos += 1;
                IrInstruction::Bg {
                    asset_idx,
                    trans_kind_idx,
                    dur_reg,
                }
            }
            Opcode::ShowChar => {
                let char_idx = read_u16_advance(bytes, &mut pos);
                let pos_byte = bytes[pos];
                pos += 1;
                let x_reg = bytes[pos];
                pos += 1;
                let y_reg = bytes[pos];
                pos += 1;
                let position = match pos_byte {
                    0x00 => PositionEncoding::Left,
                    0x01 => PositionEncoding::Center,
                    0x02 => PositionEncoding::Right,
                    0x03 => PositionEncoding::Custom { x_reg, y_reg },
                    _ => return Err(format!("非法的 Position 编码: 0x{:02X}", pos_byte)),
                };
                let emotion_idx = read_u16_advance(bytes, &mut pos);
                let trans_kind_idx = read_u16_advance(bytes, &mut pos);
                let dur_reg = bytes[pos];
                pos += 1;
                IrInstruction::ShowChar {
                    char_idx,
                    pos: position,
                    emotion_idx,
                    trans_kind_idx,
                    dur_reg,
                }
            }
            Opcode::ShowSprite => {
                let asset_idx = read_u16_advance(bytes, &mut pos);
                let x_reg = bytes[pos];
                pos += 1;
                let y_reg = bytes[pos];
                pos += 1;
                let scale_reg = bytes[pos];
                pos += 1;
                let alpha_reg = bytes[pos];
                pos += 1;
                let trans_kind_idx = read_u16_advance(bytes, &mut pos);
                let dur_reg = bytes[pos];
                pos += 1;
                IrInstruction::ShowSprite {
                    asset_idx,
                    x_reg,
                    y_reg,
                    scale_reg,
                    alpha_reg,
                    trans_kind_idx,
                    dur_reg,
                }
            }
            Opcode::MoveChar => {
                let char_idx = read_u16_advance(bytes, &mut pos);
                let pos_byte = bytes[pos];
                pos += 1;
                let x_reg = bytes[pos];
                pos += 1;
                let y_reg = bytes[pos];
                pos += 1;
                let position = match pos_byte {
                    0x00 => PositionEncoding::Left,
                    0x01 => PositionEncoding::Center,
                    0x02 => PositionEncoding::Right,
                    0x03 => PositionEncoding::Custom { x_reg, y_reg },
                    _ => return Err(format!("非法的 Position 编码: 0x{:02X}", pos_byte)),
                };
                let emotion_idx = read_u16_advance(bytes, &mut pos);
                let trans_kind_idx = read_u16_advance(bytes, &mut pos);
                let dur_reg = bytes[pos];
                pos += 1;
                IrInstruction::MoveChar {
                    char_idx,
                    pos: position,
                    emotion_idx,
                    trans_kind_idx,
                    dur_reg,
                }
            }
            Opcode::Emotion => {
                let char_idx = read_u16_advance(bytes, &mut pos);
                let emotion_idx = read_u16_advance(bytes, &mut pos);
                let trans_kind_idx = read_u16_advance(bytes, &mut pos);
                let dur_reg = bytes[pos];
                pos += 1;
                IrInstruction::Emotion {
                    char_idx,
                    emotion_idx,
                    trans_kind_idx,
                    dur_reg,
                }
            }
            Opcode::HideChar => {
                let char_idx = read_u16_advance(bytes, &mut pos);
                let trans_kind_idx = read_u16_advance(bytes, &mut pos);
                let dur_reg = bytes[pos];
                pos += 1;
                IrInstruction::HideChar {
                    char_idx,
                    trans_kind_idx,
                    dur_reg,
                }
            }
            Opcode::HideSprite => {
                let asset_idx = read_u16_advance(bytes, &mut pos);
                let trans_kind_idx = read_u16_advance(bytes, &mut pos);
                let dur_reg = bytes[pos];
                pos += 1;
                IrInstruction::HideSprite {
                    asset_idx,
                    trans_kind_idx,
                    dur_reg,
                }
            }
            Opcode::Dialogue => {
                let speaker_idx = read_u16_advance(bytes, &mut pos);
                let text_idx = read_u16_advance(bytes, &mut pos);
                let voice_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::Dialogue {
                    speaker_idx,
                    text_idx,
                    voice_idx,
                }
            }
            Opcode::Narrate => {
                let text_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::Narrate { text_idx }
            }
            Opcode::Menu => {
                let prompt_idx = read_u16_advance(bytes, &mut pos);
                let choice_count = bytes[pos] as usize;
                pos += 1;
                let mut choices = Vec::with_capacity(choice_count);
                for _ in 0..choice_count {
                    let text_idx = read_u16_advance(bytes, &mut pos);
                    let _target_offset = read_u16_advance(bytes, &mut pos);
                    let _cond_flag_idx = read_u16_advance(bytes, &mut pos);
                    choices.push(ChoiceData {
                        text_idx,
                        target: String::new(), // 解码时无法恢复标签名
                        condition_flag_idx: NONE_POOL,
                    });
                }
                IrInstruction::Menu {
                    prompt_idx,
                    choices,
                }
            }
            Opcode::Jump => {
                let _offset = read_u16_advance(bytes, &mut pos);
                IrInstruction::Jump {
                    target: String::new(),
                }
            }
            Opcode::JumpIf => {
                let reg = bytes[pos];
                pos += 1;
                let _offset = read_u16_advance(bytes, &mut pos);
                IrInstruction::JumpIf {
                    reg,
                    target: String::new(),
                }
            }
            Opcode::JumpIfFlag => {
                let flag_idx = read_u16_advance(bytes, &mut pos);
                let _offset = read_u16_advance(bytes, &mut pos);
                IrInstruction::JumpIfFlag {
                    flag_idx,
                    target: String::new(),
                }
            }
            Opcode::Call => {
                let _offset = read_u16_advance(bytes, &mut pos);
                let arg_count = bytes[pos] as usize;
                pos += 1;
                let mut args = Vec::with_capacity(arg_count);
                for _ in 0..arg_count {
                    args.push(bytes[pos]);
                    pos += 1;
                }
                IrInstruction::Call {
                    target: String::new(),
                    args,
                }
            }
            Opcode::Return => IrInstruction::Return,
            Opcode::Label => {
                // Label 不产生字节码，正常情况下不应出现在 instruction stream 中
                IrInstruction::Return // fallback
            }
            Opcode::Goto => {
                let scene_idx = read_u16_advance(bytes, &mut pos);
                let label_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::Goto {
                    scene_idx,
                    label_idx,
                }
            }
            Opcode::SetVar => {
                let name_idx = read_u16_advance(bytes, &mut pos);
                let value_reg = bytes[pos];
                pos += 1;
                IrInstruction::SetVar {
                    name_idx,
                    value_reg,
                }
            }
            Opcode::SetFlag => {
                let flag_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::SetFlag { flag_idx }
            }
            Opcode::UnsetFlag => {
                let flag_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::UnsetFlag { flag_idx }
            }
            Opcode::ToggleFlag => {
                let flag_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::ToggleFlag { flag_idx }
            }
            Opcode::PlayBgm => {
                let asset_idx = read_u16_advance(bytes, &mut pos);
                let fade_reg = bytes[pos];
                pos += 1;
                let looping = bytes[pos] != 0;
                pos += 1;
                IrInstruction::PlayBgm {
                    asset_idx,
                    fade_reg,
                    looping,
                }
            }
            Opcode::StopBgm => {
                let fade_reg = bytes[pos];
                pos += 1;
                IrInstruction::StopBgm { fade_reg }
            }
            Opcode::PlaySe => {
                let asset_idx = read_u16_advance(bytes, &mut pos);
                let fade_reg = bytes[pos];
                pos += 1;
                IrInstruction::PlaySe {
                    asset_idx,
                    fade_reg,
                }
            }
            Opcode::PlayVoice => {
                let asset_idx = read_u16_advance(bytes, &mut pos);
                IrInstruction::PlayVoice { asset_idx }
            }
            Opcode::Effect => {
                let type_idx = read_u16_advance(bytes, &mut pos);
                let param_count = bytes[pos] as usize;
                pos += 1;
                let mut params = Vec::with_capacity(param_count);
                for _ in 0..param_count {
                    let key_idx = read_u16_advance(bytes, &mut pos);
                    let value_reg = read_u16_advance(bytes, &mut pos);
                    params.push((key_idx, value_reg));
                }
                IrInstruction::Effect { type_idx, params }
            }
            Opcode::Wait => {
                let dur_reg = bytes[pos];
                pos += 1;
                IrInstruction::Wait { dur_reg }
            }
            Opcode::End => IrInstruction::End,
        };

        instructions.push(inst);
    }

    Ok(instructions)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证所有 Opcode 的 from_byte / try_from 双向一致性。
    #[test]
    fn opcode_from_byte_roundtrip() {
        let all_opcodes = [
            Opcode::PushStr,
            Opcode::PushInt,
            Opcode::PushFloat,
            Opcode::PushBool,
            Opcode::LoadVar,
            Opcode::StoreVar,
            Opcode::CheckFlag,
            Opcode::Add,
            Opcode::Sub,
            Opcode::Mul,
            Opcode::Div,
            Opcode::Eq,
            Opcode::Neq,
            Opcode::Lt,
            Opcode::Gt,
            Opcode::Le,
            Opcode::Ge,
            Opcode::And,
            Opcode::Or,
            Opcode::Not,
            Opcode::Neg,
            Opcode::Bg,
            Opcode::ShowChar,
            Opcode::ShowSprite,
            Opcode::MoveChar,
            Opcode::Emotion,
            Opcode::HideChar,
            Opcode::HideSprite,
            Opcode::Dialogue,
            Opcode::Narrate,
            Opcode::Menu,
            Opcode::Jump,
            Opcode::JumpIf,
            Opcode::JumpIfFlag,
            Opcode::Call,
            Opcode::Return,
            Opcode::Label,
            Opcode::Goto,
            Opcode::SetVar,
            Opcode::SetFlag,
            Opcode::UnsetFlag,
            Opcode::ToggleFlag,
            Opcode::PlayBgm,
            Opcode::StopBgm,
            Opcode::PlaySe,
            Opcode::PlayVoice,
            Opcode::Effect,
            Opcode::Wait,
            Opcode::End,
        ];

        for op in &all_opcodes {
            let byte = *op as u8;
            let restored = Opcode::try_from(byte).expect("合法操作码");
            assert_eq!(restored, *op, "{op} 的 from_byte round-trip 失败");
        }

        // 测试非法操作码
        assert!(Opcode::try_from(0x00).is_err());
        assert!(Opcode::try_from(0xFE).is_err());
    }

    /// AC04 — CompiledScene bincode 序列化 round-trip。
    #[test]
    fn ac04_compiled_scene_bincode_roundtrip() {
        let mut label_table = HashMap::new();
        label_table.insert("start".into(), 0);
        label_table.insert("end".into(), 15);

        let scene = CompiledScene {
            version: 1,
            instructions: vec![
                0x01, 0x00, 0x00, 0x00, // PUSH_STR r0, pool[0]
                0xFF, // END
            ],
            constant_pool: vec!["hello".into(), "world".into()],
            label_table,
        };

        // 序列化
        let bytes = bincode::serialize(&scene).expect("bincode 序列化失败");
        assert!(!bytes.is_empty(), "序列化结果不应为空");

        // 反序列化
        let restored: CompiledScene = bincode::deserialize(&bytes).expect("bincode 反序列化失败");

        assert_eq!(restored.version, scene.version);
        assert_eq!(restored.instructions, scene.instructions);
        assert_eq!(restored.constant_pool, scene.constant_pool);
        assert_eq!(restored.label_table.len(), scene.label_table.len());
        assert_eq!(restored.label_table.get("start"), Some(&0usize));
        assert_eq!(restored.label_table.get("end"), Some(&15usize));
    }

    /// 验证空 CompiledScene 的序列化。
    #[test]
    fn empty_compiled_scene_roundtrip() {
        let scene = CompiledScene {
            version: 1,
            instructions: vec![],
            constant_pool: vec![],
            label_table: HashMap::new(),
        };

        let bytes = bincode::serialize(&scene).expect("序列化失败");
        let restored: CompiledScene = bincode::deserialize(&bytes).expect("反序列化失败");

        assert_eq!(restored.version, 1);
        assert!(restored.instructions.is_empty());
        assert!(restored.constant_pool.is_empty());
    }

    /// 验证 Opcode Display 实现。
    #[test]
    fn opcode_display() {
        assert_eq!(Opcode::PushStr.to_string(), "PUSH_STR");
        assert_eq!(Opcode::End.to_string(), "END");
        assert_eq!(Opcode::Dialogue.to_string(), "DIALOGUE");
    }
}
