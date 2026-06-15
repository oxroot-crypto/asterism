//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/src/opcode.rs
//! 功能概述：VM 操作码工具模块 — 从 `aster_compiler::bytecode` 重导出
//!           Opcode 枚举、指令尺寸查询和字节码操作数解码辅助函数。
//!           所有实现位于编译器 crate，此处仅为 VM 侧提供统一导入路径。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - aster_compiler::bytecode（所有实现）

// 从编译器重导出，消除指令尺寸表和 read 辅助函数的重复维护
pub use aster_compiler::Opcode;
pub use aster_compiler::bytecode::{
    effect_size, instruction_size, menu_size, read_f64, read_i64, read_u16,
};

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
        assert_eq!(instruction_size(Opcode::Call), 0);
        assert_eq!(instruction_size(Opcode::Label), 0);
    }

    /// 验证 menu_size 计算正确。
    #[test]
    fn menu_size_calculation() {
        assert_eq!(menu_size(0), 4);
        assert_eq!(menu_size(1), 10);
        assert_eq!(menu_size(3), 22);
    }

    /// 验证 effect_size 计算正确。
    #[test]
    fn effect_size_calculation() {
        assert_eq!(effect_size(0), 4);
        assert_eq!(effect_size(1), 8);
        assert_eq!(effect_size(3), 16);
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
