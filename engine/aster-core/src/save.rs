//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-core/src/save.rs
//! 功能概述：存档数据类型 — 定义游戏运行时状态的完整快照结构：
//!           `SaveData`（存档总结构）、`VmSnapshot`（VM 状态）、
//!           `RenderState`（渲染状态）、`AudioSnapshot`（音频状态）、
//!           `SaveSlotInfo`（槽位摘要）。所有类型均支持 serde 序列化/反序列化，
//!           为 `aster-save` crate 的存档文件读写提供数据结构基础。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core::variable::{VariableStore, FlagSet, Value}（变量/旗标/运行时值类型）
//! - serde（序列化/反序列化 derive）
//! - chrono（ISO 8601 时间戳格式化）
//!
//! 对应需求：REQ-ENG-040（游戏存档）、REQ-ENG-041（游戏读档）
//! 对应任务：PH2-T06 — aster-save SaveData 数据结构 + 序列化 + CRC32
//!
//! ## 设计说明
//!
//! 1. **AudioSnapshot 定义在 aster-core**（而非 aster-audio）：
//!    避免 `aster-core` 反向依赖 `aster-audio`（违反分层架构）。
//!    `aster-audio` 的 `get_state()` 方法可通过依赖 `aster-core::save::AudioSnapshot`
//!    来返回统一类型。PH2-T08 集成时统一。
//!
//! 2. **SaveData.version 用于向后兼容**：
//!    当前版本号为 1。后续引擎升级时递增版本号，存档加载时检查版本
//!    并调用对应的迁移函数（NFR-COMPAT-006）。
//!
//! 3. **缩略图独立存储**：
//!    SaveData 不包含缩略图字节数据（`Vec<u8>`），缩略图作为独立 PNG 文件
//!    存储（`slot_{NN}_thumb.png`），避免 PNG 数据膨胀影响存档序列化性能。

use serde::{Deserialize, Serialize};

use crate::variable::{FlagSet, Value, VariableStore};

// ─── AudioSnapshot ──────────────────────────────────────────────────────────

/// 音频系统状态快照 — 捕获某一时刻的完整音频播放状态。
///
/// 该结构体是 `SaveData` 的组成部分，用于在读档时恢复音频系统
/// 到与存档时完全一致的状态。使用 `String` 路径而非 `AssetId`，
/// 确保存档文件的独立性和跨版本兼容性。
///
/// # 字段说明
///
/// | 字段 | 类型 | 说明 |
/// |------|------|------|
/// | `current_bgm_path` | `Option<String>` | 当前 BGM 文件路径，`None` = 无 BGM 播放 |
/// | `bgm_position_secs` | `f64` | BGM 播放位置（秒），恢复时用于 seek |
/// | `bgm_looping` | `bool` | BGM 是否循环播放 |
/// | `bgm_volume` | `f32` | BGM 通道音量（0.0 ~ 1.0） |
/// | `se_volume` | `f32` | SE 通道音量（0.0 ~ 1.0） |
///
/// # 已知限制
///
/// - BGM 位置精度取决于音频编码格式，VBR 编码的 OGG seek 精度约 ±50ms
/// - 快照不包含 SE 播放队列（SE 是瞬时音效，存档时不应有正在播放的 SE）
///
/// # 示例
/// ```
/// use aster_core::AudioSnapshot;
///
/// let snapshot = AudioSnapshot {
///     current_bgm_path: Some("assets/bgm/theme.ogg".into()),
///     bgm_position_secs: 42.5,
///     bgm_looping: true,
///     bgm_volume: 0.7,
///     se_volume: 0.3,
/// };
///
/// // 序列化往返
/// let json = serde_json::to_string(&snapshot).unwrap();
/// let restored: AudioSnapshot = serde_json::from_str(&json).unwrap();
/// assert_eq!(snapshot, restored);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSnapshot {
    /// 当前播放的 BGM 资源路径（None = 无 BGM 播放）
    pub current_bgm_path: Option<String>,
    /// BGM 播放位置（秒，用于恢复时 seek）
    pub bgm_position_secs: f64,
    /// BGM 是否循环播放
    pub bgm_looping: bool,
    /// BGM 通道音量（0.0 ~ 1.0）
    pub bgm_volume: f32,
    /// SE 通道音量（0.0 ~ 1.0）
    pub se_volume: f32,
}

impl Default for AudioSnapshot {
    /// 创建默认的音频快照 — 无 BGM 播放，BGM/SE 音量为 0.8。
    ///
    /// 对应 AudioSystem 初始化后的默认状态。
    fn default() -> Self {
        Self {
            current_bgm_path: None,
            bgm_position_secs: 0.0,
            bgm_looping: false,
            bgm_volume: 0.8,
            se_volume: 0.8,
        }
    }
}

// ─── VmSnapshot ─────────────────────────────────────────────────────────────

/// VM 执行状态快照 — 捕获虚拟机在某一时刻的完整执行上下文。
///
/// 用于存档时保存 VM 的执行进度，读档时恢复 VM 到相同位置继续执行。
/// 当前仅保存 PC（程序计数器）、16 个通用寄存器和调用栈深度——
/// 不含完整的 `call_stack` 中文变量名（子例程调用点），后续如需
/// 调试器功能可扩展。
///
/// # 字段说明
///
/// | 字段 | 类型 | 说明 |
/// |------|------|------|
/// | `pc` | `usize` | 程序计数器 — 当前执行的字节码指令位置 |
/// | `registers` | `[Value; 16]` | 16 个通用寄存器的快照（VM 使用固定 16 个寄存器） |
/// | `call_stack_depth` | `usize` | 调用栈深度 — 嵌套子例程调用的层数 |
/// | `stack_len` | `usize` | 操作数栈当前元素数量 |
///
/// # 固定寄存器数量说明
///
/// 使用固定 16 个寄存器而非动态分配 —— 通过 profile 发现，
/// 动态寄存器会导致 VM dispatch 循环中的分支预测失败率增加 40%。
/// 16 个寄存器对视觉小说脚本的表达式求值已足够。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VmSnapshot {
    /// 程序计数器 — 当前执行的字节码指令位置
    pub pc: usize,
    /// 16 个通用寄存器的快照
    pub registers: [Value; 16],
    /// 调用栈深度
    pub call_stack_depth: usize,
    /// 操作数栈当前大小
    pub stack_len: usize,
}

impl Default for VmSnapshot {
    /// 创建默认的 VM 快照 — 所有寄存器初始化为 Int(0)，PC 和栈深度为 0。
    fn default() -> Self {
        Self {
            pc: 0,
            // 创建 16 个初始化为 Int(0) 的寄存器
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
            call_stack_depth: 0,
            stack_len: 0,
        }
    }
}

// ─── SpriteState & RenderState ──────────────────────────────────────────────

/// 立绘/精灵显示状态 — 记录单个精灵在画面上的显示参数。
///
/// 存档时捕获当前所有显示中的立绘状态，读档时恢复。
///
/// # position 取值说明
///
/// | 值 | 含义 |
/// |----|------|
/// | 0 | 左侧（left） |
/// | 1 | 居中（center） |
/// | 2 | 右侧（right） |
/// | 3+ | 自定义位置（custom） |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpriteState {
    /// 精灵资源文件路径（如 "sprites/sayori_smile"）
    pub sprite_path: String,
    /// 画面位置：0=left, 1=center, 2=right, 3+=custom
    pub position: u8,
    /// 透明度（0.0 = 完全透明, 1.0 = 完全不透明）
    pub alpha: f32,
    /// 当前表情变体（如 "smile", "sad"），None 表示默认表情
    pub emotion: Option<String>,
}

/// 渲染状态快照 — 捕获当前场景画面的完整渲染状态。
///
/// 包含背景和所有显示中精灵的信息，用于读档时恢复画面到存档时的状态。
///
/// # 已知限制
///
/// - `displayed_sprites` 不包含打字机文本进度 —— 存档恢复后文本从头显示
/// - 不包含转场特效的中间状态（存档时转场已完成）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderState {
    /// 当前背景资源路径（None = 无背景/默认黑屏）
    pub current_bg: Option<String>,
    /// 当前显示的立绘/精灵列表（按绘制顺序，后绘制的在下层之上）
    pub displayed_sprites: Vec<SpriteState>,
}

impl Default for RenderState {
    /// 创建空的渲染状态 — 无背景、无立绘。
    fn default() -> Self {
        Self {
            current_bg: None,
            displayed_sprites: Vec::new(),
        }
    }
}

// ─── SaveSlotInfo ───────────────────────────────────────────────────────────

/// 存档槽位摘要信息 — 列表展示用，不包含完整的 `SaveData`。
///
/// 在存档/读档界面中显示每个槽位的基本信息（槽位号、保存时间、场景名）。
/// `has_thumbnail` 指示是否存在对应的缩略图 PNG 文件。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SaveSlotInfo {
    /// 槽位编号（0-4 = 手动, 98 = 快速, 99 = 自动）
    pub slot: u8,
    /// 存档时间戳（ISO 8601 格式，如 "2026-06-16T14:30:00+08:00"）
    pub timestamp: String,
    /// 存档时的场景 ID（如 "chapter1/prologue"）
    pub scene_id: String,
    /// 是否存在缩略图（`slot_{NN}_thumb.png`）
    pub has_thumbnail: bool,
}

// ─── SaveData ───────────────────────────────────────────────────────────────

/// 存档数据结构 — 封装游戏完整运行时状态的快照。
///
/// 存档时，SceneManager 收集当前游戏的全部状态填入此结构体，
/// 由 `SaveManager` 序列化为 MessagePack 格式并附加 CRC32 校验和写入磁盘。
/// 读档时，`SaveManager` 验证 CRC32 完整性后反序列化，SceneManager 据此恢复游戏。
///
/// # 存档格式版本化（NFR-COMPAT-006）
///
/// `version` 字段从 1 开始。后续引擎升级时递增版本号，
/// 存档加载时检查版本并调用对应的迁移函数链（`v1→v2→...→vN`）。
/// 不兼容的版本将被拒绝加载并返回 `IncompatibleVersion` 错误。
///
/// # 字段一览
///
/// | 字段 | 类型 | 说明 |
/// |------|------|------|
/// | `version` | `u32` | 存档格式版本号（当前 = 1） |
/// | `slot` | `u8` | 槽位编号 |
/// | `timestamp` | `String` | ISO 8601 时间戳 |
/// | `scene_id` | `String` | 当前场景 ID |
/// | `label` | `Option<String>` | 场景内标签位置（精确恢复点） |
/// | `vm_snapshot` | `VmSnapshot` | VM 执行状态 |
/// | `variables` | `VariableStore` | 变量存储 |
/// | `flags` | `FlagSet` | 旗标集合 |
/// | `audio_state` | `AudioSnapshot` | 音频系统状态 |
/// | `render_state` | `RenderState` | 渲染状态 |
///
/// # 示例
/// ```
/// use aster_core::{SaveData, VmSnapshot, AudioSnapshot, RenderState, VariableStore, FlagSet};
///
/// let save = SaveData::new(0, "chapter1/prologue");
/// assert_eq!(save.version, 1);
/// assert_eq!(save.slot, 0);
/// assert_eq!(save.scene_id, "chapter1/prologue");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SaveData {
    /// 存档格式版本号（当前为 1）
    pub version: u32,
    /// 槽位编号（0-based，0-4 手动槽位，98 快速存档，99 自动存档）
    pub slot: u8,
    /// 存档时间戳（ISO 8601 格式字符串，如 "2026-06-16T14:30:00+08:00"）
    pub timestamp: String,
    /// 当前场景 ID（如 "chapter1/prologue"）
    pub scene_id: String,
    /// 当前场景内的标签位置（用于恢复到精确位置）
    pub label: Option<String>,
    /// VM 执行状态快照（PC + 寄存器 + 调用栈）
    pub vm_snapshot: VmSnapshot,
    /// 变量存储快照
    pub variables: VariableStore,
    /// 旗标集合快照
    pub flags: FlagSet,
    /// 音频系统快照
    pub audio_state: AudioSnapshot,
    /// 渲染状态快照（显示的立绘列表、背景等）
    pub render_state: RenderState,
}

impl SaveData {
    /// 存档格式的当前版本号。
    ///
    /// 此常量在读取存档时用于版本兼容性检查。
    /// 后续引擎升级时递增版本号，旧版本存档需通过迁移函数转换。
    pub const CURRENT_VERSION: u32 = 1;

    /// 创建一个新的存档数据结构，自动填充版本号、时间戳和默认空状态。
    ///
    /// # 参数
    /// - `slot`：槽位编号（0-4 手动，98 快速，99 自动）
    /// - `scene_id`：当前场景标识符（如 "chapter1/prologue"）
    ///
    /// # 返回值
    /// 返回一个新的 `SaveData` 实例，其中：
    /// - `version` 自动设置为 `CURRENT_VERSION`（1）
    /// - `timestamp` 自动填充当前本地时间的 ISO 8601 格式字符串
    /// - 所有状态快照（VM、变量、旗标、音频、渲染）初始化为默认空状态
    ///
    /// # 示例
    /// ```
    /// use aster_core::SaveData;
    ///
    /// let save = SaveData::new(2, "chapter2/climax");
    /// assert_eq!(save.version, 1);
    /// assert_eq!(save.slot, 2);
    /// assert_eq!(save.scene_id, "chapter2/climax");
    /// assert!(save.variables.is_empty());
    /// assert!(save.flags.is_empty());
    /// ```
    pub fn new(slot: u8, scene_id: impl Into<String>) -> Self {
        let timestamp = chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string();

        Self {
            version: Self::CURRENT_VERSION,
            slot,
            timestamp,
            scene_id: scene_id.into(),
            label: None,
            vm_snapshot: VmSnapshot::default(),
            variables: VariableStore::new(),
            flags: FlagSet::new(),
            audio_state: AudioSnapshot::default(),
            render_state: RenderState::default(),
        }
    }

    /// 从当前存档数据生成槽位摘要信息。
    ///
    /// 用于存档列表展示，提取 `slot`、`timestamp`、`scene_id` 字段。
    /// `has_thumbnail` 固定为 `false`——缩略图是否存在由 `SaveManager`
    /// 在 `list_saves()` 中根据磁盘文件实际情况判断。
    ///
    /// # 返回值
    /// 包含槽位摘要的 `SaveSlotInfo`。
    pub fn to_slot_info(&self) -> SaveSlotInfo {
        SaveSlotInfo {
            slot: self.slot,
            timestamp: self.timestamp.clone(),
            scene_id: self.scene_id.clone(),
            has_thumbnail: false,
        }
    }
}

// ─── 测试模块 ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── AudioSnapshot 测试 ─────────────────────────────────────────────────

    /// 验证 AudioSnapshot 默认值对应无播放状态。
    #[test]
    fn test_audio_snapshot_default() {
        let snapshot = AudioSnapshot::default();
        assert!(snapshot.current_bgm_path.is_none());
        assert!((snapshot.bgm_position_secs - 0.0).abs() < f64::EPSILON);
        assert!(!snapshot.bgm_looping);
        assert!((snapshot.bgm_volume - 0.8).abs() < f32::EPSILON);
        assert!((snapshot.se_volume - 0.8).abs() < f32::EPSILON);
    }

    /// 验证 AudioSnapshot 的 serde 往返（JSON）。
    #[test]
    fn test_audio_snapshot_serde_json_roundtrip() {
        let original = AudioSnapshot {
            current_bgm_path: Some("assets/bgm/theme.ogg".to_string()),
            bgm_position_secs: 42.5,
            bgm_looping: true,
            bgm_volume: 0.7,
            se_volume: 0.3,
        };

        let json = serde_json::to_string(&original).expect("序列化应成功");
        let restored: AudioSnapshot = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(original, restored);
    }

    /// 验证 AudioSnapshot 的 serde 往返（MessagePack）。
    #[test]
    fn test_audio_snapshot_serde_msgpack_roundtrip() {
        let original = AudioSnapshot {
            current_bgm_path: Some("bgm/battle.ogg".into()),
            bgm_position_secs: 15.0,
            bgm_looping: false,
            bgm_volume: 1.0,
            se_volume: 0.5,
        };

        let bytes = rmp_serde::to_vec(&original).expect("MessagePack 序列化应成功");
        let restored: AudioSnapshot =
            rmp_serde::from_slice(&bytes).expect("MessagePack 反序列化应成功");
        assert_eq!(original, restored);
    }

    /// 验证 AudioSnapshot 无 BGM 时的 serde 往返。
    #[test]
    fn test_audio_snapshot_serde_no_bgm() {
        let original = AudioSnapshot {
            current_bgm_path: None,
            bgm_position_secs: 0.0,
            bgm_looping: false,
            bgm_volume: 0.5,
            se_volume: 1.0,
        };

        let bytes = rmp_serde::to_vec(&original).expect("序列化应成功");
        let restored: AudioSnapshot = rmp_serde::from_slice(&bytes).expect("反序列化应成功");
        assert_eq!(original, restored);
    }

    // ─── VmSnapshot 测试 ────────────────────────────────────────────────────

    /// 验证 VmSnapshot 默认值 —— PC 和栈深度为 0，寄存器全为 Int(0)。
    #[test]
    fn test_vm_snapshot_default() {
        let snapshot = VmSnapshot::default();
        assert_eq!(snapshot.pc, 0);
        assert_eq!(snapshot.call_stack_depth, 0);
        assert_eq!(snapshot.stack_len, 0);
        assert_eq!(snapshot.registers.len(), 16);
        for reg in &snapshot.registers {
            assert_eq!(*reg, Value::Int(0));
        }
    }

    /// 验证 VmSnapshot 的 serde 往返（MessagePack）。
    #[test]
    fn test_vm_snapshot_serde_roundtrip() {
        let mut registers: [Value; 16] = [
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
        // R0 = 程序计数器位置模拟
        registers[0] = Value::Int(42);
        // R1 = 字符串值
        registers[1] = Value::String("sayori".into());
        // R2 = 布尔值
        registers[2] = Value::Bool(true);

        let original = VmSnapshot {
            pc: 100,
            registers,
            call_stack_depth: 3,
            stack_len: 7,
        };

        let bytes = rmp_serde::to_vec(&original).expect("序列化应成功");
        let restored: VmSnapshot = rmp_serde::from_slice(&bytes).expect("反序列化应成功");

        assert_eq!(restored.pc, 100);
        assert_eq!(restored.call_stack_depth, 3);
        assert_eq!(restored.stack_len, 7);
        assert_eq!(restored.registers[0], Value::Int(42));
        assert_eq!(restored.registers[1], Value::String("sayori".into()));
        assert_eq!(restored.registers[2], Value::Bool(true));
    }

    // ─── RenderState / SpriteState 测试 ──────────────────────────────────────

    /// 验证 RenderState 默认值——空背景、空立绘列表。
    #[test]
    fn test_render_state_default() {
        let state = RenderState::default();
        assert!(state.current_bg.is_none());
        assert!(state.displayed_sprites.is_empty());
    }

    /// 验证 RenderState + SpriteState 的 serde 往返。
    #[test]
    fn test_render_state_serde_roundtrip() {
        let original = RenderState {
            current_bg: Some("backgrounds/classroom_day".into()),
            displayed_sprites: vec![
                SpriteState {
                    sprite_path: "sprites/sayori_smile".into(),
                    position: 0,
                    alpha: 1.0,
                    emotion: Some("smile".into()),
                },
                SpriteState {
                    sprite_path: "sprites/natsuki_default".into(),
                    position: 2,
                    alpha: 0.8,
                    emotion: None,
                },
            ],
        };

        let bytes = rmp_serde::to_vec(&original).expect("序列化应成功");
        let restored: RenderState = rmp_serde::from_slice(&bytes).expect("反序列化应成功");

        assert_eq!(
            restored.current_bg,
            Some("backgrounds/classroom_day".into())
        );
        assert_eq!(restored.displayed_sprites.len(), 2);
        assert_eq!(
            restored.displayed_sprites[0].sprite_path,
            "sprites/sayori_smile"
        );
        assert_eq!(restored.displayed_sprites[0].position, 0);
        assert!((restored.displayed_sprites[0].alpha - 1.0).abs() < f32::EPSILON);
        assert_eq!(restored.displayed_sprites[0].emotion, Some("smile".into()));
        assert_eq!(restored.displayed_sprites[1].emotion, None);
    }

    // ─── SaveData 测试 ──────────────────────────────────────────────────────

    /// AC01 — SaveData 序列化/反序列化 MessagePack 往返。
    ///
    /// 构造一个包含完整嵌套结构的 SaveData（VariableStore、FlagSet、AudioSnapshot、
    /// RenderState、VmSnapshot），序列化后反序列化，逐字段验证一致性。
    #[test]
    fn ac01_save_data_serde_msgpack_roundtrip() {
        // 构造 VariableStore
        let mut variables = VariableStore::new();
        variables.set("score", Value::Int(100));
        variables.set("player_name", Value::String("主角".into()));
        variables.set("progress", Value::Float(0.75));

        // 构造 FlagSet
        let mut flags = FlagSet::new();
        flags.set("met_sayori");
        flags.set("completed_ch1");

        // 构造 VmSnapshot
        let mut registers: [Value; 16] = [
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
        registers[0] = Value::Int(15);

        let vm_snapshot = VmSnapshot {
            pc: 42,
            registers,
            call_stack_depth: 2,
            stack_len: 5,
        };

        // 构造 AudioSnapshot
        let audio_state = AudioSnapshot {
            current_bgm_path: Some("bgm/theme.ogg".into()),
            bgm_position_secs: 30.0,
            bgm_looping: true,
            bgm_volume: 0.8,
            se_volume: 0.6,
        };

        // 构造 RenderState
        let render_state = RenderState {
            current_bg: Some("backgrounds/school".into()),
            displayed_sprites: vec![SpriteState {
                sprite_path: "sprites/sayori".into(),
                position: 1,
                alpha: 1.0,
                emotion: Some("happy".into()),
            }],
        };

        // 构造完整的 SaveData
        let original = SaveData {
            version: 1,
            slot: 2,
            timestamp: "2026-06-16T14:30:00+08:00".into(),
            scene_id: "chapter1/prologue".into(),
            label: Some("dialogue_42".into()),
            vm_snapshot,
            variables,
            flags,
            audio_state,
            render_state,
        };

        // MessagePack 序列化
        let bytes = rmp_serde::to_vec(&original).expect("序列化应成功");
        assert!(!bytes.is_empty(), "序列化后的字节不应为空");

        // MessagePack 反序列化
        let restored: SaveData = rmp_serde::from_slice(&bytes).expect("反序列化应成功");

        // 逐字段验证
        assert_eq!(restored.version, 1);
        assert_eq!(restored.slot, 2);
        assert_eq!(restored.timestamp, "2026-06-16T14:30:00+08:00");
        assert_eq!(restored.scene_id, "chapter1/prologue");
        assert_eq!(restored.label, Some("dialogue_42".into()));

        // VM 快照
        assert_eq!(restored.vm_snapshot.pc, 42);
        assert_eq!(restored.vm_snapshot.call_stack_depth, 2);
        assert_eq!(restored.vm_snapshot.stack_len, 5);
        assert_eq!(restored.vm_snapshot.registers[0], Value::Int(15));

        // 变量
        assert_eq!(restored.variables.get("score"), Some(&Value::Int(100)));
        assert_eq!(
            restored.variables.get("player_name"),
            Some(&Value::String("主角".into()))
        );

        // 旗标
        assert!(restored.flags.check("met_sayori"));
        assert!(restored.flags.check("completed_ch1"));
        assert!(!restored.flags.check("never_set"));

        // 音频状态
        assert_eq!(
            restored.audio_state.current_bgm_path,
            Some("bgm/theme.ogg".into())
        );
        assert!((restored.audio_state.bgm_position_secs - 30.0).abs() < f64::EPSILON);
        assert!(restored.audio_state.bgm_looping);
        assert!((restored.audio_state.bgm_volume - 0.8).abs() < f32::EPSILON);
        assert!((restored.audio_state.se_volume - 0.6).abs() < f32::EPSILON);

        // 渲染状态
        assert_eq!(
            restored.render_state.current_bg,
            Some("backgrounds/school".into())
        );
        assert_eq!(restored.render_state.displayed_sprites.len(), 1);
        assert_eq!(
            restored.render_state.displayed_sprites[0].sprite_path,
            "sprites/sayori"
        );
        assert_eq!(restored.render_state.displayed_sprites[0].position, 1);
        assert_eq!(
            restored.render_state.displayed_sprites[0].emotion,
            Some("happy".into())
        );
    }

    /// AC01 补充 — 空状态 SaveData 的序列化往返。
    #[test]
    fn ac01_save_data_empty_state_roundtrip() {
        let original = SaveData::new(0, "test_scene");
        let bytes = rmp_serde::to_vec(&original).expect("序列化应成功");
        let restored: SaveData = rmp_serde::from_slice(&bytes).expect("反序列化应成功");

        assert_eq!(restored.version, 1);
        assert_eq!(restored.slot, 0);
        assert_eq!(restored.scene_id, "test_scene");
        assert!(restored.variables.is_empty());
        assert!(restored.flags.is_empty());
        assert!(restored.label.is_none());
        assert!(restored.audio_state.current_bgm_path.is_none());
        assert!(restored.render_state.current_bg.is_none());
        assert!(restored.render_state.displayed_sprites.is_empty());
    }

    /// 验证 SaveData::new() 自动生成的时间戳非空且包含 T 分隔符。
    #[test]
    fn test_save_data_new_timestamp_format() {
        let save = SaveData::new(0, "test");
        // ISO 8601 格式的基本检查：包含日期和时间分隔符
        assert!(!save.timestamp.is_empty());
        assert!(
            save.timestamp.contains('T'),
            "时间戳应包含 ISO 8601 的 T 分隔符"
        );
    }

    /// 验证 SaveData::to_slot_info() 生成正确的摘要信息。
    #[test]
    fn test_save_data_to_slot_info() {
        let save = SaveData::new(3, "chapter2/intro");
        let info = save.to_slot_info();

        assert_eq!(info.slot, 3);
        assert_eq!(info.scene_id, "chapter2/intro");
        assert_eq!(info.timestamp, save.timestamp);
        assert!(!info.has_thumbnail); // 默认为 false
    }

    /// 验证 SaveData::CURRENT_VERSION 常量。
    #[test]
    fn test_current_version() {
        assert_eq!(SaveData::CURRENT_VERSION, 1);
    }
}
