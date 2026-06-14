//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-vm/src/vm.rs
//! 功能概述：字节码虚拟机核心 — `Vm` 结构体管理 16 个通用寄存器、操作数栈、
//!           调用栈、变量存储和旗标集合。`step()` 方法实现基于 token-threaded
//!           dispatch 的字节码执行循环，逐条解码并执行指令。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - aster_compiler::{CompiledScene, Opcode}
//! - aster_core::{Value, VariableStore, FlagSet}
//! - crate::action::{VmAction, MenuChoiceData}
//! - crate::engine_command::EngineCommand
//! - crate::opcode（指令尺寸 + 解码辅助）
//!
//! ## 执行模型
//!
//! `step()` 方法执行单条指令并返回 `VmAction`。纯内部指令（数据传送、
//! 算术运算、无条件跳转）静默执行不返回；外部渲染/音频指令返回
//! `VmAction::Command`；交互等待点返回 `WaitForInput`/`ShowMenu`/`SceneEnd`。
//!
//! 通过 `step()` 循环调用推进执行：
//! ```text
//! while let action = vm.step(&scene) {
//!     match action {
//!         WaitForInput => { /* 等待用户点击后继续 */ }
//!         SceneEnd => break,
//!         Command(cmd) => { /* 处理命令后继续 */ }
//!         ...
//!     }
//! }
//! ```

use aster_compiler::{CompiledScene, Opcode};
use aster_core::{FlagSet, Value, VariableStore};

use crate::action::{MenuChoiceData, VmAction};
use crate::engine_command::EngineCommand;
use crate::opcode::{self, instruction_size};

/// 子例程调用帧 — 保存返回地址和被调用者保存寄存器。
///
/// 调用约定：
/// - **r0-r3** 为调用者保存寄存器（CALL 时压入 `saved_registers`，RETURN 时恢复）
/// - **r4-r15** 为被调用者保存寄存器（子例程有责任在修改前保存并在返回前恢复，
///   当前版本因 PH1-T14 才实现 Call/Return，暂无子例程调用场景）
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// 返回地址：子例程完成后应跳转到的字节偏移
    pub return_pc: usize,

    /// 调用时保存的 r0-r3 寄存器值（调用者保存）
    pub saved_registers: [Value; 4],
}

impl CallFrame {
    /// 创建一个新的调用帧。
    ///
    /// # 参数
    /// - `return_pc`：返回地址（CALL 指令的下一条指令偏移）
    /// - `r0_r3`：当前 r0-r3 的值
    pub fn new(return_pc: usize, r0: Value, r1: Value, r2: Value, r3: Value) -> Self {
        CallFrame {
            return_pc,
            saved_registers: [r0, r1, r2, r3],
        }
    }
}

/// 字节码虚拟机 — 执行编译后的场景脚本（`CompiledScene`）。
///
/// # 架构
///
/// VM 基于寄存器式架构，具备以下组件：
/// - **程序计数器（pc）**：指向当前执行的字节码指令偏移
/// - **16 个通用寄存器（r0-r15）**：存储 `Value` 类型的中间运算结果
/// - **操作数栈**：子例程参数传递和临时值存储
/// - **变量存储（VariableStore）**：全局变量，场景间保持
/// - **旗标集合（FlagSet）**：命名布尔值，场景间保持
/// - **调用栈**：子例程调用帧（PH1-T14 启用 Call/Return）
///
/// # 执行流程
///
/// ```text
/// Vm::new() → 初始化所有状态为零值
///   loop {
///     vm.step(&compiled_scene) → VmAction
///     // SceneManager 处理 VmAction
///   }
/// ```
///
/// # 示例
/// ```
/// use aster_vm::{Vm, VmAction};
/// use aster_compiler::CompiledScene;
/// use std::collections::HashMap;
///
/// let scene = CompiledScene {
///     version: 1,
///     instructions: vec![0xFF], // [END]
///     constant_pool: vec![],
///     label_table: HashMap::new(),
/// };
///
/// let mut vm = Vm::new();
/// let action = vm.step(&scene);
/// assert_eq!(action, VmAction::SceneEnd);
/// ```
#[derive(Debug, Clone)]
pub struct Vm {
    /// 程序计数器 — 指向 `CompiledScene.instructions` 中当前指令的起始偏移
    pc: usize,

    /// 通用寄存器 r0-r15
    /// - r0-r3：调用者保存（Call 时自动压入调用帧）
    /// - r4-r15：被调用者保存
    registers: [Value; 16],

    /// 操作数栈 — 子例程参数传递和临时存储
    stack: Vec<Value>,

    /// 全局变量存储 — 命名变量的运行时值，场景间保持
    variables: VariableStore,

    /// 全局旗标集合 — 命名布尔值的运行时状态，场景间保持
    flags: FlagSet,

    /// 子例程调用栈 — 每次 CALL 压入一帧，RETURN 弹出一帧
    call_stack: Vec<CallFrame>,
}

impl Vm {
    /// 创建一个新的 VM 实例，所有寄存器初始化为 `Value::Int(0)`。
    ///
    /// 变量存储和旗标集合为空，调用栈为空，程序计数器指向偏移 0。
    ///
    /// # 示例
    /// ```
    /// use aster_vm::Vm;
    ///
    /// let vm = Vm::new();
    /// assert_eq!(vm.pc(), 0);
    /// ```
    pub fn new() -> Self {
        Vm {
            pc: 0,
            registers: [
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
            ],
            stack: Vec::new(),
            variables: VariableStore::new(),
            flags: FlagSet::new(),
            call_stack: Vec::new(),
        }
    }

    // ─── 只读访问器（测试和外部使用）───────────────────────────────────

    /// 返回当前程序计数器（字节偏移）。
    ///
    /// 用于测试验证跳转指令是否正确设置 PC。
    #[inline]
    pub fn pc(&self) -> usize {
        self.pc
    }

    /// 返回 16 个通用寄存器的不可变引用。
    #[inline]
    pub fn registers(&self) -> &[Value; 16] {
        &self.registers
    }

    /// 返回全局变量存储的不可变引用。
    #[inline]
    pub fn variables(&self) -> &VariableStore {
        &self.variables
    }

    /// 返回全局变量存储的可变引用。
    ///
    /// SceneManager 可以通过此接口在场景执行前/后修改变量
    /// （如根据菜单选择结果设置变量）。
    #[inline]
    pub fn variables_mut(&mut self) -> &mut VariableStore {
        &mut self.variables
    }

    /// 返回全局旗标集合的不可变引用。
    #[inline]
    pub fn flags(&self) -> &FlagSet {
        &self.flags
    }

    /// 返回全局旗标集合的可变引用。
    #[inline]
    pub fn flags_mut(&mut self) -> &mut FlagSet {
        &mut self.flags
    }

    /// 返回调用栈深度的不可变引用（测试用）。
    #[inline]
    pub fn call_stack(&self) -> &[CallFrame] {
        &self.call_stack
    }

    // ─── 核心方法 ──────────────────────────────────────────────────────

    /// 执行单条字节码指令并返回 `VmAction`。
    ///
    /// 这是 VM 的核心方法。根据当前 PC 指向的 opcode 字节解码并执行指令。
    ///
    /// **执行模型**：
    /// - 纯内部指令（数据传送、算术运算、跳转）执行后继续循环，不返回
    /// - 外部渲染/音频指令执行后返回 `VmAction::Command(EngineCommand::...)`
    /// - 交互点（对话、菜单、结束）执行后返回对应的 `VmAction`
    /// - 无效操作码返回 `VmAction::Command(EngineCommand::Error { ... })`
    ///
    /// **PC 推进规则**：
    /// - 非跳转指令：`pc += instruction_size(opcode)`
    /// - 跳转指令（JUMP/等）：直接设置 `pc` 为目标偏移
    ///
    /// # 参数
    /// - `bytecode`：编译后的场景字节码
    ///
    /// # 返回值
    /// 需要调用方处理的动作（继续执行/等待输入/处理命令/结束）
    ///
    /// # Panic
    ///
    /// 此方法不会 panic。所有错误情况通过 `EngineCommand::Error` 返回。
    pub fn step(&mut self, bytecode: &CompiledScene) -> VmAction {
        let instructions = &bytecode.instructions;

        // 安全计数器：防止损坏的字节码导致无限内部循环
        // 单次 step() 最多执行 10,000 条内部指令
        const MAX_INTERNAL_STEPS: u32 = 10_000;
        let mut internal_steps: u32 = 0;

        // 主执行循环：内部指令（数据传送/跳转）在循环内执行，外部动作通过 return 退出
        loop {
            // 安全计数器检查
            internal_steps += 1;
            if internal_steps > MAX_INTERNAL_STEPS {
                // 采集 PC 周围的字节用于诊断
                let start = self.pc.saturating_sub(4);
                let end = (self.pc + 16).min(instructions.len());
                let surrounding: Vec<String> = instructions[start..end]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect();
                let pc_marker = self.pc - start;
                let hex_dump = surrounding.join(" ");
                return VmAction::Command(EngineCommand::Error {
                    message: format!(
                        "VM 内部循环超过 {} 步（偏移 {} / 0x{:04X}），\
                         周围字节 [{}-{}]: {}（PC 在字节 #{}）",
                        MAX_INTERNAL_STEPS,
                        self.pc,
                        self.pc,
                        start,
                        end - 1,
                        hex_dump,
                        pc_marker
                    ),
                });
            }

            // PC 越界检查
            if self.pc >= instructions.len() {
                return VmAction::Command(EngineCommand::Error {
                    message: format!(
                        "程序计数器 {} 超出字节码范围 {}（可能缺少 END 指令）",
                        self.pc,
                        instructions.len()
                    ),
                });
            }

            // 从字节码读取操作码字节
            let op_byte = instructions[self.pc];
            let opcode = match Opcode::from_byte(op_byte) {
                Some(op) => op,
                None => {
                    let msg = format!(
                        "无效操作码 0x{:02X} 位于指令偏移 {}（字节码版本 {} 可能不兼容）",
                        op_byte, self.pc, bytecode.version
                    );
                    return VmAction::Command(EngineCommand::Error { message: msg });
                }
            };

            let action = match opcode {
                // ══════════════════════════════════════════════════════════════
                // 数据传送指令 — 常量 → 寄存器
                // ══════════════════════════════════════════════════════════════
                Opcode::PushStr => {
                    let reg = instructions[self.pc + 1] as usize;
                    if reg >= 16 {
                        return VmAction::Command(EngineCommand::Error {
                            message: format!(
                                "PushStr: 非法寄存器索引 r{}（偏移 {}），字节码可能已损坏",
                                reg, self.pc
                            ),
                        });
                    }
                    let str_idx = opcode::read_u16(instructions, self.pc + 2) as usize;
                    let value = if str_idx < bytecode.constant_pool.len() {
                        Value::String(bytecode.constant_pool[str_idx].clone())
                    } else {
                        return VmAction::Command(EngineCommand::Error {
                            message: format!(
                                "PushStr: 常量池索引 {} 越界（常量池大小 {}）",
                                str_idx,
                                bytecode.constant_pool.len()
                            ),
                        });
                    };
                    self.registers[reg] = value;
                    self.pc += instruction_size(Opcode::PushStr);
                    continue; // 继续执行下一条指令
                }

                Opcode::PushInt => {
                    let reg = instructions[self.pc + 1] as usize;
                    if reg >= 16 {
                        return VmAction::Command(EngineCommand::Error {
                            message: format!(
                                "PushInt: 非法寄存器索引 r{}（偏移 {}），字节码可能已损坏",
                                reg, self.pc
                            ),
                        });
                    }
                    let value = opcode::read_i64(instructions, self.pc + 2);
                    self.registers[reg] = Value::Int(value);
                    self.pc += instruction_size(Opcode::PushInt);
                    continue;
                }

                Opcode::PushFloat => {
                    let reg = instructions[self.pc + 1] as usize;
                    if reg >= 16 {
                        return VmAction::Command(EngineCommand::Error {
                            message: format!(
                                "PushFloat: 非法寄存器索引 r{}（偏移 {}），字节码可能已损坏",
                                reg, self.pc
                            ),
                        });
                    }
                    let value = opcode::read_f64(instructions, self.pc + 2);
                    self.registers[reg] = Value::Float(value);
                    self.pc += instruction_size(Opcode::PushFloat);
                    continue;
                }

                Opcode::PushBool => {
                    let reg = instructions[self.pc + 1] as usize;
                    if reg >= 16 {
                        return VmAction::Command(EngineCommand::Error {
                            message: format!(
                                "PushBool: 非法寄存器索引 r{}（偏移 {}），字节码可能已损坏",
                                reg, self.pc
                            ),
                        });
                    }
                    let value = instructions[self.pc + 2] != 0;
                    self.registers[reg] = Value::Bool(value);
                    self.pc += instruction_size(Opcode::PushBool);
                    continue;
                }

                // ══════════════════════════════════════════════════════════════
                // 渲染指令 — 返回 VmAction::Command
                // ══════════════════════════════════════════════════════════════
                Opcode::Bg => {
                    let asset_idx = opcode::read_u16(instructions, self.pc + 1);
                    let trans_kind_idx = opcode::read_u16(instructions, self.pc + 3);
                    let dur_reg = instructions[self.pc + 5];
                    self.pc += instruction_size(Opcode::Bg);
                    VmAction::Command(EngineCommand::SetBg {
                        asset_idx,
                        trans_kind_idx,
                        dur_reg,
                    })
                }

                Opcode::ShowChar => {
                    let char_idx = opcode::read_u16(instructions, self.pc + 1);
                    let pos_byte = instructions[self.pc + 3];
                    let x_reg = instructions[self.pc + 4];
                    let y_reg = instructions[self.pc + 5];
                    let emotion_idx = opcode::read_u16(instructions, self.pc + 6);
                    let trans_kind_idx = opcode::read_u16(instructions, self.pc + 8);
                    let dur_reg = instructions[self.pc + 10];
                    self.pc += instruction_size(Opcode::ShowChar);
                    VmAction::Command(EngineCommand::ShowChar {
                        char_idx,
                        pos_byte,
                        x_reg,
                        y_reg,
                        emotion_idx,
                        trans_kind_idx,
                        dur_reg,
                    })
                }

                Opcode::ShowSprite => {
                    let asset_idx = opcode::read_u16(instructions, self.pc + 1);
                    let x_reg = instructions[self.pc + 3];
                    let y_reg = instructions[self.pc + 4];
                    let scale_reg = instructions[self.pc + 5];
                    let alpha_reg = instructions[self.pc + 6];
                    let trans_kind_idx = opcode::read_u16(instructions, self.pc + 7);
                    let dur_reg = instructions[self.pc + 9];
                    self.pc += instruction_size(Opcode::ShowSprite);
                    VmAction::Command(EngineCommand::ShowSprite {
                        asset_idx,
                        x_reg,
                        y_reg,
                        scale_reg,
                        alpha_reg,
                        trans_kind_idx,
                        dur_reg,
                    })
                }

                Opcode::MoveChar => {
                    let char_idx = opcode::read_u16(instructions, self.pc + 1);
                    let pos_byte = instructions[self.pc + 3];
                    let x_reg = instructions[self.pc + 4];
                    let y_reg = instructions[self.pc + 5];
                    let emotion_idx = opcode::read_u16(instructions, self.pc + 6);
                    let trans_kind_idx = opcode::read_u16(instructions, self.pc + 8);
                    let dur_reg = instructions[self.pc + 10];
                    self.pc += instruction_size(Opcode::MoveChar);
                    VmAction::Command(EngineCommand::MoveChar {
                        char_idx,
                        pos_byte,
                        x_reg,
                        y_reg,
                        emotion_idx,
                        trans_kind_idx,
                        dur_reg,
                    })
                }

                Opcode::Emotion => {
                    let char_idx = opcode::read_u16(instructions, self.pc + 1);
                    let emotion_idx = opcode::read_u16(instructions, self.pc + 3);
                    let trans_kind_idx = opcode::read_u16(instructions, self.pc + 5);
                    let dur_reg = instructions[self.pc + 7];
                    self.pc += instruction_size(Opcode::Emotion);
                    VmAction::Command(EngineCommand::Emotion {
                        char_idx,
                        emotion_idx,
                        trans_kind_idx,
                        dur_reg,
                    })
                }

                Opcode::HideChar => {
                    let char_idx = opcode::read_u16(instructions, self.pc + 1);
                    let trans_kind_idx = opcode::read_u16(instructions, self.pc + 3);
                    let dur_reg = instructions[self.pc + 5];
                    self.pc += instruction_size(Opcode::HideChar);
                    VmAction::Command(EngineCommand::HideChar {
                        char_idx,
                        trans_kind_idx,
                        dur_reg,
                    })
                }

                Opcode::HideSprite => {
                    let asset_idx = opcode::read_u16(instructions, self.pc + 1);
                    let trans_kind_idx = opcode::read_u16(instructions, self.pc + 3);
                    let dur_reg = instructions[self.pc + 5];
                    self.pc += instruction_size(Opcode::HideSprite);
                    VmAction::Command(EngineCommand::HideSprite {
                        asset_idx,
                        trans_kind_idx,
                        dur_reg,
                    })
                }

                // ══════════════════════════════════════════════════════════════
                // 对话/旁白 — 返回 SetDialogue/SetNarration 命令，
                // 调用方收到命令后渲染文本，再次 step() 时返回 WaitForInput
                // ══════════════════════════════════════════════════════════════
                Opcode::Dialogue => {
                    let speaker_idx = opcode::read_u16(instructions, self.pc + 1);
                    let text_idx = opcode::read_u16(instructions, self.pc + 3);
                    let voice_idx = opcode::read_u16(instructions, self.pc + 5);
                    self.pc += instruction_size(Opcode::Dialogue);
                    VmAction::Command(EngineCommand::SetDialogue {
                        speaker_idx,
                        text_idx,
                        voice_idx,
                    })
                }

                Opcode::Narrate => {
                    let text_idx = opcode::read_u16(instructions, self.pc + 1);
                    self.pc += instruction_size(Opcode::Narrate);
                    VmAction::Command(EngineCommand::SetNarration { text_idx })
                }

                // ══════════════════════════════════════════════════════════════
                // 交互指令 — Menu
                // ══════════════════════════════════════════════════════════════
                Opcode::Menu => {
                    let prompt_idx = opcode::read_u16(instructions, self.pc + 1);
                    let choice_count = instructions[self.pc + 3] as usize;
                    let mut choices = Vec::with_capacity(choice_count);

                    let mut pos = self.pc + 4;
                    for _ in 0..choice_count {
                        let text_idx = opcode::read_u16(instructions, pos);
                        pos += 2;
                        let target_offset = opcode::read_u16(instructions, pos);
                        pos += 2;
                        let condition_flag_idx = opcode::read_u16(instructions, pos);
                        pos += 2;
                        choices.push(MenuChoiceData {
                            text_idx,
                            target_offset,
                            condition_flag_idx,
                        });
                    }

                    // Menu 指令总长度：头部 4 + choices * 6
                    self.pc += opcode::menu_size(choice_count);
                    VmAction::ShowMenu {
                        prompt_idx,
                        choices,
                    }
                }

                // ══════════════════════════════════════════════════════════════
                // 控制流指令 — 无条件跳转
                // ══════════════════════════════════════════════════════════════
                Opcode::Jump => {
                    let target_offset = opcode::read_u16(instructions, self.pc + 1) as usize;
                    self.pc = target_offset;
                    continue; // 继续执行下一条指令 // 尾递归：跳转后继续执行
                }

                // ══════════════════════════════════════════════════════════════
                // 媒体指令
                // ══════════════════════════════════════════════════════════════
                Opcode::PlayBgm => {
                    let asset_idx = opcode::read_u16(instructions, self.pc + 1);
                    let fade_reg = instructions[self.pc + 3];
                    let looping = instructions[self.pc + 4] != 0;
                    self.pc += instruction_size(Opcode::PlayBgm);
                    VmAction::Command(EngineCommand::PlayBgm {
                        asset_idx,
                        fade_reg,
                        looping,
                    })
                }

                Opcode::StopBgm => {
                    let fade_reg = instructions[self.pc + 1];
                    self.pc += instruction_size(Opcode::StopBgm);
                    VmAction::Command(EngineCommand::StopBgm { fade_reg })
                }

                Opcode::PlaySe => {
                    let asset_idx = opcode::read_u16(instructions, self.pc + 1);
                    let fade_reg = instructions[self.pc + 3];
                    self.pc += instruction_size(Opcode::PlaySe);
                    VmAction::Command(EngineCommand::PlaySe {
                        asset_idx,
                        fade_reg,
                    })
                }

                Opcode::PlayVoice => {
                    let asset_idx = opcode::read_u16(instructions, self.pc + 1);
                    self.pc += instruction_size(Opcode::PlayVoice);
                    VmAction::Command(EngineCommand::PlayVoice { asset_idx })
                }

                // ══════════════════════════════════════════════════════════════
                // 特效指令
                // ══════════════════════════════════════════════════════════════
                Opcode::Effect => {
                    let type_idx = opcode::read_u16(instructions, self.pc + 1);
                    let param_count = instructions[self.pc + 3] as usize;
                    let mut params = Vec::with_capacity(param_count);

                    let mut pos = self.pc + 4;
                    for _ in 0..param_count {
                        let key_idx = opcode::read_u16(instructions, pos);
                        pos += 2;
                        let value_reg = opcode::read_u16(instructions, pos);
                        pos += 2;
                        params.push((key_idx, value_reg));
                    }

                    self.pc += opcode::effect_size(param_count);
                    VmAction::Command(EngineCommand::Effect { type_idx, params })
                }

                // ══════════════════════════════════════════════════════════════
                // 时序指令
                // ══════════════════════════════════════════════════════════════
                Opcode::Wait => {
                    let dur_reg = instructions[self.pc + 1];
                    self.pc += instruction_size(Opcode::Wait);
                    VmAction::Command(EngineCommand::Wait { dur_reg })
                }

                // ══════════════════════════════════════════════════════════════
                // 特殊指令 — 场景结束
                // ══════════════════════════════════════════════════════════════
                Opcode::End => {
                    // END 不推进 PC——场景已结束
                    VmAction::SceneEnd
                }

                // ══════════════════════════════════════════════════════════════
                // Label 伪指令 — 字节码中不应出现
                // ══════════════════════════════════════════════════════════════
                Opcode::Label => {
                    // Label 不产生字节码。如果因字节码损坏出现在指令流中，
                    // 视为错误。推进 PC 跳过此指令以避免死循环。
                    let msg = format!("Label 伪指令不应出现在字节码指令流中（偏移 {}）", self.pc);
                    self.pc += 1;
                    VmAction::Command(EngineCommand::Error { message: msg })
                }

                // ══════════════════════════════════════════════════════════════
                // PH1-T14 才实现的指令 — 当前返回"未实现"错误
                // 推进 PC 跳过该指令，避免调用方在未处理的指令上死循环
                // ══════════════════════════════════════════════════════════════
                Opcode::LoadVar
                | Opcode::StoreVar
                | Opcode::CheckFlag
                | Opcode::Add
                | Opcode::Sub
                | Opcode::Mul
                | Opcode::Div
                | Opcode::Eq
                | Opcode::Neq
                | Opcode::Lt
                | Opcode::Gt
                | Opcode::Le
                | Opcode::Ge
                | Opcode::And
                | Opcode::Or
                | Opcode::Not
                | Opcode::Neg
                | Opcode::JumpIf
                | Opcode::JumpIfFlag
                | Opcode::Call
                | Opcode::Return
                | Opcode::Goto
                | Opcode::SetVar
                | Opcode::SetFlag
                | Opcode::UnsetFlag
                | Opcode::ToggleFlag => {
                    let size = instruction_size(opcode);
                    let msg = format!(
                        "操作码 {} (0x{:02X}) 尚未实现（计划在 PH1-T14 中实现）",
                        opcode, op_byte
                    );
                    self.pc += size;
                    VmAction::Command(EngineCommand::Error { message: msg })
                }
            }; // let action = match { ... };
            return action;
        } // loop 结束
    }

    /// 设置程序计数器到指定偏移。
    ///
    /// 用于 SceneManager 在菜单选择后跳转到目标位置，
    /// 或跳过某条指令（如跳过已处理的对话）。
    ///
    /// # 参数
    /// - `offset`：目标字节偏移
    pub fn set_pc(&mut self, offset: usize) {
        self.pc = offset;
    }

    /// 将 VM 状态重置为初始状态（所有寄存器归零、栈清空、变量/旗标清空）。
    ///
    /// 通常在项目重新加载时调用。
    pub fn reset(&mut self) {
        self.pc = 0;
        self.registers = [
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
        ];
        self.stack.clear();
        self.variables.clear();
        self.flags.clear();
        self.call_stack.clear();
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    // ─── 辅助函数 ──────────────────────────────────────────────────────

    /// 创建一个仅含 END 指令的 CompiledScene。
    fn make_empty_scene() -> CompiledScene {
        CompiledScene {
            version: 1,
            instructions: vec![Opcode::End as u8],
            constant_pool: vec![],
            label_table: HashMap::new(),
        }
    }

    /// 创建一个含指定字节码指令的 CompiledScene（自动追加 END）。
    fn make_scene_with_instructions(
        instructions: Vec<u8>,
        constant_pool: Vec<String>,
    ) -> CompiledScene {
        let mut insts = instructions;
        insts.push(Opcode::End as u8);
        CompiledScene {
            version: 1,
            instructions: insts,
            constant_pool,
            label_table: HashMap::new(),
        }
    }

    /// 构造 PushStr 指令的字节码：op(1) + reg(1) + str_idx(2)。
    fn encode_push_str(reg: u8, str_idx: u16) -> Vec<u8> {
        let mut bytes = vec![Opcode::PushStr as u8, reg];
        bytes.extend_from_slice(&str_idx.to_le_bytes());
        bytes
    }

    /// 构造 PushInt 指令的字节码：op(1) + reg(1) + value(8)。
    fn encode_push_int(reg: u8, value: i64) -> Vec<u8> {
        let mut bytes = vec![Opcode::PushInt as u8, reg];
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    /// 构造 Dialogue 指令的字节码：op(1) + speaker_idx(2) + text_idx(2) + voice_idx(2)。
    fn encode_dialogue(speaker_idx: u16, text_idx: u16, voice_idx: u16) -> Vec<u8> {
        let mut bytes = vec![Opcode::Dialogue as u8];
        bytes.extend_from_slice(&speaker_idx.to_le_bytes());
        bytes.extend_from_slice(&text_idx.to_le_bytes());
        bytes.extend_from_slice(&voice_idx.to_le_bytes());
        bytes
    }

    /// 构造 Jump 指令的字节码：op(1) + offset(2)。
    fn encode_jump(offset: u16) -> Vec<u8> {
        let mut bytes = vec![Opcode::Jump as u8];
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes
    }

    /// 构造 Menu 指令的字节码：op(1) + prompt_idx(2) + count(1) + choices...
    fn encode_menu(prompt_idx: u16, choices: &[(u16, u16, u16)]) -> Vec<u8> {
        let mut bytes = vec![Opcode::Menu as u8];
        bytes.extend_from_slice(&prompt_idx.to_le_bytes());
        bytes.push(choices.len() as u8);
        for &(text_idx, target_offset, cond_flag_idx) in choices {
            bytes.extend_from_slice(&text_idx.to_le_bytes());
            bytes.extend_from_slice(&target_offset.to_le_bytes());
            bytes.extend_from_slice(&cond_flag_idx.to_le_bytes());
        }
        bytes
    }

    // ══════════════════════════════════════════════════════════════════
    // AC01 — VM 能加载 CompiledScene 并执行到 END 指令
    // ══════════════════════════════════════════════════════════════════

    /// AC01 — 空场景（仅含 END）执行后返回 SceneEnd。
    #[test]
    fn ac01_empty_scene_returns_scene_end() {
        let mut vm = Vm::new();
        let scene = make_empty_scene();
        let action = vm.step(&scene);
        assert_eq!(action, VmAction::SceneEnd);
    }

    /// AC01 补充 — 含一条 PushInt + END 的场景能正确推进到 END。
    #[test]
    fn ac01_scene_with_push_then_end() {
        let mut vm = Vm::new();
        let mut instructions = encode_push_int(0, 42);
        instructions.push(Opcode::End as u8);
        let scene = CompiledScene {
            version: 1,
            instructions,
            constant_pool: vec![],
            label_table: HashMap::new(),
        };

        // 第一次 step：应执行 PushInt（内部指令），继续执行到 END
        let action = vm.step(&scene);
        assert_eq!(action, VmAction::SceneEnd, "应从 PushInt 连续执行到 END");
        // 验证 PushInt 的副作用：r0 = 42
        assert_eq!(vm.registers[0], Value::Int(42));
    }

    // ══════════════════════════════════════════════════════════════════
    // AC02 — DIALOGUE 指令发出正确的 EngineCommand::SetDialogue
    // ══════════════════════════════════════════════════════════════════

    /// AC02 — Dialogue 指令返回 SetDialogue 命令，包含正确的 speaker 和 text 索引。
    #[test]
    fn ac02_dialogue_returns_set_dialogue_command() {
        let mut vm = Vm::new();
        let constant_pool = vec![
            "小百合".to_string(),        // pool[0] = speaker
            "初次见面！".to_string(),    // pool[1] = text
            "voice_001.ogg".to_string(), // pool[2] = voice
        ];

        // DIALOGUE: speaker_idx=0, text_idx=1, voice_idx=2
        let mut instructions = encode_dialogue(0, 1, 2);
        instructions.push(Opcode::End as u8);
        let scene = CompiledScene {
            version: 1,
            instructions,
            constant_pool,
            label_table: HashMap::new(),
        };

        let action = vm.step(&scene);

        match action {
            VmAction::Command(EngineCommand::SetDialogue {
                speaker_idx,
                text_idx,
                voice_idx,
            }) => {
                assert_eq!(speaker_idx, 0);
                assert_eq!(text_idx, 1);
                assert_eq!(voice_idx, 2);

                // 验证常量池解析结果
                let scene = &scene;
                assert_eq!(scene.constant_pool[speaker_idx as usize], "小百合");
                assert_eq!(scene.constant_pool[text_idx as usize], "初次见面！");
                assert_eq!(scene.constant_pool[voice_idx as usize], "voice_001.ogg");
            }
            other => panic!("期望 SetDialogue 命令，实际为 {:?}", other),
        }
    }

    /// AC02 补充 — Narrate 指令返回 SetNarration 命令。
    #[test]
    fn ac02_narrate_returns_set_narration_command() {
        let mut vm = Vm::new();
        let constant_pool = vec!["这是一个春天...".to_string()];

        // NARRATE: text_idx=0
        let mut instructions = vec![Opcode::Narrate as u8];
        instructions.extend_from_slice(&0u16.to_le_bytes());
        instructions.push(Opcode::End as u8);
        let scene = CompiledScene {
            version: 1,
            instructions,
            constant_pool,
            label_table: HashMap::new(),
        };

        let action = vm.step(&scene);

        match action {
            VmAction::Command(EngineCommand::SetNarration { text_idx }) => {
                assert_eq!(text_idx, 0);
            }
            other => panic!("期望 SetNarration 命令，实际为 {:?}", other),
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // AC03 — JUMP 指令正确修改 PC 到目标标签
    // ══════════════════════════════════════════════════════════════════

    /// AC03 — Jump 指令将 PC 跳转到目标偏移。
    ///
    /// 场景布局：
    ///   [0] JUMP → offset 11  (3 bytes)
    ///   [3] PushInt r0, 999   (10 bytes) ← 应被跳过
    ///   [13] END              (1 byte)
    #[test]
    fn ac03_jump_modifies_pc_correctly() {
        let mut vm = Vm::new();

        // JUMP 到偏移 13（即 END 的位置）
        let mut instructions = encode_jump(13);
        // PushInt r0, 999（应被跳过）
        instructions.extend(encode_push_int(0, 999));
        // END 在偏移 13
        instructions.push(Opcode::End as u8);

        let scene = CompiledScene {
            version: 1,
            instructions,
            constant_pool: vec![],
            label_table: HashMap::new(),
        };

        let action = vm.step(&scene);

        // 应执行 JUMP → 跳转到 13 → 执行 END → 返回 SceneEnd
        assert_eq!(action, VmAction::SceneEnd);
        // PushInt 被跳过，r0 应保持默认值 0 而非 999
        assert_eq!(vm.registers()[0], Value::Int(0));
    }

    /// AC03 补充 — 验证 step() 执行后 PC 停留在 END 指令位置。
    #[test]
    fn ac03_pc_stops_at_end_after_jump() {
        let mut vm = Vm::new();

        // 场景：END 在偏移 0，但我们在偏移 0 放 JUMP → 偏移 3 的 END
        let mut instructions = encode_jump(3);
        instructions.push(Opcode::End as u8); // 偏移 3

        let scene = CompiledScene {
            version: 1,
            instructions,
            constant_pool: vec![],
            label_table: HashMap::new(),
        };

        let action = vm.step(&scene);
        assert_eq!(action, VmAction::SceneEnd);
        // PC 保持在 END 指令的位置
        assert_eq!(vm.pc(), 3);
    }

    // ══════════════════════════════════════════════════════════════════
    // AC04 — 执行无效操作码（损坏的字节码）不 panic
    // ══════════════════════════════════════════════════════════════════

    /// AC04 — 非法操作码 0xFE 返回 Error 而非 panic。
    #[test]
    fn ac04_invalid_opcode_does_not_panic() {
        let mut vm = Vm::new();
        let scene = CompiledScene {
            version: 1,
            instructions: vec![0xFE], // 非法操作码
            constant_pool: vec![],
            label_table: HashMap::new(),
        };

        let action = vm.step(&scene);

        match action {
            VmAction::Command(EngineCommand::Error { message }) => {
                assert!(message.contains("无效操作码"));
                assert!(message.contains("0xFE"));
            }
            other => panic!("期望 Error 命令，实际为 {:?}", other),
        }
    }

    /// AC04 补充 — 未实现的操作码（如 0x05 LoadVar）返回友好的未实现错误。
    #[test]
    fn ac04_unimplemented_opcode_returns_error_not_panic() {
        let mut vm = Vm::new();
        // LoadVar (0x05) 是 PH1-T14 的指令，当前应返回错误
        let mut instructions = vec![Opcode::LoadVar as u8, 0, 0, 0]; // 4 bytes
        instructions.push(Opcode::End as u8);
        let scene = CompiledScene {
            version: 1,
            instructions,
            constant_pool: vec![],
            label_table: HashMap::new(),
        };

        let action = vm.step(&scene);

        match action {
            VmAction::Command(EngineCommand::Error { message }) => {
                assert!(
                    message.contains("尚未实现"),
                    "错误消息应说明指令未实现：{}",
                    message
                );
            }
            other => panic!("期望 Error 命令，实际为 {:?}", other),
        }
    }

    /// AC04 补充 — 常量池索引越界返回 Error 而非 panic。
    #[test]
    fn ac04_constant_pool_out_of_bounds_returns_error() {
        let mut vm = Vm::new();
        // PushStr 引用不存在的常量池索引 5（但常量池只有 1 个条目）
        let mut instructions = encode_push_str(0, 5);
        instructions.push(Opcode::End as u8);
        let scene = CompiledScene {
            version: 1,
            instructions,
            constant_pool: vec!["only_one".to_string()],
            label_table: HashMap::new(),
        };

        let action = vm.step(&scene);

        match action {
            VmAction::Command(EngineCommand::Error { message }) => {
                assert!(message.contains("越界"));
            }
            other => panic!("期望 Error 命令，实际为 {:?}", other),
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // AC05 — VM 执行 1000 条指令耗时 < 0.5ms
    // ══════════════════════════════════════════════════════════════════

    /// AC05 — 1000 条 PushInt 指令的执行时间 < 0.5ms。
    #[test]
    fn ac05_performance_1000_instructions_under_half_millisecond() {
        let mut vm = Vm::new();

        // 构造 1000 条 PushInt 指令 + 1 条 END
        let mut instructions = Vec::with_capacity(1000 * 10 + 1);
        for i in 0..1000 {
            instructions.extend(encode_push_int(0, i as i64));
        }
        instructions.push(Opcode::End as u8);

        let scene = CompiledScene {
            version: 1,
            instructions,
            constant_pool: vec![],
            label_table: HashMap::new(),
        };

        let start = std::time::Instant::now();
        let action = vm.step(&scene);
        let elapsed = start.elapsed();

        // 应执行到 END
        assert_eq!(action, VmAction::SceneEnd);

        // AC05 核心断言：< 500 微秒
        let elapsed_micros = elapsed.as_micros();
        assert!(
            elapsed_micros < 500,
            "AC05 失败：1000 条指令执行耗时 {}μs，期望 < 500μs",
            elapsed_micros
        );

        // 验证最后一条 PushInt 的值（第 1000 条 = i=999）
        assert_eq!(vm.registers()[0], Value::Int(999));
    }

    // ══════════════════════════════════════════════════════════════════
    // 数据传送指令测试
    // ══════════════════════════════════════════════════════════════════

    /// PushInt 将立即数正确加载到寄存器。
    #[test]
    fn push_int_loads_value_to_register() {
        let mut vm = Vm::new();
        let instructions = encode_push_int(3, -12345);
        let scene = make_scene_with_instructions(instructions, vec![]);

        let _ = vm.step(&scene); // PushInt (内部) → END → SceneEnd
        assert_eq!(vm.registers[3], Value::Int(-12345));
    }

    /// PushFloat 将浮点数正确加载到寄存器。
    #[test]
    fn push_float_loads_value_to_register() {
        let mut vm = Vm::new();
        let mut instructions = vec![Opcode::PushFloat as u8, 1];
        instructions.extend_from_slice(&std::f64::consts::PI.to_le_bytes());
        let scene = make_scene_with_instructions(instructions, vec![]);

        let _ = vm.step(&scene);
        match vm.registers[1] {
            Value::Float(f) => assert!((f - std::f64::consts::PI).abs() < f64::EPSILON),
            ref other => panic!("期望 Float(PI)，实际为 {:?}", other),
        }
    }

    /// PushBool 将布尔值正确加载到寄存器。
    #[test]
    fn push_bool_loads_value_to_register() {
        let mut vm = Vm::new();
        // PushBool r5, true
        let instructions = vec![Opcode::PushBool as u8, 5, 1];
        let scene = make_scene_with_instructions(instructions, vec![]);

        let _ = vm.step(&scene);
        assert_eq!(vm.registers[5], Value::Bool(true));
    }

    /// PushStr 将常量池字符串正确加载到寄存器。
    #[test]
    fn push_str_loads_value_from_constant_pool() {
        let mut vm = Vm::new();
        let pool = vec!["樱花飘落的季节".to_string()];
        let instructions = encode_push_str(2, 0);
        let scene = make_scene_with_instructions(instructions, pool);

        let _ = vm.step(&scene);
        assert_eq!(vm.registers[2], Value::String("樱花飘落的季节".to_string()));
    }

    // ══════════════════════════════════════════════════════════════════
    // 渲染指令测试
    // ══════════════════════════════════════════════════════════════════

    /// Bg 指令返回 SetBg 命令。
    #[test]
    fn bg_returns_set_bg_command() {
        let mut vm = Vm::new();
        // Bg: op(1) + asset_idx(2) + trans_kind_idx(2) + dur_reg(1)
        let mut instructions = vec![Opcode::Bg as u8];
        instructions.extend_from_slice(&0u16.to_le_bytes()); // asset_idx = 0
        instructions.extend_from_slice(&0xFFFFu16.to_le_bytes()); // trans_kind = NONE
        instructions.push(0xFF); // dur_reg = NONE
        let scene = make_scene_with_instructions(instructions, vec!["bg_school.png".to_string()]);

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::SetBg {
                asset_idx,
                trans_kind_idx,
                dur_reg,
            }) => {
                assert_eq!(asset_idx, 0);
                assert_eq!(trans_kind_idx, 0xFFFF);
                assert_eq!(dur_reg, 0xFF);
            }
            other => panic!("期望 SetBg 命令，实际为 {:?}", other),
        }
    }

    /// ShowChar 指令返回 ShowChar 命令。
    #[test]
    fn show_char_returns_show_char_command() {
        let mut vm = Vm::new();
        // ShowChar: op(1) + char_idx(2) + pos(1) + x_reg(1) + y_reg(1)
        //           + emotion_idx(2) + trans_kind_idx(2) + dur_reg(1)
        let mut instructions = vec![Opcode::ShowChar as u8];
        instructions.extend_from_slice(&0u16.to_le_bytes()); // char_idx=0
        instructions.push(1); // pos=Center
        instructions.push(0xFF); // x_reg=NONE
        instructions.push(0xFF); // y_reg=NONE
        instructions.extend_from_slice(&1u16.to_le_bytes()); // emotion_idx=1
        instructions.extend_from_slice(&0xFFFFu16.to_le_bytes()); // trans_kind=NONE
        instructions.push(0xFF); // dur_reg=NONE
        let scene = make_scene_with_instructions(
            instructions,
            vec!["sayori".to_string(), "smile".to_string()],
        );

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::ShowChar {
                char_idx,
                pos_byte,
                emotion_idx,
                ..
            }) => {
                assert_eq!(char_idx, 0);
                assert_eq!(pos_byte, 1); // Center
                assert_eq!(emotion_idx, 1);
            }
            other => panic!("期望 ShowChar 命令，实际为 {:?}", other),
        }
    }

    /// HideChar 指令返回 HideChar 命令。
    #[test]
    fn hide_char_returns_hide_char_command() {
        let mut vm = Vm::new();
        let mut instructions = vec![Opcode::HideChar as u8];
        instructions.extend_from_slice(&0u16.to_le_bytes()); // char_idx
        instructions.extend_from_slice(&0xFFFFu16.to_le_bytes()); // trans_kind
        instructions.push(0xFF); // dur_reg
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        assert!(matches!(
            action,
            VmAction::Command(EngineCommand::HideChar { .. })
        ));
    }

    // ══════════════════════════════════════════════════════════════════
    // Menu 指令测试
    // ══════════════════════════════════════════════════════════════════

    /// Menu 指令返回 ShowMenu 动作，choices 内容正确。
    #[test]
    fn menu_returns_show_menu_with_correct_choices() {
        let mut vm = Vm::new();
        let choices = vec![
            (0u16, 100u16, 0xFFFFu16), // text=0, target=100, no condition
            (1u16, 200u16, 5u16),      // text=1, target=200, flag=5
        ];
        let instructions = encode_menu(10, &choices);
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        match action {
            VmAction::ShowMenu {
                prompt_idx,
                choices: stored_choices,
            } => {
                assert_eq!(prompt_idx, 10);
                assert_eq!(stored_choices.len(), 2);
                assert_eq!(stored_choices[0].text_idx, 0);
                assert_eq!(stored_choices[0].target_offset, 100);
                assert_eq!(stored_choices[0].condition_flag_idx, 0xFFFF);
                assert_eq!(stored_choices[1].text_idx, 1);
                assert_eq!(stored_choices[1].target_offset, 200);
                assert_eq!(stored_choices[1].condition_flag_idx, 5);
            }
            other => panic!("期望 ShowMenu 动作，实际为 {:?}", other),
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // 媒体指令测试
    // ══════════════════════════════════════════════════════════════════

    /// PlayBgm 返回 PlayBgm 命令。
    #[test]
    fn play_bgm_returns_play_bgm_command() {
        let mut vm = Vm::new();
        let mut instructions = vec![Opcode::PlayBgm as u8];
        instructions.extend_from_slice(&0u16.to_le_bytes()); // asset_idx
        instructions.push(2); // fade_reg
        instructions.push(1); // looping = true
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::PlayBgm {
                asset_idx,
                fade_reg,
                looping,
            }) => {
                assert_eq!(asset_idx, 0);
                assert_eq!(fade_reg, 2);
                assert!(looping);
            }
            other => panic!("期望 PlayBgm 命令，实际为 {:?}", other),
        }
    }

    /// StopBgm 返回 StopBgm 命令。
    #[test]
    fn stop_bgm_returns_stop_bgm_command() {
        let mut vm = Vm::new();
        let instructions = vec![Opcode::StopBgm as u8, 1]; // fade_reg=1
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::StopBgm { fade_reg }) => {
                assert_eq!(fade_reg, 1);
            }
            other => panic!("期望 StopBgm 命令，实际为 {:?}", other),
        }
    }

    /// PlaySe 返回 PlaySe 命令。
    #[test]
    fn play_se_returns_play_se_command() {
        let mut vm = Vm::new();
        let mut instructions = vec![Opcode::PlaySe as u8];
        instructions.extend_from_slice(&3u16.to_le_bytes()); // asset_idx
        instructions.push(0xFF); // fade_reg
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::PlaySe {
                asset_idx,
                fade_reg,
            }) => {
                assert_eq!(asset_idx, 3);
                assert_eq!(fade_reg, 0xFF);
            }
            other => panic!("期望 PlaySe 命令，实际为 {:?}", other),
        }
    }

    /// PlayVoice 返回 PlayVoice 命令。
    #[test]
    fn play_voice_returns_play_voice_command() {
        let mut vm = Vm::new();
        let mut instructions = vec![Opcode::PlayVoice as u8];
        instructions.extend_from_slice(&1u16.to_le_bytes());
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::PlayVoice { asset_idx }) => {
                assert_eq!(asset_idx, 1);
            }
            other => panic!("期望 PlayVoice 命令，实际为 {:?}", other),
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // VM 生命周期测试
    // ══════════════════════════════════════════════════════════════════

    /// Vm::new() 正确初始化所有状态。
    #[test]
    fn vm_new_initializes_all_state() {
        let vm = Vm::new();
        assert_eq!(vm.pc(), 0);
        for i in 0..16 {
            assert_eq!(vm.registers[i], Value::Int(0));
        }
        assert!(vm.variables().is_empty());
        assert!(vm.flags().is_empty());
        assert!(vm.call_stack().is_empty());
    }

    /// Vm::reset() 将状态恢复到初始值。
    #[test]
    fn vm_reset_clears_all_state() {
        let mut vm = Vm::new();
        // 修改状态
        vm.set_pc(42);
        vm.registers[0] = Value::String("dirty".to_string());
        vm.variables_mut().set("x", Value::Int(100));
        vm.flags_mut().set("flag");

        // 重置
        vm.reset();

        assert_eq!(vm.pc(), 0);
        assert_eq!(vm.registers[0], Value::Int(0));
        assert!(vm.variables().is_empty());
        assert!(vm.flags().is_empty());
        assert!(vm.call_stack().is_empty());
    }

    /// set_pc() 正确修改程序计数器。
    #[test]
    fn set_pc_modifies_program_counter() {
        let mut vm = Vm::new();
        vm.set_pc(100);
        assert_eq!(vm.pc(), 100);
    }

    /// Effect 指令正确解码并返回。
    #[test]
    fn effect_returns_effect_command() {
        let mut vm = Vm::new();
        // Effect: op(1) + type_idx(2) + count(1) + params...
        let mut instructions = vec![Opcode::Effect as u8];
        instructions.extend_from_slice(&0u16.to_le_bytes()); // type_idx=0
        instructions.push(2); // 2 params
        // param 0: key_idx=1, value_reg=3
        instructions.extend_from_slice(&1u16.to_le_bytes());
        instructions.extend_from_slice(&3u16.to_le_bytes());
        // param 1: key_idx=2, value_reg=4
        instructions.extend_from_slice(&2u16.to_le_bytes());
        instructions.extend_from_slice(&4u16.to_le_bytes());
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::Effect { type_idx, params }) => {
                assert_eq!(type_idx, 0);
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], (1, 3));
                assert_eq!(params[1], (2, 4));
            }
            other => panic!("期望 Effect 命令，实际为 {:?}", other),
        }
    }

    /// Wait 指令返回 Wait 命令。
    #[test]
    fn wait_returns_wait_command() {
        let mut vm = Vm::new();
        let instructions = vec![Opcode::Wait as u8, 5]; // dur_reg=5
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::Wait { dur_reg }) => {
                assert_eq!(dur_reg, 5);
            }
            other => panic!("期望 Wait 命令，实际为 {:?}", other),
        }
    }

    /// Label 伪指令在字节码流中应返回错误。
    #[test]
    fn label_in_bytecode_stream_returns_error() {
        let mut vm = Vm::new();
        let instructions = vec![Opcode::Label as u8];
        let scene = make_scene_with_instructions(instructions, vec![]);

        let action = vm.step(&scene);
        match action {
            VmAction::Command(EngineCommand::Error { message }) => {
                assert!(message.contains("Label"));
            }
            other => panic!("期望 Error 命令，实际为 {:?}", other),
        }
    }
}
