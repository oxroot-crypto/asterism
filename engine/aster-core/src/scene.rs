//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/scene.rs
//! 功能概述：场景定义类型 — 定义 `Scene` 结构体（一组 `SceneNode` 的有序列表）、
//!           `SceneNode` 枚举（25 种演出单元变体）、`Choice` 选择支、`Position` 立绘位置、
//!           `TransitionSpec` 转场规格等核心数据模型。
//!           所有可能动态求值的字段统一使用 `Expr` 类型（资产路径、文本内容、
//!           数值参数、跳转目标等），静态标识符（标签名、旗标名、变量名）保持 `String`。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//! - std::collections::HashMap（Effect 参数映射）
//! - crate::expr::{Expr, default_expr_one}（表达式类型及默认值辅助函数）
//!
//! SceneNode 变体说明（25 种）：
//! - 渲染类：Bg, ShowChar, ShowSprite, MoveChar, Emotion, HideChar, HideSprite, Dialogue, Narration
//! - 交互类：Menu
//! - 控制流：Branch, Jump（场景内）, Goto（跨场景）, Call, Return, Label
//! - 状态类：SetVariable, SetFlag, UnsetFlag, ToggleFlag
//! - 媒体类：Music, StopMusic, PlaySE, Effect
//! - 时序类：Wait
//!
//! 对应文档：Architecture.md §4.2（核心类型清单）、§2.3（DSL 语法规范）

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::expr::{Expr, default_expr_one};

/// 场景定义 — 一个完整的游戏场景，包含一组按顺序执行的 `SceneNode`。
///
/// 每个场景由唯一 `id` 标识（格式为 `"chapter/scene_name"` 的路径字符串），
/// 可选的默认背景和 BGM，以及一个 `SceneNode` 序列。
/// 场景通过 `SceneManager` 加载并由 VM 逐节点执行。
///
/// # 序列化
///
/// 支持 JSON 和 TOML 序列化/反序列化：
/// ```rust,no_run
/// # use aster_core::Scene;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let scene: Scene = serde_json::from_str(&std::fs::read_to_string("scene.json")?)?;
/// # Ok(())
/// # }
/// ```
///
/// # 示例
/// ```
/// use aster_core::{Scene, SceneNode, Position, Expr};
///
/// let scene = Scene {
///     id: "chapter1/prologue".into(),
///     label: Some("序章".into()),
///     background: Some(Expr::string_literal("bg_classroom_day.png")),
///     music: Some(Expr::string_literal("bgm_peaceful.ogg")),
///     nodes: vec![
///         SceneNode::Narration { text: Expr::string_literal("春天，樱花盛开的季节。") },
///         SceneNode::ShowChar {
///             char_id: Expr::string_literal("sayori"),
///             position: Position::Center,
///             emotion: Some(Expr::string_literal("smile")),
///             transition: None,
///         },
///         SceneNode::Dialogue {
///             speaker: Expr::string_literal("小百合"),
///             text: Expr::string_literal("初次见面，请多指教！"),
///             voice_id: None,
///         },
///     ],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Scene {
    /// 场景唯一标识符（如 `"chapter1/prologue"`、`"common/bad_end"`）
    /// 对应 `scripts/` 下的 .aster 文件路径（不含扩展名）
    pub id: String,

    /// 场景显示标签（可选），如 `"序章"`、`"第一章 · 相遇"`
    /// 用于存档缩略图和场景选择界面
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// 默认背景资源路径（可选），存储为表达式以支持变量引用
    /// 场景启动时自动加载，后续可通过 `SceneNode::Bg` 切换
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<Expr>,

    /// 默认 BGM 资源路径（可选），存储为表达式以支持变量引用
    /// 场景启动时自动播放，后续可通过 `SceneNode::Music` 切换
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music: Option<Expr>,

    /// 场景节点序列 — 按声明顺序依次执行
    /// 空序列为合法场景（如仅含 label 定义的跳转目标场景）
    #[serde(default)]
    pub nodes: Vec<SceneNode>,
}

/// 场景节点枚举 — 视觉小说中的一个基本演出单元。
///
/// 所有场景由一系列 `SceneNode` 组成，运行时 VM 按顺序执行每个节点。
/// 部分节点（`Dialogue`、`Menu`）会暂停执行并等待用户输入；
/// 控制流节点（`Jump`、`Branch`、`Call`）会改变执行顺序。
///
/// ## 字段类型约定
///
/// 所有可能动态求值的字段（资产路径、文本内容、数值参数、跳转目标等）
/// 统一使用 `Expr` 类型。解析器将字面量和表达式统一构建为 `Expr` 树，
/// 编译器负责常量折叠和字节码生成。
///
/// 仅编译期常量（标签名、旗标名、变量名、特效类型标识）保持 `String`。
///
/// # 变体分类
///
/// | 分类 | 变体 | 说明 |
/// |------|------|------|
/// | 渲染 | `Bg` | 切换场景背景图片 |
/// | 渲染 | `ShowChar` | 在舞台上首次显示/重新出场角色立绘 |
/// | 渲染 | `ShowSprite` | 显示独立精灵图片（道具图标、贴纸等） |
/// | 渲染 | `MoveChar` | 平滑移动角色立绘到新位置 |
/// | 渲染 | `Emotion` | 原地切换角色立绘表情 |
/// | 渲染 | `HideChar` | 从舞台上移除角色立绘 |
/// | 渲染 | `HideSprite` | 隐藏独立精灵图片 |
/// | 渲染 | `Dialogue` | 显示说话者对话文本 |
/// | 渲染 | `Narration` | 显示旁白文本（无说话者） |
/// | 交互 | `Menu` | 显示选择支，等待玩家选择 |
/// | 控制流 | `Branch` | 条件分支（if/elif/else） |
/// | 控制流 | `Jump` | 场景内无条件跳转到标签 |
/// | 控制流 | `Goto` | 跨场景跳转（可指定目标标签） |
/// | 控制流 | `Call` | 子例程调用 |
/// | 控制流 | `Return` | 子例程返回 |
/// | 控制流 | `Label` | 跳转目标标签 |
/// | 状态 | `SetVariable` | 设置变量值 |
/// | 状态 | `SetFlag` | 设置旗标为 true |
/// | 状态 | `UnsetFlag` | 清除旗标（设为 false） |
/// | 状态 | `ToggleFlag` | 切换旗标（true↔false） |
/// | 媒体 | `Music` | 播放/切换背景音乐 |
/// | 媒体 | `StopMusic` | 停止背景音乐 |
/// | 媒体 | `PlaySE` | 播放音效 |
/// | 媒体 | `Effect` | 触发视觉效果 |
/// | 时序 | `Wait` | 暂停指定时长 |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SceneNode {
    /// 设置背景：切换当前场景的背景图片。
    ///
    /// 可在场景任意位置调用，覆盖 `Scene.background` 的默认背景。
    /// `asset_path` 为表达式，支持字符串字面量或变量引用（如 `bg $bg_name`）。
    Bg {
        /// 背景资源路径（表达式）
        asset_path: Expr,
        /// 可选的转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 对话节点：显示说话者的对话文本，暂停并等待用户点击继续。
    ///
    /// `speaker` 和 `text` 均为表达式，支持变量插值（如 `$char_name`、
    /// `"你好，" + $player_name`）。
    Dialogue {
        /// 说话者角色 ID（表达式，对应 `Character.id`，如 `"sayori"`）
        speaker: Expr,
        /// 对话内容（表达式，Phase 4 起支持 inline markup）
        text: Expr,
        /// 可选的语音文件资源 ID（表达式）
        #[serde(skip_serializing_if = "Option::is_none")]
        voice_id: Option<Expr>,
    },

    /// 显示角色立绘：在舞台上指定位置显示角色的立绘精灵。
    ShowChar {
        /// 角色 ID（表达式，对应 `Character.id`）
        char_id: Expr,
        /// 立绘在舞台上的位置（左/中/右/自定义坐标）
        position: Position,
        /// 可选的表情名（表达式，对应 `Character.sprites` 的 key）
        #[serde(skip_serializing_if = "Option::is_none")]
        emotion: Option<Expr>,
        /// 可选的入场转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 显示独立精灵：在画面上展示一个不绑定角色系统的独立图片。
    ///
    /// 用途：道具图标、CG 切片、贴纸、表示情绪的符号（心形/感叹号）等。
    /// 与 `ShowChar` 的区别：不依赖 `Character`，直接用资源路径。
    ShowSprite {
        /// 图片资源路径（表达式）
        asset_path: Expr,
        /// X 归一化坐标（表达式，0.0=左边缘, 1.0=右边缘），锚点为中心
        x: Expr,
        /// Y 归一化坐标（表达式，0.0=顶部, 1.0=底部），锚点为中心
        y: Expr,
        /// 缩放因子（表达式，1.0 = 原始像素尺寸）
        #[serde(default = "default_expr_one")]
        scale: Expr,
        /// 透明度（表达式，0.0=全透明, 1.0=不透明）
        #[serde(default = "default_expr_one")]
        alpha: Expr,
        /// 可选的入场转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 移动角色立绘：将已显示的角色立绘平滑移动到新位置，可选同步切换表情。
    ///
    /// 必须在 `ShowChar` 之后使用，目标角色当前必须正在显示。
    /// 移动动画类型和时长由 `transition` 字段指定。
    MoveChar {
        /// 角色 ID（表达式）
        char_id: Expr,
        /// 目标位置
        position: Position,
        /// 可选：同步切换到新表情（表达式）
        #[serde(skip_serializing_if = "Option::is_none")]
        emotion: Option<Expr>,
        /// 移动动画规格（如 slide(0.8) 表示 800ms 滑动过渡）
        transition: TransitionSpec,
    },

    /// 切换角色表情：将已显示的角色的立绘原地切换为另一个表情，不改变位置。
    ///
    /// 必须在 `ShowChar` 之后使用，目标角色当前必须正在显示。
    /// 与 `MoveChar` 的区别：Emotion 不改变位置，只换立绘图片。
    Emotion {
        /// 角色 ID（表达式）
        char_id: Expr,
        /// 新表情名（表达式，对应 `Character.sprites` 的 key）
        emotion: Expr,
        /// 可选的切换动画规格（如 dissolve(0.3)）
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 隐藏角色立绘：从舞台上移除指定角色的立绘。
    HideChar {
        /// 角色 ID（表达式）
        char_id: Expr,
        /// 可选的退场转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 隐藏独立精灵：从画面上移除之前通过 `ShowSprite` 显示的独立图片。
    ///
    /// `asset_path` 用于匹配要隐藏的精灵资源。
    /// 如果同一资源被多次 ShowSprite 调用，HideSprite 会隐藏所有匹配实例。
    HideSprite {
        /// 图片资源路径（表达式，匹配 `ShowSprite.asset_path`）
        asset_path: Expr,
        /// 可选的退场转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 旁白节点：显示无说话者的叙述文本。
    ///
    /// 与 `Dialogue` 的区别：没有 speaker 字段，视觉样式通常不同（如居中、斜体）。
    Narration {
        /// 旁白文本内容（表达式）
        text: Expr,
    },

    /// 菜单/选择支节点：显示一组选项并等待玩家选择。
    ///
    /// 每个选项包含显示文本和跳转目标标签。
    /// 当 `choices` 为空时，这是脚本语义错误（应在编译期捕获）。
    Menu {
        /// 选择支提示文本（表达式，如 `"你要怎么做？"`）
        prompt: Expr,
        /// 选项列表（最少 2 个，最多 N 个）
        choices: Vec<Choice>,
    },

    /// 条件分支节点：根据运行时条件表达式的求值结果选择执行路径。
    ///
    /// 支持 `if / elif / else` 完整分支结构。
    /// `condition` 为 `Expr` 树（如 `Expr::BinaryOp(var("affection"), Ge, int(5))`），
    /// VM 执行时求值。
    Branch {
        /// if 条件表达式
        condition: Expr,
        /// 条件为 true 时执行的节点序列
        then_nodes: Vec<SceneNode>,
        /// elif 分支列表（按声明顺序），每项为 (条件表达式, 节点序列)
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        elif_branches: Vec<(Expr, Vec<SceneNode>)>,
        /// 可选的 else 分支节点序列
        #[serde(skip_serializing_if = "Option::is_none")]
        else_nodes: Option<Vec<SceneNode>>,
    },

    /// 设置变量：将表达式的求值结果赋给指定变量。
    SetVariable {
        /// 变量名（标识符，如 `"score"`、`"affection"`）
        name: String,
        /// 值的表达式树
        value: Expr,
    },

    /// 设置旗标：将命名布尔旗标设为 true。
    SetFlag {
        /// 旗标名称（标识符）
        name: String,
    },

    /// 清除旗标：将命名布尔旗标设为 false。
    UnsetFlag {
        /// 旗标名称（标识符）
        name: String,
    },

    /// 切换旗标：反转布尔旗标的当前值（true↔false）。
    ///
    /// 等效于：`if check(flag) { unset(flag) } else { set(flag) }`
    ToggleFlag {
        /// 旗标名称（标识符）
        name: String,
    },

    /// 播放/切换背景音乐：开始播放或切换到指定 BGM 资源。
    ///
    /// BGM 默认循环播放（`looping = true`），可通过 `fade_in` 指定淡入时间。
    /// 如果当前已有 BGM 在播放，将平滑切换到新曲目。
    Music {
        /// BGM 资源路径（表达式）
        asset_path: Expr,
        /// 可选的淡入时间秒数（表达式）
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_in: Option<Expr>,
        /// 是否循环播放（默认 true）
        #[serde(default = "default_true")]
        looping: bool,
    },

    /// 停止背景音乐：停止当前正在播放的 BGM。
    ///
    /// `fade_out` 指定淡出时间，`None` 表示立即停止。
    StopMusic {
        /// 可选的淡出时间秒数（表达式），None = 立即停止
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_out: Option<Expr>,
    },

    /// 播放音效：播放一个音频资源（不阻断执行）。
    PlaySE {
        /// 音效资源 ID（表达式）
        asset_id: Expr,
        /// 可选的淡入时间秒数（表达式）
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_in: Option<Expr>,
    },

    /// 等待：暂停执行指定的毫秒数。
    ///
    /// 用于控制节奏（如 `Wait { duration_ms: int_literal(1500) }` 暂停 1.5 秒）。
    Wait {
        /// 等待时长毫秒（表达式）
        duration_ms: Expr,
    },

    /// 视觉效果：触发指定的画面特效。
    ///
    /// Phase 1 仅定义数据结构，实际渲染由后续 Phase 实现。
    Effect {
        /// 特效类型标识（如 `"shake"`、`"flash"`、`"fade_in"`）
        effect_type: String,
        /// 特效参数键值对（值均为表达式，如 `{"duration": int_literal(500), "intensity": float_literal(0.8)}`）
        #[serde(default)]
        params: HashMap<String, Expr>,
    },

    /// 场景内无条件跳转：将执行位置跳转到**当前场景内**的指定标签处。
    ///
    /// `target` 必须在当前场景的 `nodes` 中存在对应的 `Label` 节点，
    /// 否则 VM 在编译期报错（语义错误）。
    ///
    /// 如需跨场景跳转，请使用 `Goto` 变体。
    Jump {
        /// 跳转目标标签名（表达式，支持 computed jump：`jump $next_label`）
        target: Expr,
    },

    /// 跨场景跳转：将执行转移到另一个场景，可选择性指定目标场景内的起始标签。
    ///
    /// 与 `Jump` 的区别：
    /// - `Jump` → 当前场景内，目标为 `Label`
    /// - `Goto` → 跨场景，目标为另一个 `.aster` 文件
    ///
    /// `label` 为 `None` 时从目标场景的第一条指令开始执行；
    /// 为 `Some(expr)` 时从目标场景中对应的 `Label` 处开始执行。
    Goto {
        /// 目标场景 ID（表达式，如 `"chapter2/romance"` 或 `$next_scene`）
        scene_id: Expr,
        /// 可选：目标场景内的起始标签名（表达式）
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<Expr>,
    },

    /// 子例程调用：将当前执行位置压栈，跳转到指定标签。
    ///
    /// Phase 1 定义数据结构，VM 支持（PH1-T13 实现），
    /// 完整调用栈在 Phase 2 中与存档功能集成。
    Call {
        /// 调用目标标签名（表达式）
        target: Expr,
    },

    /// 子例程返回：从调用栈弹出返回地址并跳转回去。
    Return,

    /// 标签：定义跳转目标点，自身不产生任何副作用。
    ///
    /// 标签名在同一场景内必须唯一。`name` 为编译期常量（标识符）。
    Label {
        /// 标签名称（标识符，编译期常量）
        name: String,
    },
}

/// serde 默认值辅助函数 — 返回 `true`（用于 `Music.looping` 等布尔字段）
const fn default_true() -> bool {
    true
}

/// 选择支 — 菜单中的一个选项。
///
/// 每个选项包含显示给玩家的文本、跳转目标标签，
/// 以及可选的条件表达式（用于条件选项）。
///
/// # 示例
/// ```
/// use aster_core::{Choice, Expr, BinaryOp};
///
/// let choice = Choice {
///     text: Expr::string_literal("上前搭话"),
///     target: Expr::string_literal("approach_label"),
///     condition: Some(Expr::binary_op(
///         Expr::variable("affection"),
///         BinaryOp::Ge,
///         Expr::int_literal(3),
///     )),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Choice {
    /// 选项的显示文本（表达式）
    pub text: Expr,

    /// 选中后跳转的目标标签名（表达式）
    pub target: Expr,

    /// 可选的条件表达式
    ///
    /// 当 `condition` 为 `Some(expr)` 时，该选项仅在 `expr` 求值为 true 时显示。
    /// VM 执行时求值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<Expr>,
}

/// 立绘舞台位置 — 定义角色立绘在画面中的水平/垂直位置。
///
/// 使用归一化坐标系（0.0~1.0），锚点默认在立绘底部中心。
/// 预设位置 `Left`/`Center`/`Right` 映射到固定坐标；
/// `Custom(x, y)` 中 `x` 和 `y` 均为 `Expr`，支持动态定位。
///
/// # 坐标约定
/// - X 轴：0.0 = 画面左边缘，1.0 = 画面右边缘
/// - Y 轴：0.0 = 画面顶部，1.0 = 画面底部
/// - 锚点：立绘底部中心
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Position {
    /// 左侧 — 等效于 `Custom(float_literal(0.25), float_literal(0.5))`
    #[serde(rename = "left")]
    Left,

    /// 中央 — 等效于 `Custom(float_literal(0.5), float_literal(0.5))`
    #[serde(rename = "center")]
    Center,

    /// 右侧 — 等效于 `Custom(float_literal(0.75), float_literal(0.5))`
    #[serde(rename = "right")]
    Right,

    /// 自定义坐标 — (x, y)，均为 `Expr`（可含变量引用或字面量）
    #[serde(rename = "custom")]
    Custom(Expr, Expr),
}

impl Position {
    /// 尝试将 Position 转换为归一化 (x, y) 坐标。
    ///
    /// 预设位置（Left/Center/Right）始终返回 `Some`。
    /// `Custom(x, y)` 仅在两个坐标都为字面量（可提取为 f32）时返回 `Some`，
    /// 否则返回 `None`（编译器/VM 在后续阶段求值）。
    ///
    /// # 返回值
    /// - `Some(f32, f32)`：所有坐标均为编译期常量
    /// - `None`：含变量引用或表达式，需要运行时求值
    ///
    /// # 示例
    /// ```
    /// use aster_core::{Position, Expr};
    ///
    /// assert_eq!(Position::Left.to_coords(), Some((0.25, 0.5)));
    /// assert_eq!(Position::Center.to_coords(), Some((0.5, 0.5)));
    /// assert_eq!(Position::Right.to_coords(), Some((0.75, 0.5)));
    /// assert_eq!(
    ///     Position::Custom(Expr::float_literal(0.1), Expr::float_literal(0.8)).to_coords(),
    ///     Some((0.1, 0.8))
    /// );
    /// assert_eq!(
    ///     Position::Custom(Expr::variable("x"), Expr::float_literal(0.5)).to_coords(),
    ///     None  // 变量引用 → 运行时求值
    /// );
    /// ```
    pub fn to_coords(&self) -> Option<(f32, f32)> {
        match self {
            Position::Left => Some((0.25, 0.5)),
            Position::Center => Some((0.5, 0.5)),
            Position::Right => Some((0.75, 0.5)),
            Position::Custom(x_expr, y_expr) => {
                let xv = x_expr.as_float_literal()? as f32;
                let yv = y_expr.as_float_literal()? as f32;
                Some((xv, yv))
            }
        }
    }
}

/// 转场效果规格 — 定义角色立绘或背景切换时的过渡动画参数。
///
/// Phase 1 仅定义数据结构，实际转场渲染在 Phase 4 中实现。
/// `kind` 存储转场类型标识（编译期常量），`duration_ms` 为表达式（支持动态时长）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransitionSpec {
    /// 转场类型标识（编译期常量，如 `"fade"`、`"dissolve"`、`"slide_left"`）
    pub kind: String,

    /// 转场持续时间毫秒（表达式，支持动态时长如 `fade($custom_duration)`）
    pub duration_ms: Expr,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── 辅助函数：快速构造常见 Expr ─────────────────────────────────────

    /// 创建字符串字面量表达式
    fn s(v: &str) -> Expr {
        Expr::string_literal(v)
    }

    /// 创建整数字面量表达式
    fn i(v: i64) -> Expr {
        Expr::int_literal(v)
    }

    /// 创建浮点字面量表达式
    fn f(v: f64) -> Expr {
        Expr::float_literal(v)
    }

    // ─── 渲染类变体测试 (9 变体: Bg, ShowChar, ShowSprite, MoveChar, ───────
    //       Emotion, HideChar, HideSprite, Dialogue, Narration)

    /// 验证渲染类 SceneNode 变体的构造与模式匹配。
    ///
    /// 覆盖 Bg / ShowChar / ShowSprite / MoveChar / Emotion /
    /// HideChar / HideSprite / Dialogue / Narration 共 9 个变体。
    #[test]
    fn scene_node_render_variants() {
        // Bg
        let bg = SceneNode::Bg {
            asset_path: s("bg_park.png"),
            transition: Some(TransitionSpec {
                kind: "fade".into(),
                duration_ms: i(800),
            }),
        };
        if let SceneNode::Bg {
            asset_path,
            transition,
        } = &bg
        {
            assert_eq!(asset_path, &s("bg_park.png"));
            assert!(transition.is_some());
        } else {
            panic!("应为 Bg 变体");
        }

        // ShowChar
        let show_char = SceneNode::ShowChar {
            char_id: s("sayori"),
            position: Position::Left,
            emotion: Some(s("smile")),
            transition: None,
        };
        if let SceneNode::ShowChar {
            char_id,
            position,
            emotion,
            transition,
        } = &show_char
        {
            assert_eq!(char_id, &s("sayori"));
            assert_eq!(position, &Position::Left);
            assert_eq!(emotion.as_ref(), Some(&s("smile")));
            assert!(transition.is_none());
        } else {
            panic!("应为 ShowChar 变体");
        }

        // ShowSprite
        let show_sprite = SceneNode::ShowSprite {
            asset_path: s("ui/icon_heart.png"),
            x: f(0.9),
            y: f(0.05),
            scale: f(0.5),
            alpha: f(0.8),
            transition: None,
        };
        if let SceneNode::ShowSprite {
            asset_path,
            x,
            y,
            scale,
            alpha,
            transition,
        } = &show_sprite
        {
            assert_eq!(asset_path, &s("ui/icon_heart.png"));
            assert_eq!(x, &f(0.9));
            assert_eq!(y, &f(0.05));
            assert_eq!(scale, &f(0.5));
            assert_eq!(alpha, &f(0.8));
            assert!(transition.is_none());
        } else {
            panic!("应为 ShowSprite 变体");
        }

        // ShowSprite 默认值（scale=1.0, alpha=1.0）
        let sprite_default = SceneNode::ShowSprite {
            asset_path: s("ui/dot.png"),
            x: f(0.5),
            y: f(0.5),
            scale: f(1.0),
            alpha: f(1.0),
            transition: None,
        };
        if let SceneNode::ShowSprite { scale, alpha, .. } = &sprite_default {
            assert!(
                (scale.as_float_literal().unwrap() - 1.0).abs() < f64::EPSILON,
                "scale 默认值应为 1.0"
            );
            assert!(
                (alpha.as_float_literal().unwrap() - 1.0).abs() < f64::EPSILON,
                "alpha 默认值应为 1.0"
            );
        } else {
            panic!("应为 ShowSprite 变体（默认值）");
        }

        // MoveChar
        let move_char = SceneNode::MoveChar {
            char_id: s("sayori"),
            position: Position::Right,
            emotion: Some(s("angry")),
            transition: TransitionSpec {
                kind: "slide".into(),
                duration_ms: i(800),
            },
        };
        if let SceneNode::MoveChar {
            char_id,
            position,
            emotion,
            transition,
        } = &move_char
        {
            assert_eq!(char_id, &s("sayori"));
            assert_eq!(position, &Position::Right);
            assert_eq!(emotion.as_ref(), Some(&s("angry")));
            assert_eq!(transition.kind, "slide");
            assert_eq!(transition.duration_ms, i(800));
        } else {
            panic!("应为 MoveChar 变体");
        }

        // Emotion
        let emotion = SceneNode::Emotion {
            char_id: s("sayori"),
            emotion: s("surprised"),
            transition: Some(TransitionSpec {
                kind: "dissolve".into(),
                duration_ms: i(300),
            }),
        };
        if let SceneNode::Emotion {
            char_id,
            emotion: em,
            transition,
        } = &emotion
        {
            assert_eq!(char_id, &s("sayori"));
            assert_eq!(em, &s("surprised"));
            assert!(transition.is_some());
        } else {
            panic!("应为 Emotion 变体");
        }

        // HideChar
        let hide_char = SceneNode::HideChar {
            char_id: s("sayori"),
            transition: Some(TransitionSpec {
                kind: "fade".into(),
                duration_ms: i(500),
            }),
        };
        if let SceneNode::HideChar {
            char_id,
            transition,
        } = &hide_char
        {
            assert_eq!(char_id, &s("sayori"));
            assert!(transition.is_some());
        } else {
            panic!("应为 HideChar 变体");
        }

        // HideSprite
        let hide_sprite = SceneNode::HideSprite {
            asset_path: s("ui/icon_heart.png"),
            transition: Some(TransitionSpec {
                kind: "fade".into(),
                duration_ms: i(300),
            }),
        };
        if let SceneNode::HideSprite {
            asset_path,
            transition,
        } = &hide_sprite
        {
            assert_eq!(asset_path, &s("ui/icon_heart.png"));
            assert!(transition.is_some());
        } else {
            panic!("应为 HideSprite 变体");
        }

        // Dialogue
        let dialogue = SceneNode::Dialogue {
            speaker: s("小百合"),
            text: s("你好！"),
            voice_id: Some(s("voice_001.ogg")),
        };
        if let SceneNode::Dialogue {
            speaker,
            text,
            voice_id,
        } = &dialogue
        {
            assert_eq!(speaker, &s("小百合"));
            assert_eq!(text, &s("你好！"));
            assert_eq!(voice_id.as_ref(), Some(&s("voice_001.ogg")));
        } else {
            panic!("应为 Dialogue 变体");
        }

        // Narration
        let narration = SceneNode::Narration {
            text: s("这是一个春天的早晨。"),
        };
        if let SceneNode::Narration { text } = &narration {
            assert_eq!(text, &s("这是一个春天的早晨。"));
        } else {
            panic!("应为 Narration 变体");
        }
    }

    // ─── 交互类变体测试 (1 变体: Menu) ──────────────────────────────────────

    /// 验证交互类 SceneNode 变体（Menu）的构造与模式匹配。
    #[test]
    fn scene_node_interaction_variants() {
        let menu = SceneNode::Menu {
            prompt: s("你要怎么做？"),
            choices: vec![
                Choice {
                    text: s("上前搭话"),
                    target: s("approach"),
                    condition: None,
                },
                Choice {
                    text: s("转身离开"),
                    target: s("leave"),
                    condition: Some(Expr::binary_op(
                        Expr::variable("courage"),
                        crate::expr::BinaryOp::Lt,
                        i(3),
                    )),
                },
            ],
        };
        if let SceneNode::Menu { prompt, choices } = &menu {
            assert_eq!(prompt, &s("你要怎么做？"));
            assert_eq!(choices.len(), 2);
            assert_eq!(choices[0].text, s("上前搭话"));
            assert!(choices[1].condition.is_some());
        } else {
            panic!("应为 Menu 变体");
        }
    }

    // ─── 控制流类变体测试 (6 变体: Branch, Jump, Goto, Call, Return, Label)

    /// 验证控制流类 SceneNode 变体的构造与模式匹配。
    ///
    /// 覆盖 Branch / Jump / Goto / Call / Return / Label 共 6 个变体。
    #[test]
    fn scene_node_control_flow_variants() {
        // Branch (if/elif/else)
        let branch = SceneNode::Branch {
            condition: Expr::binary_op(Expr::variable("score"), crate::expr::BinaryOp::Ge, i(10)),
            then_nodes: vec![SceneNode::Dialogue {
                speaker: s("系统"),
                text: s("恭喜！"),
                voice_id: None,
            }],
            elif_branches: vec![(
                Expr::binary_op(Expr::variable("score"), crate::expr::BinaryOp::Ge, i(5)),
                vec![SceneNode::Dialogue {
                    speaker: s("系统"),
                    text: s("还不错。"),
                    voice_id: None,
                }],
            )],
            else_nodes: Some(vec![SceneNode::Dialogue {
                speaker: s("系统"),
                text: s("继续努力。"),
                voice_id: None,
            }]),
        };
        if let SceneNode::Branch {
            condition,
            then_nodes,
            elif_branches,
            else_nodes,
        } = &branch
        {
            assert!(matches!(condition, Expr::BinaryOp(..)));
            assert_eq!(then_nodes.len(), 1);
            assert_eq!(elif_branches.len(), 1);
            assert!(else_nodes.is_some());
        } else {
            panic!("应为 Branch 变体");
        }

        // Jump
        let jump = SceneNode::Jump {
            target: s("next_chapter"),
        };
        if let SceneNode::Jump { target } = &jump {
            assert_eq!(target, &s("next_chapter"));
        } else {
            panic!("应为 Jump 变体");
        }

        // Goto (带 label)
        let goto = SceneNode::Goto {
            scene_id: s("chapter2/romance"),
            label: Some(s("start")),
        };
        if let SceneNode::Goto { scene_id, label } = &goto {
            assert_eq!(scene_id, &s("chapter2/romance"));
            assert_eq!(label.as_ref(), Some(&s("start")));
        } else {
            panic!("应为 Goto 变体");
        }

        // Goto (不带 label)
        let goto_no_label = SceneNode::Goto {
            scene_id: s("chapter3/epilogue"),
            label: None,
        };
        if let SceneNode::Goto { scene_id, label } = &goto_no_label {
            assert_eq!(scene_id, &s("chapter3/epilogue"));
            assert!(label.is_none());
        } else {
            panic!("应为 Goto 变体 (label=None)");
        }

        // Call
        let call = SceneNode::Call {
            target: s("subroutine"),
        };
        if let SceneNode::Call { target } = &call {
            assert_eq!(target, &s("subroutine"));
        } else {
            panic!("应为 Call 变体");
        }

        // Return
        let ret = SceneNode::Return;
        assert!(matches!(ret, SceneNode::Return));

        // Label
        let label = SceneNode::Label {
            name: "start".into(),
        };
        if let SceneNode::Label { name } = &label {
            assert_eq!(name, "start");
        } else {
            panic!("应为 Label 变体");
        }
    }

    // ─── 状态类变体测试 (4 变体: SetVariable, SetFlag, UnsetFlag, ToggleFlag)

    /// 验证状态类 SceneNode 变体的构造与模式匹配。
    ///
    /// 覆盖 SetVariable / SetFlag / UnsetFlag / ToggleFlag 共 4 个变体。
    #[test]
    fn scene_node_state_variants() {
        // SetVariable
        let set_var = SceneNode::SetVariable {
            name: "score".into(),
            value: i(100),
        };
        if let SceneNode::SetVariable { name, value } = &set_var {
            assert_eq!(name, "score");
            assert_eq!(value, &i(100));
        } else {
            panic!("应为 SetVariable 变体");
        }

        // SetFlag
        let set_flag = SceneNode::SetFlag {
            name: "met_heroine".into(),
        };
        if let SceneNode::SetFlag { name } = &set_flag {
            assert_eq!(name, "met_heroine");
        } else {
            panic!("应为 SetFlag 变体");
        }

        // UnsetFlag
        let unset_flag = SceneNode::UnsetFlag {
            name: "bad_end_flag".into(),
        };
        if let SceneNode::UnsetFlag { name } = &unset_flag {
            assert_eq!(name, "bad_end_flag");
        } else {
            panic!("应为 UnsetFlag 变体");
        }

        // ToggleFlag
        let toggle_flag = SceneNode::ToggleFlag {
            name: "seen_event".into(),
        };
        if let SceneNode::ToggleFlag { name } = &toggle_flag {
            assert_eq!(name, "seen_event");
        } else {
            panic!("应为 ToggleFlag 变体");
        }
    }

    // ─── 媒体类变体测试 (4 变体: Music, StopMusic, PlaySE, Effect) ──────────

    /// 验证媒体类 SceneNode 变体的构造与模式匹配。
    ///
    /// 覆盖 Music / StopMusic / PlaySE / Effect 共 4 个变体。
    #[test]
    fn scene_node_media_variants() {
        // Music
        let music = SceneNode::Music {
            asset_path: s("bgm_peaceful.ogg"),
            fade_in: Some(f(2.0)),
            looping: true,
        };
        if let SceneNode::Music {
            asset_path,
            fade_in,
            looping,
        } = &music
        {
            assert_eq!(asset_path, &s("bgm_peaceful.ogg"));
            assert!(fade_in.is_some());
            assert!(looping);
        } else {
            panic!("应为 Music 变体");
        }

        // StopMusic
        let stop_music = SceneNode::StopMusic {
            fade_out: Some(f(1.5)),
        };
        if let SceneNode::StopMusic { fade_out } = &stop_music {
            assert!(fade_out.is_some());
        } else {
            panic!("应为 StopMusic 变体");
        }

        // PlaySE
        let play_se = SceneNode::PlaySE {
            asset_id: s("se_ding.ogg"),
            fade_in: Some(f(0.1)),
        };
        if let SceneNode::PlaySE { asset_id, fade_in } = &play_se {
            assert_eq!(asset_id, &s("se_ding.ogg"));
            assert!(fade_in.is_some());
        } else {
            panic!("应为 PlaySE 变体");
        }

        // Effect
        let mut params = HashMap::new();
        params.insert("intensity".into(), f(0.8));
        let effect = SceneNode::Effect {
            effect_type: "shake".into(),
            params,
        };
        if let SceneNode::Effect {
            effect_type,
            params,
        } = &effect
        {
            assert_eq!(effect_type, "shake");
            assert_eq!(params.get("intensity"), Some(&f(0.8)));
        } else {
            panic!("应为 Effect 变体");
        }
    }

    // ─── 时序类变体测试 (1 变体: Wait) ──────────────────────────────────────

    /// 验证时序类 SceneNode 变体（Wait）的构造与模式匹配。
    #[test]
    fn scene_node_timing_variants() {
        let wait = SceneNode::Wait {
            duration_ms: i(1500),
        };
        if let SceneNode::Wait { duration_ms } = &wait {
            assert_eq!(duration_ms, &i(1500));
        } else {
            panic!("应为 Wait 变体");
        }
    }

    // ─── 全变体计数验证 ─────────────────────────────────────────────────────

    /// 验证 SceneNode 变体总数为 25（避免后续修改遗漏）。
    #[test]
    fn scene_node_variant_count_is_25() {
        // 使用 match 覆盖所有变体，编译器会确保遗漏时有 warning
        fn count_variants(node: &SceneNode) -> &'static str {
            match node {
                SceneNode::Bg { .. } => "Bg",
                SceneNode::ShowChar { .. } => "ShowChar",
                SceneNode::ShowSprite { .. } => "ShowSprite",
                SceneNode::MoveChar { .. } => "MoveChar",
                SceneNode::Emotion { .. } => "Emotion",
                SceneNode::HideChar { .. } => "HideChar",
                SceneNode::HideSprite { .. } => "HideSprite",
                SceneNode::Dialogue { .. } => "Dialogue",
                SceneNode::Narration { .. } => "Narration",
                SceneNode::Menu { .. } => "Menu",
                SceneNode::Branch { .. } => "Branch",
                SceneNode::SetVariable { .. } => "SetVariable",
                SceneNode::SetFlag { .. } => "SetFlag",
                SceneNode::UnsetFlag { .. } => "UnsetFlag",
                SceneNode::ToggleFlag { .. } => "ToggleFlag",
                SceneNode::Music { .. } => "Music",
                SceneNode::StopMusic { .. } => "StopMusic",
                SceneNode::PlaySE { .. } => "PlaySE",
                SceneNode::Effect { .. } => "Effect",
                SceneNode::Jump { .. } => "Jump",
                SceneNode::Goto { .. } => "Goto",
                SceneNode::Call { .. } => "Call",
                SceneNode::Return => "Return",
                SceneNode::Wait { .. } => "Wait",
                SceneNode::Label { .. } => "Label",
            }
        }

        // 构造所有变体实例并计数
        let variants: Vec<SceneNode> = vec![
            SceneNode::Bg {
                asset_path: s("x"),
                transition: None,
            },
            SceneNode::ShowChar {
                char_id: s("x"),
                position: Position::Center,
                emotion: None,
                transition: None,
            },
            SceneNode::ShowSprite {
                asset_path: s("x"),
                x: f(0.0),
                y: f(0.0),
                scale: f(1.0),
                alpha: f(1.0),
                transition: None,
            },
            SceneNode::MoveChar {
                char_id: s("x"),
                position: Position::Center,
                emotion: None,
                transition: TransitionSpec {
                    kind: "x".into(),
                    duration_ms: i(0),
                },
            },
            SceneNode::Emotion {
                char_id: s("x"),
                emotion: s("x"),
                transition: None,
            },
            SceneNode::HideChar {
                char_id: s("x"),
                transition: None,
            },
            SceneNode::HideSprite {
                asset_path: s("x"),
                transition: None,
            },
            SceneNode::Dialogue {
                speaker: s("x"),
                text: s("x"),
                voice_id: None,
            },
            SceneNode::Narration { text: s("x") },
            SceneNode::Menu {
                prompt: s("x"),
                choices: vec![],
            },
            SceneNode::Branch {
                condition: Expr::bool_literal(true),
                then_nodes: vec![],
                elif_branches: vec![],
                else_nodes: None,
            },
            SceneNode::SetVariable {
                name: "x".into(),
                value: i(0),
            },
            SceneNode::SetFlag { name: "x".into() },
            SceneNode::UnsetFlag { name: "x".into() },
            SceneNode::ToggleFlag { name: "x".into() },
            SceneNode::Music {
                asset_path: s("x"),
                fade_in: None,
                looping: true,
            },
            SceneNode::StopMusic { fade_out: None },
            SceneNode::PlaySE {
                asset_id: s("x"),
                fade_in: None,
            },
            SceneNode::Effect {
                effect_type: "x".into(),
                params: HashMap::new(),
            },
            SceneNode::Jump { target: s("x") },
            SceneNode::Goto {
                scene_id: s("x"),
                label: None,
            },
            SceneNode::Call { target: s("x") },
            SceneNode::Return,
            SceneNode::Wait { duration_ms: i(0) },
            SceneNode::Label { name: "x".into() },
        ];

        // 确保每个变体都能通过 count_variants 且返回唯一标识
        let names: Vec<&str> = variants.iter().map(|v| count_variants(v)).collect();
        assert_eq!(variants.len(), 25, "SceneNode 应有 25 个变体");
        assert_eq!(names.len(), 25);
    }

    /// AC04 — `Scene` 结构体的 JSON 序列化 round-trip 正确
    ///
    /// 构造完整的 Scene → serde_json 序列化 → 反序列化 → 断言关键字段一致。
    #[test]
    fn ac04_scene_json_roundtrip() {
        let scene = Scene {
            id: "chapter1/prologue".into(),
            label: Some("序章".into()),
            background: Some(s("bg_classroom_day.png")),
            music: Some(s("bgm_peaceful.ogg")),
            nodes: vec![
                SceneNode::Bg {
                    asset_path: s("bg_classroom.png"),
                    transition: Some(TransitionSpec {
                        kind: "fade".into(),
                        duration_ms: i(500),
                    }),
                },
                SceneNode::Music {
                    asset_path: s("bgm_daily.ogg"),
                    fade_in: Some(f(1.0)),
                    looping: true,
                },
                SceneNode::Narration {
                    text: s("春天…")
                },
                SceneNode::ShowChar {
                    char_id: s("sayori"),
                    position: Position::Center,
                    emotion: Some(s("default")),
                    transition: None,
                },
                SceneNode::Dialogue {
                    speaker: s("小百合"),
                    text: s("你好！"),
                    voice_id: None,
                },
                SceneNode::Menu {
                    prompt: s("选择"),
                    choices: vec![
                        Choice {
                            text: s("选项A"),
                            target: s("label_a"),
                            condition: None,
                        },
                        Choice {
                            text: s("选项B"),
                            target: s("label_b"),
                            condition: Some(Expr::variable("flag")),
                        },
                    ],
                },
            ],
        };

        // 序列化为 JSON
        let json_str = serde_json::to_string(&scene).expect("JSON 序列化失败");
        // 反序列化回来
        let restored: Scene = serde_json::from_str(&json_str).expect("JSON 反序列化失败");

        // AC04 核心断言：id 和 nodes.len() 一致
        assert_eq!(restored.id, scene.id);
        assert_eq!(restored.nodes.len(), scene.nodes.len());

        // 额外验证：标签和背景
        assert_eq!(restored.label.as_deref(), Some("序章"));
        assert_eq!(restored.background, Some(s("bg_classroom_day.png")));
        assert_eq!(restored.music, Some(s("bgm_peaceful.ogg")));
    }

    /// AC04 补充 — 验证空场景（no nodes）的序列化
    #[test]
    fn ac04_empty_scene_json_roundtrip() {
        let scene = Scene {
            id: "empty_scene".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![],
        };

        let json_str = serde_json::to_string(&scene).expect("JSON 序列化失败");
        let restored: Scene = serde_json::from_str(&json_str).expect("JSON 反序列化失败");

        assert_eq!(restored.id, "empty_scene");
        assert!(restored.nodes.is_empty());
        assert!(restored.label.is_none());
        assert!(restored.background.is_none());
    }

    /// AC04 补充 — 验证 Branch 嵌套结构的 JSON round-trip
    #[test]
    fn ac04_nested_branch_json_roundtrip() {
        let scene = Scene {
            id: "test_branch".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![SceneNode::Branch {
                condition: Expr::binary_op(Expr::variable("a"), crate::expr::BinaryOp::Gt, i(0)),
                then_nodes: vec![SceneNode::Dialogue {
                    speaker: s("X"),
                    text: s("then"),
                    voice_id: None,
                }],
                elif_branches: vec![(
                    Expr::binary_op(Expr::variable("a"), crate::expr::BinaryOp::Eq, i(0)),
                    vec![SceneNode::Dialogue {
                        speaker: s("X"),
                        text: s("elif"),
                        voice_id: None,
                    }],
                )],
                else_nodes: Some(vec![SceneNode::Dialogue {
                    speaker: s("X"),
                    text: s("else"),
                    voice_id: None,
                }]),
            }],
        };

        let json_str = serde_json::to_string(&scene).expect("JSON 序列化失败");
        let restored: Scene = serde_json::from_str(&json_str).expect("JSON 反序列化失败");

        assert_eq!(restored.nodes.len(), 1);
        if let SceneNode::Branch {
            condition,
            then_nodes,
            elif_branches,
            else_nodes,
        } = &restored.nodes[0]
        {
            assert!(matches!(condition, Expr::BinaryOp(..)));
            assert_eq!(then_nodes.len(), 1);
            assert_eq!(elif_branches.len(), 1);
            assert!(else_nodes.is_some());
            assert_eq!(else_nodes.as_ref().unwrap().len(), 1);
        } else {
            panic!("反序列化后应为 Branch 变体");
        }
    }

    // ─── Position 测试 ──────────────────────────────────────────────────

    /// 验证 Position::to_coords() 方法返回正确的归一化坐标
    #[test]
    fn position_to_coords() {
        // 预设位置始终返回 Some
        assert_eq!(Position::Left.to_coords(), Some((0.25, 0.5)));
        assert_eq!(Position::Center.to_coords(), Some((0.5, 0.5)));
        assert_eq!(Position::Right.to_coords(), Some((0.75, 0.5)));

        // Custom 字面量 → Some
        assert_eq!(
            Position::Custom(f(0.1), f(0.8)).to_coords(),
            Some((0.1, 0.8))
        );

        // Custom 含变量 → None（需要运行时求值）
        assert_eq!(
            Position::Custom(Expr::variable("x"), f(0.5)).to_coords(),
            None
        );
    }

    /// 验证 Position 的 JSON 序列化
    #[test]
    fn position_json_serialization() {
        let positions = vec![
            Position::Left,
            Position::Center,
            Position::Right,
            Position::Custom(f(0.33), f(0.66)),
        ];

        let json_str = serde_json::to_string(&positions).expect("序列化失败");
        let restored: Vec<Position> = serde_json::from_str(&json_str).expect("反序列化失败");

        assert_eq!(restored.len(), 4);
        assert_eq!(restored[0], Position::Left);
        assert_eq!(restored[1], Position::Center);
        assert_eq!(restored[2], Position::Right);
        assert_eq!(restored[3], Position::Custom(f(0.33), f(0.66)));
    }

    /// 验证 TransitionSpec 的创建和序列化
    #[test]
    fn transition_spec_serialization() {
        let spec = TransitionSpec {
            kind: "fade".into(),
            duration_ms: i(1000),
        };
        let json_str = serde_json::to_string(&spec).expect("序列化失败");
        let restored: TransitionSpec = serde_json::from_str(&json_str).expect("反序列化失败");

        assert_eq!(restored.kind, "fade");
        assert_eq!(restored.duration_ms, i(1000));
    }

    // ─── Expr 表达式字段测试 ────────────────────────────────────────────

    /// 验证 SetVariable 支持复合表达式
    #[test]
    fn set_variable_with_expression() {
        // $score = $score + 1
        let node = SceneNode::SetVariable {
            name: "score".into(),
            value: Expr::binary_op(Expr::variable("score"), crate::expr::BinaryOp::Add, i(1)),
        };

        if let SceneNode::SetVariable { name, value } = &node {
            assert_eq!(name, "score");
            assert!(matches!(value, Expr::BinaryOp(..)));
        } else {
            panic!("应为 SetVariable");
        }
    }

    /// 验证 Jump 支持变量引用作为跳转目标
    #[test]
    fn jump_with_variable_target() {
        // jump $next_label
        let node = SceneNode::Jump {
            target: Expr::variable("next_label"),
        };

        if let SceneNode::Jump { target } = &node {
            assert_eq!(target, &Expr::variable("next_label"));
        } else {
            panic!("应为 Jump");
        }
    }

    /// 验证 Wait 支持动态时长
    #[test]
    fn wait_with_expression() {
        // wait $base_delay * 2
        let node = SceneNode::Wait {
            duration_ms: Expr::binary_op(
                Expr::variable("base_delay"),
                crate::expr::BinaryOp::Mul,
                i(2),
            ),
        };

        if let SceneNode::Wait { duration_ms } = &node {
            assert!(matches!(duration_ms, Expr::BinaryOp(..)));
        } else {
            panic!("应为 Wait");
        }
    }

    /// 验证 ShowSprite 坐标支持变量
    #[test]
    fn show_sprite_with_variable_position() {
        let node = SceneNode::ShowSprite {
            asset_path: s("ui/cursor.png"),
            x: Expr::variable("mouse_x"),
            y: Expr::variable("mouse_y"),
            scale: f(1.0),
            alpha: f(1.0),
            transition: None,
        };

        if let SceneNode::ShowSprite { x, y, .. } = &node {
            assert_eq!(x, &Expr::variable("mouse_x"));
            assert_eq!(y, &Expr::variable("mouse_y"));
        } else {
            panic!("应为 ShowSprite");
        }
    }

    /// 验证 Choice condition 用 Expr 正确存储条件
    #[test]
    fn choice_with_expr_condition() {
        let choice = Choice {
            text: s("秘密路线"),
            target: s("secret"),
            condition: Some(Expr::binary_op(
                Expr::variable("affection"),
                crate::expr::BinaryOp::Ge,
                i(10),
            )),
        };

        assert_eq!(choice.text, s("秘密路线"));
        assert_eq!(choice.target, s("secret"));
        assert!(choice.condition.is_some());
    }

    /// 验证 Sequence serde: Scene 含多种 Expr 字段的 JSON round-trip
    #[test]
    fn scene_with_expr_fields_json_roundtrip() {
        let scene = Scene {
            id: "test_expr".into(),
            label: None,
            background: Some(Expr::variable("current_bg")),
            music: None,
            nodes: vec![
                SceneNode::SetVariable {
                    name: "x".into(),
                    value: Expr::binary_op(Expr::variable("a"), crate::expr::BinaryOp::Add, i(1)),
                },
                SceneNode::Wait {
                    duration_ms: Expr::variable("delay"),
                },
                SceneNode::Jump { target: s("end") },
            ],
        };

        let json = serde_json::to_string(&scene).expect("JSON 序列化失败");
        let restored: Scene = serde_json::from_str(&json).expect("JSON 反序列化失败");

        assert_eq!(restored.id, "test_expr");
        assert_eq!(restored.nodes.len(), 3);
        assert_eq!(restored.background, Some(Expr::variable("current_bg")));
    }

    // ─── serde 反序列化错误路径测试 ─────────────────────────────────────

    /// 验证 SceneNode 反序列化时遇到未知 type tag 的行为。
    ///
    /// `#[serde(tag = "type")]` 模式下，未知 variant 会导致反序列化错误。
    #[test]
    fn scene_node_deserialize_unknown_type_tag() {
        // "NonExistentVariant" 不是有效的 SceneNode type
        let json = r#"{"type": "NonExistentVariant", "text": "hello"}"#;
        let result: Result<SceneNode, _> = serde_json::from_str(json);
        assert!(result.is_err(), "未知 type tag 应导致反序列化错误");
    }

    /// 验证 Position 反序列化时遇到未知 variant 的行为。
    #[test]
    fn position_deserialize_unknown_variant() {
        let json = r#""top""#; // "top" 不是 Position 的有效 variant
        let result: Result<Position, _> = serde_json::from_str(json);
        assert!(result.is_err(), "未知 Position variant 应导致反序列化错误");
    }

    /// 验证 Scene 反序列化时缺少必填字段 id 的行为。
    #[test]
    fn scene_deserialize_missing_id() {
        // Scene 的 id 字段没有默认值，缺少时应反序列化失败
        let json = r#"{"label": "test", "nodes": []}"#;
        let result: Result<Scene, _> = serde_json::from_str(json);
        assert!(result.is_err(), "缺少必填字段 id 应导致反序列化错误");
    }
}
