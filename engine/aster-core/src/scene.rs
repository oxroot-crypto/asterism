//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/scene.rs
//! 功能概述：场景定义类型 — 定义 `Scene` 结构体（一组 `SceneNode` 的有序列表）、
//!           `SceneNode` 枚举（25 种演出单元变体）、`Choice` 选择支、`Position` 立绘位置、
//!           `TransitionSpec` 转场规格等核心数据模型。
//! 作者：Claude (AI)
//! 创建日期：2026-06-13
//! 最后修改：2026-06-13
//!
//! 依赖模块：
//! - serde（序列化/反序列化支持）
//! - std::collections::HashMap（Effect 参数映射）
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

/// 场景定义 — 一个完整的游戏场景，包含一组按顺序执行的 `SceneNode`。
///
/// 每个场景由唯一 `id` 标识（格式为 `"chapter/scene_name"` 的路径字符串），
/// 可选的默认背景和 BGM，以及一个 `SceneNode` 序列。
/// 场景通过 `SceneManager` 加载并由 VM 逐节点执行。
///
/// # 序列化
///
/// 支持 JSON 和 TOML 序列化/反序列化：
/// ```rust,ignore
/// let scene: Scene = serde_json::from_str(&fs::read_to_string("scene.json")?)?;
/// ```
///
/// # 示例
/// ```rust,ignore
/// use aster_core::{Scene, SceneNode, Position};
///
/// let scene = Scene {
///     id: "chapter1/prologue".into(),
///     label: Some("序章".into()),
///     background: Some("bg_classroom_day.png".into()),
///     music: Some("bgm_peaceful.ogg".into()),
///     nodes: vec![
///         SceneNode::Narration { text: "春天，樱花盛开的季节。".into() },
///         SceneNode::ShowChar {
///             char_id: "sayori".into(),
///             position: Position::Center,
///             emotion: Some("smile".into()),
///             transition: None,
///         },
///         SceneNode::Dialogue {
///             speaker: "小百合".into(),
///             text: "初次见面，请多指教！".into(),
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

    /// 默认背景资源路径（可选）
    /// 场景启动时自动加载，后续可通过 `SceneNode::Bg` 切换
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,

    /// 默认 BGM 资源路径（可选）
    /// 场景启动时自动播放，后续可通过 `SceneNode::Music` 切换
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music: Option<String>,

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
    /// `transition` 指定可选的转场效果（如 fade、dissolve）。
    Bg {
        /// 背景资源路径
        asset_path: String,
        /// 可选的转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 对话节点：显示说话者的对话文本，暂停并等待用户点击继续。
    ///
    /// 对话文本支持 inline markup（Phase 4 实现），当前为纯文本。
    Dialogue {
        /// 说话者角色 ID（对应 `Character.id`）
        speaker: String,
        /// 对话内容（纯文本，Phase 4 起支持 inline markup）
        text: String,
        /// 可选的语音文件资源 ID
        #[serde(skip_serializing_if = "Option::is_none")]
        voice_id: Option<String>,
    },

    /// 显示角色立绘：在舞台上指定位置显示角色的立绘精灵。
    ShowChar {
        /// 角色 ID（对应 `Character.id`）
        char_id: String,
        /// 立绘在舞台上的位置（左/中/右/自定义坐标）
        position: Position,
        /// 可选的表情名（对应 `Character.sprites` 的 key，如 "smile"）
        #[serde(skip_serializing_if = "Option::is_none")]
        emotion: Option<String>,
        /// 可选的入场转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 显示独立精灵：在画面上展示一个不绑定角色系统的独立图片。
    ///
    /// 用途：道具图标、CG 切片、贴纸、表示情绪的符号（心形/感叹号）等。
    /// 与 `ShowChar` 的区别：不依赖 `Character`，直接用资源路径。
    ShowSprite {
        /// 图片资源路径
        asset_path: String,
        /// X 归一化坐标（0.0=左边缘, 1.0=右边缘），锚点为中心
        x: f32,
        /// Y 归一化坐标（0.0=顶部, 1.0=底部），锚点为中心
        y: f32,
        /// 缩放因子（1.0 = 原始像素尺寸）
        #[serde(default = "default_one")]
        scale: f32,
        /// 透明度（0.0=全透明, 1.0=不透明）
        #[serde(default = "default_one")]
        alpha: f32,
        /// 可选的入场转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 移动角色立绘：将已显示的角色立绘平滑移动到新位置，可选同步切换表情。
    ///
    /// 必须在 `ShowChar` 之后使用，目标角色当前必须正在显示。
    /// 移动动画类型和时长由 `transition` 字段指定。
    MoveChar {
        /// 角色 ID
        char_id: String,
        /// 目标位置
        position: Position,
        /// 可选：同步切换到新表情
        #[serde(skip_serializing_if = "Option::is_none")]
        emotion: Option<String>,
        /// 移动动画规格（如 slide(0.8) 表示 800ms 滑动过渡）
        transition: TransitionSpec,
    },

    /// 切换角色表情：将已显示的角色的立绘原地切换为另一个表情，不改变位置。
    ///
    /// 必须在 `ShowChar` 之后使用，目标角色当前必须正在显示。
    /// 与 `MoveChar` 的区别：Emotion 不改变位置，只换立绘图片。
    Emotion {
        /// 角色 ID
        char_id: String,
        /// 新表情名（对应 `Character.sprites` 的 key）
        emotion: String,
        /// 可选的切换动画规格（如 dissolve(0.3)）
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 隐藏角色立绘：从舞台上移除指定角色的立绘。
    HideChar {
        /// 角色 ID
        char_id: String,
        /// 可选的退场转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 隐藏独立精灵：从画面上移除之前通过 `ShowSprite` 显示的独立图片。
    ///
    /// `asset_path` 用于匹配要隐藏的精灵资源。
    /// 如果同一资源被多次 ShowSprite 调用，HideSprite 会隐藏所有匹配实例。
    HideSprite {
        /// 图片资源路径（匹配 `ShowSprite.asset_path`）
        asset_path: String,
        /// 可选的退场转场效果规格
        #[serde(skip_serializing_if = "Option::is_none")]
        transition: Option<TransitionSpec>,
    },

    /// 旁白节点：显示无说话者的叙述文本。
    ///
    /// 与 `Dialogue` 的区别：没有 speaker 字段，视觉样式通常不同（如居中、斜体）。
    Narration {
        /// 旁白文本内容
        text: String,
    },

    /// 菜单/选择支节点：显示一组选项并等待玩家选择。
    ///
    /// 每个选项包含显示文本和跳转目标标签。
    /// 当 `choices` 为空时，这是脚本语义错误（应在编译期捕获）。
    Menu {
        /// 选择支提示文本（如 "你要怎么做？"）
        prompt: String,
        /// 选项列表（最少 2 个，最多 N 个）
        choices: Vec<Choice>,
    },

    /// 条件分支节点：根据运行时条件表达式的求值结果选择执行路径。
    ///
    /// 支持 `if / elif / else` 完整分支结构。
    /// `condition` 为字符串形式的表达式（如 `"$affection >= 5"`），
    /// VM 执行时解析并求值。
    Branch {
        /// if 条件表达式字符串
        condition: String,
        /// 条件为 true 时执行的节点序列
        then_nodes: Vec<SceneNode>,
        /// elif 分支列表（按声明顺序），每项为 (条件, 节点序列)
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        elif_branches: Vec<(String, Vec<SceneNode>)>,
        /// 可选的 else 分支节点序列
        #[serde(skip_serializing_if = "Option::is_none")]
        else_nodes: Option<Vec<SceneNode>>,
    },

    /// 设置变量：将表达式的求值结果赋给指定变量。
    ///
    /// `value` 为字符串形式的表达式（如 `"42"`、`"$score + 1"`），
    /// VM 执行时解析求值并写入 `VariableStore`。
    SetVariable {
        /// 变量名（如 `"score"`、`"affection"`）
        name: String,
        /// 值的表达式字符串
        value: String,
    },

    /// 设置旗标：将命名布尔旗标设为 true。
    SetFlag {
        /// 旗标名称
        name: String,
    },

    /// 清除旗标：将命名布尔旗标设为 false。
    UnsetFlag {
        /// 旗标名称
        name: String,
    },

    /// 切换旗标：反转布尔旗标的当前值（true↔false）。
    ///
    /// 等效于：`if check(flag) { unset(flag) } else { set(flag) }`
    ToggleFlag {
        /// 旗标名称
        name: String,
    },

    /// 播放/切换背景音乐：开始播放或切换到指定 BGM 资源。
    ///
    /// BGM 默认循环播放（`looping = true`），可通过 `fade_in` 指定淡入时间。
    /// 如果当前已有 BGM 在播放，将平滑切换到新曲目。
    Music {
        /// BGM 资源路径
        asset_path: String,
        /// 可选的淡入时间（秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_in: Option<f32>,
        /// 是否循环播放（默认 true）
        #[serde(default = "default_true")]
        looping: bool,
    },

    /// 停止背景音乐：停止当前正在播放的 BGM。
    ///
    /// `fade_out` 指定淡出时间，`None` 表示立即停止。
    StopMusic {
        /// 可选的淡出时间（秒），None = 立即停止
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_out: Option<f32>,
    },

    /// 播放音效：播放一个音频资源（不阻断执行）。
    PlaySE {
        /// 音效资源 ID
        asset_id: String,
        /// 可选的淡入时间（秒）
        #[serde(skip_serializing_if = "Option::is_none")]
        fade_in: Option<f32>,
    },

    /// 等待：暂停执行指定的毫秒数。
    ///
    /// 用于控制节奏（如 `Wait { duration_ms: 1500 }` 暂停 1.5 秒）。
    Wait {
        /// 等待时长（毫秒）
        duration_ms: u64,
    },

    /// 视觉效果：触发指定的画面特效。
    ///
    /// Phase 1 仅定义数据结构，实际渲染由后续 Phase 实现。
    Effect {
        /// 特效类型标识（如 `"shake"`、`"flash"`、`"fade_in"`）
        effect_type: String,
        /// 特效参数键值对（如 `{"duration": "500", "intensity": "0.8"}`）
        #[serde(default)]
        params: HashMap<String, String>,
    },

    /// 场景内无条件跳转：将执行位置跳转到**当前场景内**的指定标签处。
    ///
    /// `target` 必须在当前场景的 `nodes` 中存在对应的 `Label` 节点，
    /// 否则 VM 在编译期报错（语义错误）。
    ///
    /// 如需跨场景跳转，请使用 `Goto` 变体。
    Jump {
        /// 跳转目标标签名（场景内）
        target: String,
    },

    /// 跨场景跳转：将执行转移到另一个场景，可选择性指定目标场景内的起始标签。
    ///
    /// 与 `Jump` 的区别：
    /// - `Jump` → 当前场景内，目标为 `Label`
    /// - `Goto` → 跨场景，目标为另一个 `.aster` 文件
    ///
    /// `label` 为 `None` 时从目标场景的第一条指令开始执行；
    /// 为 `Some(name)` 时从目标场景中对应的 `Label` 处开始执行。
    Goto {
        /// 目标场景 ID（如 `"chapter2/romance"`）
        scene_id: String,
        /// 可选：目标场景内的起始标签名
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },

    /// 子例程调用：将当前执行位置压栈，跳转到指定标签。
    ///
    /// Phase 1 定义数据结构，VM 支持（PH1-T13 实现），
    /// 完整调用栈在 Phase 2 中与存档功能集成。
    Call {
        /// 调用目标标签名
        target: String,
    },

    /// 子例程返回：从调用栈弹出返回地址并跳转回去。
    Return,

    /// 标签：定义跳转目标点，自身不产生任何副作用。
    ///
    /// 标签名在同一场景内必须唯一。
    Label {
        /// 标签名称
        name: String,
    },
}

/// serde 默认值辅助函数 — 返回 `true`（用于 `Music.looping` 等布尔字段）
const fn default_true() -> bool {
    true
}

/// serde 默认值辅助函数 — 返回 `1.0`（用于 `ShowSprite.scale`、`ShowSprite.alpha` 等浮点字段）
const fn default_one() -> f32 {
    1.0
}

/// 选择支 — 菜单中的一个选项。
///
/// 每个选项包含显示给玩家的文本、跳转目标标签，
/// 以及可选的条件表达式（用于条件选项，Phase 1 阶段仅存储字符串）。
///
/// # 示例
/// ```rust,ignore
/// let choice = Choice {
///     text: "上前搭话".into(),
///     target: "approach_label".into(),
///     condition: Some("$affection >= 3".into()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Choice {
    /// 选项的显示文本
    pub text: String,

    /// 选中后跳转的目标标签名
    pub target: String,

    /// 可选的条件表达式字符串
    ///
    /// 当 `condition` 为 `Some(expr)` 时，该选项仅在 `expr` 求值为 true 时显示。
    /// Phase 1 阶段仅存储此字符串，VM 执行时解析和求值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

/// 立绘舞台位置 — 定义角色立绘在画面中的水平/垂直位置。
///
/// 使用归一化坐标系（0.0~1.0），锚点默认在立绘底部中心。
/// 预设位置 `Left`/`Center`/`Right` 映射到固定坐标。
///
/// # 坐标约定
/// - X 轴：0.0 = 画面左边缘，1.0 = 画面右边缘
/// - Y 轴：0.0 = 画面顶部，1.0 = 画面底部
/// - 锚点：立绘底部中心
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Position {
    /// 左侧 — 等效于 `Custom(0.25, 0.5)`
    #[serde(rename = "left")]
    Left,

    /// 中央 — 等效于 `Custom(0.5, 0.5)`
    #[serde(rename = "center")]
    Center,

    /// 右侧 — 等效于 `Custom(0.75, 0.5)`
    #[serde(rename = "right")]
    Right,

    /// 自定义坐标 — (x, y)，均为归一化值（0.0~1.0）
    #[serde(rename = "custom")]
    Custom(f32, f32),
}

impl Position {
    /// 将 Position 转换为归一化 (x, y) 坐标。
    ///
    /// # 返回值
    /// - `Left` → `(0.25, 0.5)`
    /// - `Center` → `(0.5, 0.5)`
    /// - `Right` → `(0.75, 0.5)`
    /// - `Custom(x, y)` → `(x, y)`（直接返回）
    ///
    /// # 示例
    /// ```rust,ignore
    /// assert_eq!(Position::Left.to_coords(), (0.25, 0.5));
    /// assert_eq!(Position::Center.to_coords(), (0.5, 0.5));
    /// assert_eq!(Position::Right.to_coords(), (0.75, 0.5));
    /// assert_eq!(Position::Custom(0.1, 0.8).to_coords(), (0.1, 0.8));
    /// ```
    pub fn to_coords(&self) -> (f32, f32) {
        match self {
            Position::Left => (0.25, 0.5),
            Position::Center => (0.5, 0.5),
            Position::Right => (0.75, 0.5),
            Position::Custom(x, y) => (*x, *y),
        }
    }
}

/// 转场效果规格 — 定义角色立绘或背景切换时的过渡动画参数。
///
/// Phase 1 仅定义数据结构，实际转场渲染在 Phase 4 中实现。
/// 当前 `kind` 字段存储转场类型名（如 `"fade"`、`"dissolve"`、`"slide_left"`），
/// `duration_ms` 指定持续时间。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransitionSpec {
    /// 转场类型标识（如 `"fade"`、`"dissolve"`、`"slide_left"`）
    pub kind: String,

    /// 转场持续时间（毫秒）
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC02 — `SceneNode` 枚举的所有 variant 均可正确创建和模式匹配
    ///
    /// 逐一构造每个 SceneNode variant，通过模式匹配提取字段值并断言正确性。
    #[test]
    fn ac02_create_and_match_all_scene_node_variants() {
        // 渲染类：Dialogue
        let dialogue = SceneNode::Dialogue {
            speaker: "小百合".into(),
            text: "你好！".into(),
            voice_id: Some("voice_001.ogg".into()),
        };
        if let SceneNode::Dialogue {
            speaker,
            text,
            voice_id,
        } = &dialogue
        {
            assert_eq!(speaker, "小百合");
            assert_eq!(text, "你好！");
            assert_eq!(voice_id.as_deref(), Some("voice_001.ogg"));
        } else {
            panic!("应为 Dialogue 变体");
        }

        // 渲染类：ShowChar
        let show_char = SceneNode::ShowChar {
            char_id: "sayori".into(),
            position: Position::Left,
            emotion: Some("smile".into()),
            transition: None,
        };
        if let SceneNode::ShowChar {
            char_id,
            position,
            emotion,
            transition,
        } = &show_char
        {
            assert_eq!(char_id, "sayori");
            assert_eq!(position, &Position::Left);
            assert_eq!(emotion.as_deref(), Some("smile"));
            assert!(transition.is_none());
        } else {
            panic!("应为 ShowChar 变体");
        }

        // 渲染类：HideChar
        let hide_char = SceneNode::HideChar {
            char_id: "sayori".into(),
            transition: Some(TransitionSpec {
                kind: "fade".into(),
                duration_ms: 500,
            }),
        };
        if let SceneNode::HideChar {
            char_id,
            transition,
        } = &hide_char
        {
            assert_eq!(char_id, "sayori");
            assert!(transition.is_some());
        } else {
            panic!("应为 HideChar 变体");
        }

        // 渲染类：Narration
        let narration = SceneNode::Narration {
            text: "这是一个春天的早晨。".into(),
        };
        if let SceneNode::Narration { text } = &narration {
            assert_eq!(text, "这是一个春天的早晨。");
        } else {
            panic!("应为 Narration 变体");
        }

        // 交互类：Menu
        let menu = SceneNode::Menu {
            prompt: "你要怎么做？".into(),
            choices: vec![
                Choice {
                    text: "上前搭话".into(),
                    target: "approach".into(),
                    condition: None,
                },
                Choice {
                    text: "转身离开".into(),
                    target: "leave".into(),
                    condition: Some("$courage < 3".into()),
                },
            ],
        };
        if let SceneNode::Menu { prompt, choices } = &menu {
            assert_eq!(prompt, "你要怎么做？");
            assert_eq!(choices.len(), 2);
            assert_eq!(choices[0].text, "上前搭话");
            assert_eq!(choices[1].condition.as_deref(), Some("$courage < 3"));
        } else {
            panic!("应为 Menu 变体");
        }

        // 控制流：Branch (if/elif/else)
        let branch = SceneNode::Branch {
            condition: "$score >= 10".into(),
            then_nodes: vec![SceneNode::Dialogue {
                speaker: "系统".into(),
                text: "恭喜！".into(),
                voice_id: None,
            }],
            elif_branches: vec![(
                "$score >= 5".into(),
                vec![SceneNode::Dialogue {
                    speaker: "系统".into(),
                    text: "还不错。".into(),
                    voice_id: None,
                }],
            )],
            else_nodes: Some(vec![SceneNode::Dialogue {
                speaker: "系统".into(),
                text: "继续努力。".into(),
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
            assert_eq!(condition, "$score >= 10");
            assert_eq!(then_nodes.len(), 1);
            assert_eq!(elif_branches.len(), 1);
            assert_eq!(elif_branches[0].0, "$score >= 5");
            assert!(else_nodes.is_some());
        } else {
            panic!("应为 Branch 变体");
        }

        // 状态类：SetVariable
        let set_var = SceneNode::SetVariable {
            name: "score".into(),
            value: "100".into(),
        };
        if let SceneNode::SetVariable { name, value } = &set_var {
            assert_eq!(name, "score");
            assert_eq!(value, "100");
        } else {
            panic!("应为 SetVariable 变体");
        }

        // 状态类：SetFlag
        let set_flag = SceneNode::SetFlag {
            name: "met_heroine".into(),
        };
        if let SceneNode::SetFlag { name } = &set_flag {
            assert_eq!(name, "met_heroine");
        } else {
            panic!("应为 SetFlag 变体");
        }

        // 状态类：UnsetFlag
        let unset_flag = SceneNode::UnsetFlag {
            name: "bad_end_flag".into(),
        };
        if let SceneNode::UnsetFlag { name } = &unset_flag {
            assert_eq!(name, "bad_end_flag");
        } else {
            panic!("应为 UnsetFlag 变体");
        }

        // 媒体类：PlaySE
        let play_se = SceneNode::PlaySE {
            asset_id: "se_ding.ogg".into(),
            fade_in: Some(0.1),
        };
        if let SceneNode::PlaySE { asset_id, fade_in } = &play_se {
            assert_eq!(asset_id, "se_ding.ogg");
            assert!((fade_in.unwrap() - 0.1).abs() < f32::EPSILON);
        } else {
            panic!("应为 PlaySE 变体");
        }

        // 时序类：Wait
        let wait = SceneNode::Wait { duration_ms: 1500 };
        if let SceneNode::Wait { duration_ms } = &wait {
            assert_eq!(*duration_ms, 1500);
        } else {
            panic!("应为 Wait 变体");
        }

        // 媒体类：Effect
        let mut params = HashMap::new();
        params.insert("intensity".into(), "0.8".into());
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
            assert_eq!(params.get("intensity"), Some(&"0.8".to_string()));
        } else {
            panic!("应为 Effect 变体");
        }

        // 控制流：Jump
        let jump = SceneNode::Jump {
            target: "next_chapter".into(),
        };
        if let SceneNode::Jump { target } = &jump {
            assert_eq!(target, "next_chapter");
        } else {
            panic!("应为 Jump 变体");
        }

        // 控制流：Call
        let call = SceneNode::Call {
            target: "subroutine".into(),
        };
        if let SceneNode::Call { target } = &call {
            assert_eq!(target, "subroutine");
        } else {
            panic!("应为 Call 变体");
        }

        // 控制流：Return
        let ret = SceneNode::Return;
        assert!(matches!(ret, SceneNode::Return));

        // 控制流：Label
        let label = SceneNode::Label {
            name: "start".into(),
        };
        if let SceneNode::Label { name } = &label {
            assert_eq!(name, "start");
        } else {
            panic!("应为 Label 变体");
        }

        // 渲染类：Bg
        let bg = SceneNode::Bg {
            asset_path: "bg_park.png".into(),
            transition: Some(TransitionSpec {
                kind: "fade".into(),
                duration_ms: 800,
            }),
        };
        if let SceneNode::Bg {
            asset_path,
            transition,
        } = &bg
        {
            assert_eq!(asset_path, "bg_park.png");
            assert!(transition.is_some());
        } else {
            panic!("应为 Bg 变体");
        }

        // 媒体类：Music
        let music = SceneNode::Music {
            asset_path: "bgm_peaceful.ogg".into(),
            fade_in: Some(2.0),
            looping: true,
        };
        if let SceneNode::Music {
            asset_path,
            fade_in,
            looping,
        } = &music
        {
            assert_eq!(asset_path, "bgm_peaceful.ogg");
            assert!((fade_in.unwrap() - 2.0).abs() < f32::EPSILON);
            assert!(looping);
        } else {
            panic!("应为 Music 变体");
        }

        // 媒体类：StopMusic
        let stop_music = SceneNode::StopMusic {
            fade_out: Some(1.5),
        };
        if let SceneNode::StopMusic { fade_out } = &stop_music {
            assert!((fade_out.unwrap() - 1.5).abs() < f32::EPSILON);
        } else {
            panic!("应为 StopMusic 变体");
        }

        // 状态类：ToggleFlag
        let toggle_flag = SceneNode::ToggleFlag {
            name: "seen_event".into(),
        };
        if let SceneNode::ToggleFlag { name } = &toggle_flag {
            assert_eq!(name, "seen_event");
        } else {
            panic!("应为 ToggleFlag 变体");
        }

        // 控制流：Goto (跨场景跳转)
        let goto = SceneNode::Goto {
            scene_id: "chapter2/romance".into(),
            label: Some("start".into()),
        };
        if let SceneNode::Goto { scene_id, label } = &goto {
            assert_eq!(scene_id, "chapter2/romance");
            assert_eq!(label.as_deref(), Some("start"));
        } else {
            panic!("应为 Goto 变体");
        }

        // 控制流：Goto 不带 label
        let goto_no_label = SceneNode::Goto {
            scene_id: "chapter3/epilogue".into(),
            label: None,
        };
        if let SceneNode::Goto { scene_id, label } = &goto_no_label {
            assert_eq!(scene_id, "chapter3/epilogue");
            assert!(label.is_none());
        } else {
            panic!("应为 Goto 变体 (label=None)");
        }

        // 渲染类：ShowSprite
        let show_sprite = SceneNode::ShowSprite {
            asset_path: "ui/icon_heart.png".into(),
            x: 0.9,
            y: 0.05,
            scale: 0.5,
            alpha: 0.8,
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
            assert_eq!(asset_path, "ui/icon_heart.png");
            assert!((*x - 0.9).abs() < f32::EPSILON);
            assert!((*y - 0.05).abs() < f32::EPSILON);
            assert!((*scale - 0.5).abs() < f32::EPSILON);
            assert!((*alpha - 0.8).abs() < f32::EPSILON);
            assert!(transition.is_none());
        } else {
            panic!("应为 ShowSprite 变体");
        }

        // 渲染类：HideSprite
        let hide_sprite = SceneNode::HideSprite {
            asset_path: "ui/icon_heart.png".into(),
            transition: Some(TransitionSpec {
                kind: "fade".into(),
                duration_ms: 300,
            }),
        };
        if let SceneNode::HideSprite {
            asset_path,
            transition,
        } = &hide_sprite
        {
            assert_eq!(asset_path, "ui/icon_heart.png");
            assert!(transition.is_some());
        } else {
            panic!("应为 HideSprite 变体");
        }

        // 渲染类：MoveChar
        let move_char = SceneNode::MoveChar {
            char_id: "sayori".into(),
            position: Position::Right,
            emotion: Some("angry".into()),
            transition: TransitionSpec {
                kind: "slide".into(),
                duration_ms: 800,
            },
        };
        if let SceneNode::MoveChar {
            char_id,
            position,
            emotion,
            transition,
        } = &move_char
        {
            assert_eq!(char_id, "sayori");
            assert_eq!(position, &Position::Right);
            assert_eq!(emotion.as_deref(), Some("angry"));
            assert_eq!(transition.kind, "slide");
            assert_eq!(transition.duration_ms, 800);
        } else {
            panic!("应为 MoveChar 变体");
        }

        // 渲染类：Emotion
        let emotion = SceneNode::Emotion {
            char_id: "sayori".into(),
            emotion: "surprised".into(),
            transition: Some(TransitionSpec {
                kind: "dissolve".into(),
                duration_ms: 300,
            }),
        };
        if let SceneNode::Emotion {
            char_id,
            emotion: em,
            transition,
        } = &emotion
        {
            assert_eq!(char_id, "sayori");
            assert_eq!(em, "surprised");
            assert!(transition.is_some());
        } else {
            panic!("应为 Emotion 变体");
        }

        // ShowSprite 验证默认值（scale=1.0, alpha=1.0）
        let sprite_default = SceneNode::ShowSprite {
            asset_path: "ui/dot.png".into(),
            x: 0.5,
            y: 0.5,
            scale: 1.0,
            alpha: 1.0,
            transition: None,
        };
        assert!(matches!(
            sprite_default,
            SceneNode::ShowSprite {
                scale: 1.0,
                alpha: 1.0,
                ..
            }
        ));
    }

    /// AC04 — `Scene` 结构体的 JSON 序列化 round-trip 正确
    ///
    /// 构造完整的 Scene → serde_json 序列化 → 反序列化 → 断言关键字段一致。
    #[test]
    fn ac04_scene_json_roundtrip() {
        let scene = Scene {
            id: "chapter1/prologue".into(),
            label: Some("序章".into()),
            background: Some("bg_classroom_day.png".into()),
            music: Some("bgm_peaceful.ogg".into()),
            nodes: vec![
                SceneNode::Bg {
                    asset_path: "bg_classroom.png".into(),
                    transition: Some(TransitionSpec {
                        kind: "fade".into(),
                        duration_ms: 500,
                    }),
                },
                SceneNode::Music {
                    asset_path: "bgm_daily.ogg".into(),
                    fade_in: Some(1.0),
                    looping: true,
                },
                SceneNode::Narration {
                    text: "春天…".into(),
                },
                SceneNode::ShowChar {
                    char_id: "sayori".into(),
                    position: Position::Center,
                    emotion: Some("default".into()),
                    transition: None,
                },
                SceneNode::Dialogue {
                    speaker: "小百合".into(),
                    text: "你好！".into(),
                    voice_id: None,
                },
                SceneNode::Menu {
                    prompt: "选择".into(),
                    choices: vec![
                        Choice {
                            text: "选项A".into(),
                            target: "label_a".into(),
                            condition: None,
                        },
                        Choice {
                            text: "选项B".into(),
                            target: "label_b".into(),
                            condition: Some("$flag".into()),
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
        assert_eq!(restored.background.as_deref(), Some("bg_classroom_day.png"));
        assert_eq!(restored.music.as_deref(), Some("bgm_peaceful.ogg"));
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
                condition: "$a > 0".into(),
                then_nodes: vec![SceneNode::Dialogue {
                    speaker: "X".into(),
                    text: "then".into(),
                    voice_id: None,
                }],
                elif_branches: vec![(
                    "$a == 0".into(),
                    vec![SceneNode::Dialogue {
                        speaker: "X".into(),
                        text: "elif".into(),
                        voice_id: None,
                    }],
                )],
                else_nodes: Some(vec![SceneNode::Dialogue {
                    speaker: "X".into(),
                    text: "else".into(),
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
            assert_eq!(condition, "$a > 0");
            assert_eq!(then_nodes.len(), 1);
            assert_eq!(elif_branches.len(), 1);
            assert_eq!(elif_branches[0].0, "$a == 0");
            assert!(else_nodes.is_some());
            assert_eq!(else_nodes.as_ref().unwrap().len(), 1);
        } else {
            panic!("反序列化后应为 Branch 变体");
        }
    }

    /// 验证 Position::to_coords() 方法返回正确的归一化坐标
    #[test]
    fn position_to_coords() {
        assert_eq!(Position::Left.to_coords(), (0.25, 0.5));
        assert_eq!(Position::Center.to_coords(), (0.5, 0.5));
        assert_eq!(Position::Right.to_coords(), (0.75, 0.5));
        assert_eq!(Position::Custom(0.1, 0.8).to_coords(), (0.1, 0.8));
    }

    /// 验证 Position 的 JSON 序列化
    #[test]
    fn position_json_serialization() {
        let positions = vec![
            Position::Left,
            Position::Center,
            Position::Right,
            Position::Custom(0.33, 0.66),
        ];

        let json_str = serde_json::to_string(&positions).expect("序列化失败");
        let restored: Vec<Position> = serde_json::from_str(&json_str).expect("反序列化失败");

        assert_eq!(restored.len(), 4);
        assert_eq!(restored[0], Position::Left);
        assert_eq!(restored[1], Position::Center);
        assert_eq!(restored[2], Position::Right);
        assert_eq!(restored[3], Position::Custom(0.33, 0.66));
    }

    /// 验证 TransitionSpec 的创建和序列化
    #[test]
    fn transition_spec_serialization() {
        let spec = TransitionSpec {
            kind: "fade".into(),
            duration_ms: 1000,
        };
        let json_str = serde_json::to_string(&spec).expect("序列化失败");
        let restored: TransitionSpec = serde_json::from_str(&json_str).expect("反序列化失败");

        assert_eq!(restored.kind, "fade");
        assert_eq!(restored.duration_ms, 1000);
    }
}
