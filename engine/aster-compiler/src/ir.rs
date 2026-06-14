//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/src/ir.rs
//! 功能概述：中间表示（IR）类型定义 — 定义 `IrInstruction` 枚举（46 个变体）和辅助类型，
//!           作为 AST→字节码 之间的扁平化表示层。所有复杂表达式已降级为寄存器操作，
//!           控制流已展开为条件/无条件跳转。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - aster_core（Position 类型引用）
//! - std::fmt（Display 实现）
//!
//! ## 设计说明
//!
//! IR 指令使用以下类型约定：
//! - `reg: u8` — 寄存器索引（0-15），`0xFF` = 无/不使用
//! - `pool_idx: u16` — 常量池索引，`0xFFFF` = 无/不使用
//! - `label: String` — 标签名（用户定义或自动生成），字节码编码阶段解析为偏移
//! - `i64` / `f64` / `bool` — 立即数值
//!
//! ## 寄存器分配
//!
//! `RegisterAllocator` 提供简单线性分配（0→15），每次 SceneNode 编译后复位。
//! PH1-T12 的窥孔优化会清理冗余分配。

use std::fmt;

use aster_core::Position;

/// 寄存器哨兵值 — 表示"无寄存器"（可选字段未提供）
pub const NONE_REG: u8 = 0xFF;

/// 常量池索引哨兵值 — 表示"无池条目"（可选字段未提供）
pub const NONE_POOL: u16 = 0xFFFF;

/// 最大寄存器数量
pub const MAX_REGISTERS: u8 = 16;

// ============================================================================
// IrInstruction — 中间表示指令集
// ============================================================================

/// 中间表示指令枚举 — 表示编译后的单条扁平化指令。
///
/// 所有 SceneNode 变体和 Expr 树在编译阶段被转换为一系列 `IrInstruction`。
/// IR 指令已经过：
/// - 表达式降级（Expr 树 → 寄存器操作序列）
/// - 控制流展开（Branch → 条件跳转 + 内部标签）
/// - 常量池化（所有字符串字面量 → 常量池索引引用）
///
/// 标签（Label）在 IR 中保留字符串名称，在字节码编码阶段解析为偏移量。
///
/// # 变体分类
///
/// | 分类 | 变体数量 | 说明 |
/// |------|----------|------|
/// | 数据传送 | 7 | Push*/LoadVar/StoreVar/CheckFlag |
/// | 算术运算 | 4 | Add/Sub/Mul/Div |
/// | 比较运算 | 6 | Eq/Neq/Lt/Gt/Le/Ge |
/// | 逻辑运算 | 2 | And/Or |
/// | 一元运算 | 2 | Not/Neg |
/// | 渲染指令 | 9 | Bg/ShowChar/ShowSprite/MoveChar/Emotion/HideChar/HideSprite/Dialogue/Narrate |
/// | 交互指令 | 1 | Menu |
/// | 控制流 | 7 | Jump/JumpIf/JumpIfFlag/Call/Return/Label/Goto |
/// | 状态指令 | 4 | SetVar/SetFlag/UnsetFlag/ToggleFlag |
/// | 媒体指令 | 5 | PlayBgm/StopBgm/PlaySe/PlayVoice/Effect |
/// | 时序指令 | 1 | Wait |
/// | 特殊 | 1 | End |
#[derive(Debug, Clone, PartialEq)]
pub enum IrInstruction {
    // ─── 数据传送指令 ───────────────────────────────────────────────
    /// 将字符串常量压入寄存器
    /// - reg: 目标寄存器
    /// - str_idx: 常量池中字符串字面量的索引
    PushStr { reg: u8, str_idx: u16 },

    /// 将整型立即数压入寄存器
    PushInt { reg: u8, value: i64 },

    /// 将浮点立即数压入寄存器
    PushFloat { reg: u8, value: f64 },

    /// 将布尔立即数压入寄存器
    PushBool { reg: u8, value: bool },

    /// 从变量存储加载值到寄存器
    /// - dst: 目标寄存器
    /// - name_idx: 常量池中变量名的索引
    LoadVar { dst: u8, name_idx: u16 },

    /// 将寄存器值存入变量存储
    /// - name_idx: 常量池中变量名的索引
    /// - src: 源寄存器
    StoreVar { name_idx: u16, src: u8 },

    /// 检查旗标状态，结果（bool）存入寄存器
    /// - dst: 目标寄存器（存储 true/false）
    /// - flag_idx: 常量池中旗标名的索引
    CheckFlag { dst: u8, flag_idx: u16 },

    // ─── 算术运算指令 ───────────────────────────────────────────────
    /// 加法：dst = left + right
    Add { dst: u8, left: u8, right: u8 },
    /// 减法：dst = left - right
    Sub { dst: u8, left: u8, right: u8 },
    /// 乘法：dst = left * right
    Mul { dst: u8, left: u8, right: u8 },
    /// 除法：dst = left / right
    Div { dst: u8, left: u8, right: u8 },

    // ─── 比较运算指令 ───────────────────────────────────────────────
    /// 等于：dst = (left == right)
    Eq { dst: u8, left: u8, right: u8 },
    /// 不等于：dst = (left != right)
    Neq { dst: u8, left: u8, right: u8 },
    /// 小于：dst = (left < right)
    Lt { dst: u8, left: u8, right: u8 },
    /// 大于：dst = (left > right)
    Gt { dst: u8, left: u8, right: u8 },
    /// 小于等于：dst = (left <= right)
    Le { dst: u8, left: u8, right: u8 },
    /// 大于等于：dst = (left >= right)
    Ge { dst: u8, left: u8, right: u8 },

    // ─── 逻辑运算指令 ───────────────────────────────────────────────
    /// 逻辑与：dst = left && right
    And { dst: u8, left: u8, right: u8 },
    /// 逻辑或：dst = left || right
    Or { dst: u8, left: u8, right: u8 },

    // ─── 一元运算指令 ───────────────────────────────────────────────
    /// 逻辑非：dst = !src
    Not { dst: u8, src: u8 },
    /// 算术取负：dst = -src
    Neg { dst: u8, src: u8 },

    // ─── 渲染指令 ───────────────────────────────────────────────────
    /// 切换背景图片
    /// - asset_idx: 背景资源路径的常量池索引
    /// - trans_kind_idx: 转场类型名的常量池索引（NONE_POOL = 无转场）
    /// - dur_reg: 转场持续时长的寄存器（0xFF = 默认）
    Bg {
        asset_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示 / 重新出场角色立绘
    /// - char_idx: 角色 ID 的常量池索引
    /// - pos: 立绘位置编码（Left/Center/Right 或 Custom(x_reg, y_reg)）
    /// - emotion_idx: 表情名的常量池索引（NONE_POOL = 使用默认表情）
    /// - trans_kind_idx: 转场类型名的常量池索引（NONE_POOL = 无转场）
    /// - dur_reg: 转场持续时长的寄存器（0xFF = 默认）
    ShowChar {
        char_idx: u16,
        pos: PositionEncoding,
        emotion_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示独立精灵图片
    /// - asset_idx: 图片资源路径的常量池索引
    /// - x_reg / y_reg: 坐标寄存器
    /// - scale_reg / alpha_reg: 缩放和透明度寄存器
    /// - trans_kind_idx / dur_reg: 转场参数
    ShowSprite {
        asset_idx: u16,
        x_reg: u8,
        y_reg: u8,
        scale_reg: u8,
        alpha_reg: u8,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 平滑移动角色立绘到新位置
    /// - char_idx: 角色 ID 的常量池索引
    /// - pos: 目标位置编码
    /// - emotion_idx: 可选的新表情（NONE_POOL = 保持现有表情）
    /// - trans_kind_idx: 移动动画类型
    /// - dur_reg: 移动时长寄存器
    MoveChar {
        char_idx: u16,
        pos: PositionEncoding,
        emotion_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 原地切换角色立绘表情
    /// - char_idx: 角色 ID 的常量池索引
    /// - emotion_idx: 新表情名的常量池索引
    /// - trans_kind_idx: 切换动画类型的常量池索引（NONE_POOL = 无动画）
    /// - dur_reg: 切换时长寄存器（0xFF = 默认）
    Emotion {
        char_idx: u16,
        emotion_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 从舞台上移除角色立绘
    /// - char_idx: 角色 ID 的常量池索引
    /// - trans_kind_idx: 退场转场类型（NONE_POOL = 无转场）
    /// - dur_reg: 转场时长寄存器（0xFF = 默认）
    HideChar {
        char_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 隐藏独立精灵图片
    /// - asset_idx: 图片资源路径的常量池索引
    /// - trans_kind_idx: 退场转场类型（NONE_POOL = 无转场）
    /// - dur_reg: 转场时长寄存器（0xFF = 默认）
    HideSprite {
        asset_idx: u16,
        trans_kind_idx: u16,
        dur_reg: u8,
    },

    /// 显示角色对话（含语音）
    /// - speaker_idx: 说话者名称的常量池索引
    /// - text_idx: 对话文本的常量池索引
    /// - voice_idx: 语音文件 ID 的常量池索引（NONE_POOL = 无语音）
    Dialogue {
        speaker_idx: u16,
        text_idx: u16,
        voice_idx: u16,
    },

    /// 显示旁白文本（无说话者）
    /// - text_idx: 旁白文本的常量池索引
    Narrate { text_idx: u16 },

    // ─── 交互指令 ───────────────────────────────────────────────────
    /// 显示选择支菜单，等待玩家选择
    /// - prompt_idx: 提示文本的常量池索引
    /// - choices: 选择支列表（内联数据，编码到字节码）
    Menu {
        prompt_idx: u16,
        choices: Vec<ChoiceData>,
    },

    // ─── 控制流指令 ─────────────────────────────────────────────────
    /// 无条件跳转到标签
    /// - target: 目标标签名（用户定义或自动生成）
    Jump { target: String },

    /// 条件跳转：寄存器值为 true 时跳转
    /// - reg: 条件寄存器
    /// - target: 目标标签名
    JumpIf { reg: u8, target: String },

    /// 旗标条件跳转：指定旗标为 true 时跳转
    /// - flag_idx: 旗标名的常量池索引
    /// - target: 目标标签名
    JumpIfFlag { flag_idx: u16, target: String },

    /// 子例程调用：将返回地址压栈后跳转
    /// - target: 目标标签名
    Call { target: String },

    /// 子例程返回：弹出调用栈顶的返回地址并跳转
    Return,

    /// 标签定义：标记跳转目标位置（自身不产生字节码）
    /// - name: 标签名
    Label { name: String },

    /// 跨场景跳转
    /// - scene_idx: 目标场景 ID 的常量池索引
    /// - label_idx: 目标场景内标签的常量池索引（NONE_POOL = 从开头执行）
    Goto { scene_idx: u16, label_idx: u16 },

    // ─── 状态指令 ───────────────────────────────────────────────────
    /// 设置变量值
    /// - name_idx: 变量名的常量池索引
    /// - value_reg: 值所在的寄存器
    SetVar { name_idx: u16, value_reg: u8 },

    /// 设置旗标为 true
    /// - flag_idx: 旗标名的常量池索引
    SetFlag { flag_idx: u16 },

    /// 清除旗标（设为 false）
    /// - flag_idx: 旗标名的常量池索引
    UnsetFlag { flag_idx: u16 },

    /// 切换旗标状态（true↔false）
    /// - flag_idx: 旗标名的常量池索引
    ToggleFlag { flag_idx: u16 },

    // ─── 媒体指令 ───────────────────────────────────────────────────
    /// 播放 / 切换背景音乐
    /// - asset_idx: BGM 资源路径的常量池索引
    /// - fade_reg: 淡入时长寄存器（0xFF = 无淡入 / 默认）
    /// - looping: 是否循环播放
    PlayBgm {
        asset_idx: u16,
        fade_reg: u8,
        looping: bool,
    },

    /// 停止背景音乐
    /// - fade_reg: 淡出时长寄存器（0xFF = 立即停止）
    StopBgm { fade_reg: u8 },

    /// 播放音效（不阻断执行）
    /// - asset_idx: 音效资源路径的常量池索引
    /// - fade_reg: 淡入时长寄存器（0xFF = 无淡入）
    PlaySe { asset_idx: u16, fade_reg: u8 },

    /// 播放语音（通常伴随 Dialogue）
    /// - asset_idx: 语音资源路径的常量池索引
    PlayVoice { asset_idx: u16 },

    /// 触发画面特效
    /// - type_idx: 特效类型标识的常量池索引
    /// - params: 特效参数键值对（内联编码到字节码）
    Effect {
        type_idx: u16,
        params: Vec<(u16, u16)>,
    },

    // ─── 时序指令 ───────────────────────────────────────────────────
    /// 暂停指定毫秒数
    /// - dur_reg: 等待时长（毫秒）的寄存器
    Wait { dur_reg: u8 },

    // ─── 特殊指令 ───────────────────────────────────────────────────
    /// 场景结束 — 总是最后一条指令
    End,
}

// ============================================================================
// 辅助类型
// ============================================================================

/// 立绘位置编码 — IR 中 Position 的扁平化表示。
///
/// `aster_core::Position` 的四种变体映射为：
/// - `Left` → `PositionEncoding::Left`
/// - `Center` → `PositionEncoding::Center`
/// - `Right` → `PositionEncoding::Right`
/// - `Custom(x, y)` → `PositionEncoding::Custom { x_reg, y_reg }`
///
/// 对于 Custom 变体，x 和 y 表达式已预先编译为寄存器操作，
/// `x_reg` 和 `y_reg` 指向持有求值结果的寄存器。
#[derive(Debug, Clone, PartialEq)]
pub enum PositionEncoding {
    /// 左侧 — (0.25, 0.5)
    Left,
    /// 中央 — (0.5, 0.5)
    Center,
    /// 右侧 — (0.75, 0.5)
    Right,
    /// 自定义坐标 — (x_reg, y_reg) 指向持有 f64 值的寄存器
    Custom { x_reg: u8, y_reg: u8 },
}

impl PositionEncoding {
    /// 将 `aster_core::Position` 转换为 IR 位置编码。
    ///
    /// 用于编译器在 SceneNode 编译阶段将 AST 的 Position 转换为 IR 编码。
    /// `compile_expr` 回调用于将 Custom 中的 Expr 编译为寄存器操作。
    ///
    /// # 参数
    /// - `position`：AST 层的位置类型
    /// - `compile_coord`：将坐标 Expr 编译为寄存器码的回调
    ///
    /// # 返回值
    /// - `Ok(PositionEncoding)`：成功编码
    /// - `Err(String)`：寄存器不足等错误
    pub fn from_position<F, E>(position: &Position, compile_coord: &mut F) -> Result<Self, E>
    where
        F: FnMut(&aster_core::Expr) -> Result<u8, E>,
    {
        match position {
            Position::Left => Ok(PositionEncoding::Left),
            Position::Center => Ok(PositionEncoding::Center),
            Position::Right => Ok(PositionEncoding::Right),
            Position::Custom(x_expr, y_expr) => {
                let x_reg = compile_coord(x_expr)?;
                let y_reg = compile_coord(y_expr)?;
                Ok(PositionEncoding::Custom { x_reg, y_reg })
            }
        }
    }

    /// 返回位置编码的 u8 表示（用于字节码操作数）。
    ///
    /// - `0x00` = Left
    /// - `0x01` = Center
    /// - `0x02` = Right
    /// - `0x03` = Custom
    pub fn to_byte(&self) -> u8 {
        match self {
            PositionEncoding::Left => 0x00,
            PositionEncoding::Center => 0x01,
            PositionEncoding::Right => 0x02,
            PositionEncoding::Custom { .. } => 0x03,
        }
    }
}

/// 选择支数据 — IR 中单个选项的表示。
///
/// 对应 `aster_core::Choice`，但所有字段已池化：
/// - `text_idx`: 选项显示文本的常量池索引
/// - `target`: 选中后跳转的目标标签名
/// - `condition_flag_idx`: 条件选项的旗标常量池索引（NONE_POOL = 无条件）
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceData {
    /// 选项显示文本的常量池索引
    pub text_idx: u16,
    /// 选中后跳转的目标标签名
    pub target: String,
    /// 条件选项关联的旗标名常量池索引（NONE_POOL = 无条件选项）
    /// VM 在显示选项前检查此旗标，为 false 时隐藏/禁用该选项
    pub condition_flag_idx: u16,
}

// ============================================================================
// RegisterAllocator — 寄存器分配器
// ============================================================================

/// 简单线性寄存器分配器。
///
/// 使用线性扫描策略（r0→r15），不跟踪寄存器生命周期。
/// 每次 SceneNode 编译前调用 `reset()` 回收所有寄存器。
/// PH1-T12 的窥孔优化会清理由此产生的冗余分配。
///
/// # 使用示例
/// ```
/// use aster_compiler::ir::RegisterAllocator;
///
/// let mut regs = RegisterAllocator::new();
/// let r0 = regs.allocate().expect("寄存器可用");
/// let r1 = regs.allocate().expect("寄存器可用");
/// assert_eq!(r0, 0);
/// assert_eq!(r1, 1);
/// regs.reset();
/// assert_eq!(regs.used_count(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct RegisterAllocator {
    /// 下一个可用寄存器索引（0-16，16 表示已耗尽）
    next: u8,
}

impl RegisterAllocator {
    /// 创建一个新的寄存器分配器（初始状态：r0 可用）。
    pub fn new() -> Self {
        RegisterAllocator { next: 0 }
    }

    /// 分配一个寄存器，返回其索引。
    ///
    /// # 返回值
    /// - `Some(reg)`：成功分配（0-15）
    /// - `None`：寄存器已耗尽（>16 个并发使用）
    pub fn allocate(&mut self) -> Option<u8> {
        if self.next < MAX_REGISTERS {
            let reg = self.next;
            self.next += 1;
            Some(reg)
        } else {
            None
        }
    }

    /// 重置分配器，回收所有寄存器。
    ///
    /// 在每个 SceneNode 编译前调用。
    pub fn reset(&mut self) {
        self.next = 0;
    }

    /// 返回当前已分配的寄存器数量。
    #[allow(dead_code)]
    pub fn used_count(&self) -> u8 {
        self.next
    }
}

impl Default for RegisterAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Display 实现 — 调试用
// ============================================================================

impl fmt::Display for IrInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrInstruction::PushStr { reg, str_idx } => {
                write!(f, "PUSH_STR r{}, pool[{}]", reg, str_idx)
            }
            IrInstruction::PushInt { reg, value } => {
                write!(f, "PUSH_INT r{}, {}", reg, value)
            }
            IrInstruction::PushFloat { reg, value } => {
                write!(f, "PUSH_FLOAT r{}, {}", reg, value)
            }
            IrInstruction::PushBool { reg, value } => {
                write!(f, "PUSH_BOOL r{}, {}", reg, value)
            }
            IrInstruction::LoadVar { dst, name_idx } => {
                write!(f, "LOAD_VAR r{}, pool[{}]", dst, name_idx)
            }
            IrInstruction::StoreVar { name_idx, src } => {
                write!(f, "STORE_VAR pool[{}], r{}", name_idx, src)
            }
            IrInstruction::CheckFlag { dst, flag_idx } => {
                write!(f, "CHECK_FLAG r{}, pool[{}]", dst, flag_idx)
            }
            IrInstruction::Add { dst, left, right } => {
                write!(f, "ADD r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Sub { dst, left, right } => {
                write!(f, "SUB r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Mul { dst, left, right } => {
                write!(f, "MUL r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Div { dst, left, right } => {
                write!(f, "DIV r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Eq { dst, left, right } => {
                write!(f, "EQ r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Neq { dst, left, right } => {
                write!(f, "NEQ r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Lt { dst, left, right } => {
                write!(f, "LT r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Gt { dst, left, right } => {
                write!(f, "GT r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Le { dst, left, right } => {
                write!(f, "LE r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Ge { dst, left, right } => {
                write!(f, "GE r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::And { dst, left, right } => {
                write!(f, "AND r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Or { dst, left, right } => {
                write!(f, "OR r{}, r{}, r{}", dst, left, right)
            }
            IrInstruction::Not { dst, src } => {
                write!(f, "NOT r{}, r{}", dst, src)
            }
            IrInstruction::Neg { dst, src } => {
                write!(f, "NEG r{}, r{}", dst, src)
            }
            IrInstruction::Bg {
                asset_idx,
                trans_kind_idx,
                dur_reg,
            } => {
                write!(
                    f,
                    "BG pool[{}], pool[{}], r{}",
                    asset_idx, trans_kind_idx, dur_reg
                )
            }
            IrInstruction::ShowChar {
                char_idx,
                pos,
                emotion_idx,
                trans_kind_idx,
                dur_reg,
            } => {
                write!(
                    f,
                    "SHOW_CHAR pool[{}], {:?}, pool[{}], pool[{}], r{}",
                    char_idx, pos, emotion_idx, trans_kind_idx, dur_reg
                )
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
                write!(
                    f,
                    "SHOW_SPRITE pool[{}], r{}@x, r{}@y, r{}@scl, r{}@alp, pool[{}], r{}",
                    asset_idx, x_reg, y_reg, scale_reg, alpha_reg, trans_kind_idx, dur_reg
                )
            }
            IrInstruction::MoveChar {
                char_idx,
                pos,
                emotion_idx,
                trans_kind_idx,
                dur_reg,
            } => {
                write!(
                    f,
                    "MOVE_CHAR pool[{}], {:?}, pool[{}], pool[{}], r{}",
                    char_idx, pos, emotion_idx, trans_kind_idx, dur_reg
                )
            }
            IrInstruction::Emotion {
                char_idx,
                emotion_idx,
                trans_kind_idx,
                dur_reg,
            } => {
                write!(
                    f,
                    "EMOTION pool[{}], pool[{}], pool[{}], r{}",
                    char_idx, emotion_idx, trans_kind_idx, dur_reg
                )
            }
            IrInstruction::HideChar {
                char_idx,
                trans_kind_idx,
                dur_reg,
            } => {
                write!(
                    f,
                    "HIDE_CHAR pool[{}], pool[{}], r{}",
                    char_idx, trans_kind_idx, dur_reg
                )
            }
            IrInstruction::HideSprite {
                asset_idx,
                trans_kind_idx,
                dur_reg,
            } => {
                write!(
                    f,
                    "HIDE_SPRITE pool[{}], pool[{}], r{}",
                    asset_idx, trans_kind_idx, dur_reg
                )
            }
            IrInstruction::Dialogue {
                speaker_idx,
                text_idx,
                voice_idx,
            } => {
                write!(
                    f,
                    "DIALOGUE pool[{}], pool[{}], pool[{}]",
                    speaker_idx, text_idx, voice_idx
                )
            }
            IrInstruction::Narrate { text_idx } => {
                write!(f, "NARRATE pool[{}]", text_idx)
            }
            IrInstruction::Menu {
                prompt_idx,
                choices,
            } => {
                write!(f, "MENU pool[{}], {} choices", prompt_idx, choices.len())
            }
            IrInstruction::Jump { target } => {
                write!(f, "JUMP {}", target)
            }
            IrInstruction::JumpIf { reg, target } => {
                write!(f, "JUMP_IF r{}, {}", reg, target)
            }
            IrInstruction::JumpIfFlag { flag_idx, target } => {
                write!(f, "JUMP_IF_FLAG pool[{}], {}", flag_idx, target)
            }
            IrInstruction::Call { target } => {
                write!(f, "CALL {}", target)
            }
            IrInstruction::Return => {
                write!(f, "RETURN")
            }
            IrInstruction::Label { name } => {
                write!(f, "LABEL {}", name)
            }
            IrInstruction::Goto {
                scene_idx,
                label_idx,
            } => {
                write!(f, "GOTO pool[{}], pool[{}]", scene_idx, label_idx)
            }
            IrInstruction::SetVar {
                name_idx,
                value_reg,
            } => {
                write!(f, "SET_VAR pool[{}], r{}", name_idx, value_reg)
            }
            IrInstruction::SetFlag { flag_idx } => {
                write!(f, "SET_FLAG pool[{}]", flag_idx)
            }
            IrInstruction::UnsetFlag { flag_idx } => {
                write!(f, "UNSET_FLAG pool[{}]", flag_idx)
            }
            IrInstruction::ToggleFlag { flag_idx } => {
                write!(f, "TOGGLE_FLAG pool[{}]", flag_idx)
            }
            IrInstruction::PlayBgm {
                asset_idx,
                fade_reg,
                looping,
            } => {
                write!(
                    f,
                    "PLAY_BGM pool[{}], r{}, loop={}",
                    asset_idx, fade_reg, looping
                )
            }
            IrInstruction::StopBgm { fade_reg } => {
                write!(f, "STOP_BGM r{}", fade_reg)
            }
            IrInstruction::PlaySe {
                asset_idx,
                fade_reg,
            } => {
                write!(f, "PLAY_SE pool[{}], r{}", asset_idx, fade_reg)
            }
            IrInstruction::PlayVoice { asset_idx } => {
                write!(f, "PLAY_VOICE pool[{}]", asset_idx)
            }
            IrInstruction::Effect { type_idx, params } => {
                write!(f, "EFFECT pool[{}], {} params", type_idx, params.len())
            }
            IrInstruction::Wait { dur_reg } => {
                write!(f, "WAIT r{}", dur_reg)
            }
            IrInstruction::End => {
                write!(f, "END")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 IrInstruction 所有变体可以构造（编译期检查完整性）。
    #[test]
    fn ir_instruction_all_variants_constructible() {
        // 数据传送
        let _ = IrInstruction::PushStr { reg: 0, str_idx: 0 };
        let _ = IrInstruction::PushInt { reg: 0, value: 42 };
        let _ = IrInstruction::PushFloat { reg: 1, value: 1.5 };
        let _ = IrInstruction::PushBool {
            reg: 2,
            value: true,
        };
        let _ = IrInstruction::LoadVar {
            dst: 3,
            name_idx: 0,
        };
        let _ = IrInstruction::StoreVar {
            name_idx: 1,
            src: 4,
        };
        let _ = IrInstruction::CheckFlag {
            dst: 5,
            flag_idx: 2,
        };

        // 算术
        let _ = IrInstruction::Add {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Sub {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Mul {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Div {
            dst: 0,
            left: 1,
            right: 2,
        };

        // 比较
        let _ = IrInstruction::Eq {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Neq {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Lt {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Gt {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Le {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Ge {
            dst: 0,
            left: 1,
            right: 2,
        };

        // 逻辑
        let _ = IrInstruction::And {
            dst: 0,
            left: 1,
            right: 2,
        };
        let _ = IrInstruction::Or {
            dst: 0,
            left: 1,
            right: 2,
        };

        // 一元
        let _ = IrInstruction::Not { dst: 0, src: 1 };
        let _ = IrInstruction::Neg { dst: 0, src: 1 };

        // 渲染
        let _ = IrInstruction::Bg {
            asset_idx: 0,
            trans_kind_idx: NONE_POOL,
            dur_reg: NONE_REG,
        };
        let _ = IrInstruction::ShowChar {
            char_idx: 0,
            pos: PositionEncoding::Center,
            emotion_idx: 1,
            trans_kind_idx: NONE_POOL,
            dur_reg: NONE_REG,
        };
        let _ = IrInstruction::ShowSprite {
            asset_idx: 0,
            x_reg: 1,
            y_reg: 2,
            scale_reg: 3,
            alpha_reg: 4,
            trans_kind_idx: NONE_POOL,
            dur_reg: NONE_REG,
        };
        let _ = IrInstruction::MoveChar {
            char_idx: 0,
            pos: PositionEncoding::Left,
            emotion_idx: NONE_POOL,
            trans_kind_idx: 1,
            dur_reg: 2,
        };
        let _ = IrInstruction::Emotion {
            char_idx: 0,
            emotion_idx: 1,
            trans_kind_idx: NONE_POOL,
            dur_reg: NONE_REG,
        };
        let _ = IrInstruction::HideChar {
            char_idx: 0,
            trans_kind_idx: 1,
            dur_reg: 2,
        };
        let _ = IrInstruction::HideSprite {
            asset_idx: 0,
            trans_kind_idx: NONE_POOL,
            dur_reg: NONE_REG,
        };
        let _ = IrInstruction::Dialogue {
            speaker_idx: 0,
            text_idx: 1,
            voice_idx: NONE_POOL,
        };
        let _ = IrInstruction::Narrate { text_idx: 0 };

        // 交互
        let _ = IrInstruction::Menu {
            prompt_idx: 0,
            choices: vec![ChoiceData {
                text_idx: 1,
                target: "label".into(),
                condition_flag_idx: NONE_POOL,
            }],
        };

        // 控制流
        let _ = IrInstruction::Jump {
            target: "target".into(),
        };
        let _ = IrInstruction::JumpIf {
            reg: 0,
            target: "t".into(),
        };
        let _ = IrInstruction::JumpIfFlag {
            flag_idx: 0,
            target: "t".into(),
        };
        let _ = IrInstruction::Call {
            target: "sub".into(),
        };
        let _ = IrInstruction::Return;
        let _ = IrInstruction::Label {
            name: "start".into(),
        };
        let _ = IrInstruction::Goto {
            scene_idx: 0,
            label_idx: NONE_POOL,
        };

        // 状态
        let _ = IrInstruction::SetVar {
            name_idx: 0,
            value_reg: 1,
        };
        let _ = IrInstruction::SetFlag { flag_idx: 0 };
        let _ = IrInstruction::UnsetFlag { flag_idx: 1 };
        let _ = IrInstruction::ToggleFlag { flag_idx: 2 };

        // 媒体
        let _ = IrInstruction::PlayBgm {
            asset_idx: 0,
            fade_reg: 1,
            looping: true,
        };
        let _ = IrInstruction::StopBgm { fade_reg: NONE_REG };
        let _ = IrInstruction::PlaySe {
            asset_idx: 0,
            fade_reg: NONE_REG,
        };
        let _ = IrInstruction::PlayVoice { asset_idx: 0 };
        let _ = IrInstruction::Effect {
            type_idx: 0,
            params: vec![],
        };

        // 时序
        let _ = IrInstruction::Wait { dur_reg: 0 };

        // 特殊
        let _ = IrInstruction::End;
    }

    /// 验证 RegisterAllocator 的基本分配/回收行为。
    #[test]
    fn register_allocator_linear() {
        let mut regs = RegisterAllocator::new();
        assert_eq!(regs.allocate(), Some(0));
        assert_eq!(regs.allocate(), Some(1));
        assert_eq!(regs.allocate(), Some(2));
        assert_eq!(regs.used_count(), 3);

        // 重置后从 0 开始
        regs.reset();
        assert_eq!(regs.allocate(), Some(0));
        assert_eq!(regs.used_count(), 1);
    }

    /// 验证寄存器耗尽时的行为。
    #[test]
    fn register_allocator_exhaustion() {
        let mut regs = RegisterAllocator::new();
        // 分配全部 16 个寄存器
        for i in 0..16 {
            assert_eq!(regs.allocate(), Some(i));
        }
        // 第 17 次分配应返回 None
        assert_eq!(regs.allocate(), None);
        assert_eq!(regs.used_count(), 16);
    }

    /// 验证 PositionEncoding::to_byte() 编码。
    #[test]
    fn position_encoding_to_byte() {
        assert_eq!(PositionEncoding::Left.to_byte(), 0x00);
        assert_eq!(PositionEncoding::Center.to_byte(), 0x01);
        assert_eq!(PositionEncoding::Right.to_byte(), 0x02);
        assert_eq!(
            PositionEncoding::Custom { x_reg: 1, y_reg: 2 }.to_byte(),
            0x03
        );
    }

    /// 验证哨兵常量值。
    #[test]
    fn sentinel_values() {
        assert_eq!(NONE_REG, 0xFF);
        assert_eq!(NONE_POOL, 0xFFFF);
        assert_eq!(MAX_REGISTERS, 16);
    }
}
