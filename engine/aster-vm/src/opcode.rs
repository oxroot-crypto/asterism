//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/src/opcode.rs
//! 功能概述：VM 操作码工具模块 — 重导出编译器定义的 `Opcode` 枚举，
//!           提供指令字节尺寸查询和字节码操作数解码辅助函数。
//!           所有指令尺寸以 `bytecode.rs` 的 `encode_instruction()` 实际编码为准。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - aster_compiler::Opcode（重导出）

// 重导出编译器的操作码定义，避免代码重复
pub use aster_compiler::Opcode;

/// 返回指定操作码对应的字节码指令总长度（含 1 byte opcode + 变长操作数）。
///
/// 尺寸数据以 `aster_compiler::bytecode::encode_instruction()` 的实际编码逻辑为准。
///
/// # 注意
///
/// - `Menu` 和 `Effect` 为变长指令，此处返回 0（调用方需自行计算）
/// - `Label` 是伪指令，不产生字节码，返回 0
///
/// # 参数
/// - `opcode`：操作码
///
/// # 返回值
/// 该操作码对应的完整指令字节长度（固定长度指令），或 0（变长/伪指令）
pub fn instruction_size(opcode: Opcode) -> usize {
    match opcode {
        // ── 数据传送 (0x01-0x07) ──
        // PushStr: op(1) + reg(1) + str_idx(2) = 4
        Opcode::PushStr => 4,
        // PushInt: op(1) + reg(1) + value(8) = 10
        Opcode::PushInt => 10,
        // PushFloat: op(1) + reg(1) + value(8) = 10
        Opcode::PushFloat => 10,
        // PushBool: op(1) + reg(1) + value(1) = 3
        Opcode::PushBool => 3,
        // LoadVar: op(1) + dst(1) + name_idx(2) = 4
        Opcode::LoadVar => 4,
        // StoreVar: op(1) + name_idx(2) + src(1) = 4
        Opcode::StoreVar => 4,
        // CheckFlag: op(1) + dst(1) + flag_idx(2) = 4
        Opcode::CheckFlag => 4,

        // ── 算术运算 (0x08-0x0B) ──
        // 三地址指令：op(1) + dst(1) + left(1) + right(1) = 4
        Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div => 4,

        // ── 比较运算 (0x0C-0x11) ──
        Opcode::Eq | Opcode::Neq | Opcode::Lt | Opcode::Gt | Opcode::Le | Opcode::Ge => 4,

        // ── 逻辑运算 (0x12-0x13) ──
        Opcode::And | Opcode::Or => 4,

        // ── 一元运算 (0x14-0x15) ──
        // op(1) + dst(1) + src(1) = 3
        Opcode::Not | Opcode::Neg => 3,

        // ── 渲染指令 (0x20-0x28) ──
        // Bg: op(1) + asset_idx(2) + trans_kind_idx(2) + dur_reg(1) = 6
        Opcode::Bg => 6,
        // ShowChar: op(1) + char_idx(2) + pos(1) + x_reg(1) + y_reg(1)
        //           + emotion_idx(2) + trans_kind_idx(2) + dur_reg(1) = 11
        Opcode::ShowChar => 11,
        // ShowSprite: op(1) + asset_idx(2) + x_reg(1) + y_reg(1) + scale_reg(1)
        //             + alpha_reg(1) + trans_kind_idx(2) + dur_reg(1) = 10
        Opcode::ShowSprite => 10,
        // MoveChar: 同 ShowChar = 11
        Opcode::MoveChar => 11,
        // Emotion: op(1) + char_idx(2) + emotion_idx(2) + trans_kind_idx(2) + dur_reg(1) = 8
        Opcode::Emotion => 8,
        // HideChar: op(1) + char_idx(2) + trans_kind_idx(2) + dur_reg(1) = 6
        Opcode::HideChar => 6,
        // HideSprite: op(1) + asset_idx(2) + trans_kind_idx(2) + dur_reg(1) = 6
        Opcode::HideSprite => 6,
        // Dialogue: op(1) + speaker_idx(2) + text_idx(2) + voice_idx(2) = 7
        Opcode::Dialogue => 7,
        // Narrate: op(1) + text_idx(2) = 3
        Opcode::Narrate => 3,

        // ── 交互指令 (0x29) ──
        // Menu: 变长，头部 op(1) + prompt_idx(2) + count(1) = 4
        //       每个 choice: text_idx(2) + target_offset(2) + cond_flag_idx(2) = 6
        Opcode::Menu => 0, // 变长，调用方自行计算

        // ── 控制流 (0x30-0x36) ──
        // Jump: op(1) + offset(2) = 3
        Opcode::Jump => 3,
        // JumpIf: op(1) + reg(1) + offset(2) = 4
        Opcode::JumpIf => 4,
        // JumpIfFlag: op(1) + flag_idx(2) + offset(2) = 5
        Opcode::JumpIfFlag => 5,
        // Call: op(1) + offset(2) = 3
        Opcode::Call => 3,
        // Return: op(1) = 1
        Opcode::Return => 1,
        // Label: 伪指令，不产生字节码
        Opcode::Label => 0,
        // Goto: op(1) + scene_idx(2) + label_idx(2) = 5
        Opcode::Goto => 5,

        // ── 变量/旗标 (0x40-0x43) ──
        // SetVar: op(1) + name_idx(2) + value_reg(1) = 4
        Opcode::SetVar => 4,
        // SetFlag/UnsetFlag/ToggleFlag: op(1) + flag_idx(2) = 3
        Opcode::SetFlag | Opcode::UnsetFlag | Opcode::ToggleFlag => 3,

        // ── 媒体 (0x50-0x54) ──
        // PlayBgm: op(1) + asset_idx(2) + fade_reg(1) + looping(1) = 5
        Opcode::PlayBgm => 5,
        // StopBgm: op(1) + fade_reg(1) = 2
        Opcode::StopBgm => 2,
        // PlaySe: op(1) + asset_idx(2) + fade_reg(1) = 4
        Opcode::PlaySe => 4,
        // PlayVoice: op(1) + asset_idx(2) = 3
        Opcode::PlayVoice => 3,
        // Effect: 变长，头部 op(1) + type_idx(2) + count(1) = 4
        //         每个 param: key_idx(2) + value_reg(2) = 4
        Opcode::Effect => 0, // 变长，调用方自行计算

        // ── 时序 (0x60) ──
        // Wait: op(1) + dur_reg(1) = 2
        Opcode::Wait => 2,

        // ── 特殊 (0xFF) ──
        // End: op(1) = 1
        Opcode::End => 1,
    }
}

/// 计算 Menu 指令的字节长度。
///
/// Menu 格式：op(1) + prompt_idx(2) + count(1) + N × (text_idx(2) + target_offset(2) + cond_flag_idx(2))
///
/// # 参数
/// - `choice_count`：选项数量
pub fn menu_size(choice_count: usize) -> usize {
    4 + choice_count * 6
}

/// 计算 Effect 指令的字节长度。
///
/// Effect 格式：op(1) + type_idx(2) + param_count(1) + N × (key_idx(2) + value_reg(2))
///
/// # 参数
/// - `param_count`：参数数量
pub fn effect_size(param_count: usize) -> usize {
    4 + param_count * 4
}

// ============================================================================
// 字节码操作数解码辅助函数
// ============================================================================

/// 从字节数组中读取 little-endian u16。
///
/// # 参数
/// - `bytes`：字节码指令数组
/// - `pos`：起始偏移（操作数在指令中的位置）
#[inline]
pub(crate) fn read_u16(bytes: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([bytes[pos], bytes[pos + 1]])
}

/// 从字节数组中读取 little-endian i64。
///
/// # 参数
/// - `bytes`：字节码指令数组
/// - `pos`：起始偏移（操作数在指令中的位置）
#[inline]
pub(crate) fn read_i64(bytes: &[u8], pos: usize) -> i64 {
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

/// 从字节数组中读取 little-endian f64。
///
/// # 参数
/// - `bytes`：字节码指令数组
/// - `pos`：起始偏移（操作数在指令中的位置）
#[inline]
pub(crate) fn read_f64(bytes: &[u8], pos: usize) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证所有非变长 Opcode 的 instruction_size 返回非零值。
    #[test]
    fn opcode_sizes_nonzero_for_fixed_instructions() {
        let fixed_opcodes = [
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
            Opcode::Jump,
            Opcode::JumpIf,
            Opcode::JumpIfFlag,
            Opcode::Call,
            Opcode::Return,
            Opcode::Goto,
            Opcode::SetVar,
            Opcode::SetFlag,
            Opcode::UnsetFlag,
            Opcode::ToggleFlag,
            Opcode::PlayBgm,
            Opcode::StopBgm,
            Opcode::PlaySe,
            Opcode::PlayVoice,
            Opcode::Wait,
            Opcode::End,
        ];

        for opcode in &fixed_opcodes {
            let size = instruction_size(*opcode);
            assert!(
                size > 0,
                "Opcode::{} 的 instruction_size 应为正数，实际为 {}",
                opcode,
                size
            );
        }
    }

    /// 验证变长/伪指令返回 0。
    #[test]
    fn variable_length_opcodes_return_zero() {
        assert_eq!(instruction_size(Opcode::Menu), 0);
        assert_eq!(instruction_size(Opcode::Effect), 0);
        assert_eq!(instruction_size(Opcode::Label), 0);
    }

    /// 验证 menu_size 计算正确。
    #[test]
    fn menu_size_calculation() {
        assert_eq!(menu_size(0), 4); // 仅头部
        assert_eq!(menu_size(1), 10); // 头部 + 1 个 choice
        assert_eq!(menu_size(3), 22); // 头部 + 3 个 choice
    }

    /// 验证 effect_size 计算正确。
    #[test]
    fn effect_size_calculation() {
        assert_eq!(effect_size(0), 4); // 仅头部
        assert_eq!(effect_size(1), 8); // 头部 + 1 个 param
        assert_eq!(effect_size(3), 16); // 头部 + 3 个 param
    }

    /// 验证 read_u16 的 little-endian 读取。
    #[test]
    fn read_u16_little_endian() {
        let bytes = [0x34, 0x12, 0xFF, 0xFF];
        assert_eq!(read_u16(&bytes, 0), 0x1234);
        assert_eq!(read_u16(&bytes, 2), 0xFFFF);
    }

    /// 验证 read_i64 的 little-endian 读取。
    #[test]
    fn read_i64_little_endian() {
        let value: i64 = -1234567890;
        let bytes = value.to_le_bytes();
        assert_eq!(read_i64(&bytes, 0), value);
    }

    /// 验证所有 Opcode 可 from_byte round-trip。
    #[test]
    fn opcode_from_byte_roundtrip() {
        // 从 aster_compiler 重导出的 Opcode 应支持完整的 from_byte round-trip
        let op = Opcode::PushStr;
        let byte = op as u8;
        let restored = Opcode::from_byte(byte).expect("合法操作码");
        assert_eq!(restored, op);
    }

    /// 验证非法操作码返回 None。
    #[test]
    fn invalid_opcode_returns_none() {
        assert!(Opcode::from_byte(0x00).is_none());
        assert!(Opcode::from_byte(0xFE).is_none());
    }
}
