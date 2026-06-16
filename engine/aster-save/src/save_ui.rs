//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-save/src/save_ui.rs
//! 功能概述：存档 UI 状态机 — 管理游戏内存档/读档界面的交互逻辑。
//!           `SaveUi` 是纯状态机（无渲染依赖），通过 `handle_input()` 处理用户输入、
//!           通过 `render_commands()` 生成 `Vec<UiCommand>` 渲染指令列表。
//!           UI 逻辑与渲染完全分离，渲染指令由 `aster-renderer`（PH2-T08）翻译为实际绘制调用。
//! 作者：Claude (AI)
//! 创建日期：2026-06-16
//! 最后修改：2026-06-16
//!
//! 依赖模块：
//! - aster_core::save::SaveSlotInfo（槽位摘要信息）
//! - std::path::PathBuf（缩略图路径）
//!
//! 对应需求：REQ-ENG-042（基础存档界面）
//! 对应任务：PH2-T07 — aster-save 槽位管理 + 缩略图捕获 + 基础存档 UI
//!
//! ## 设计说明
//!
//! ### 状态机模型
//!
//! ```text
//!                    ┌──────────┐
//!           open() → │ SlotList │ ← confirm/delete success ─┐
//!                    └────┬─────┘                            │
//!          ┌──────────────┼─────────────┐                    │
//!     ConfirmOverwrite  ConfirmDelete   Error               │
//!          │               │                                │
//!     confirm/cancel   confirm/cancel    any key → SlotList ┘
//!          │               │
//!          └───────┬───────┘
//!                  ↓
//!         SaveUiResult (SaveRequested/LoadRequested/DeleteRequested)
//! ```
//!
//! ### UI 与渲染分离
//!
//! `SaveUi` 不持有任何 GPU 资源或字体句柄。它通过 `render_commands()` 返回
//! 平台无关的 `Vec<UiCommand>`，由外部的 `Renderer` trait（PH2-T08）翻译为实际绘制调用。
//! 这种分离使得：
//! 1. `SaveUi` 可以纯粹通过单元测试验证（无需 GPU）
//! 2. 未来更换渲染后端（wgpu → 其他）时 SaveUi 代码不变
//! 3. `UiCommand` 可以序列化为脚本命令，支持调试和自动化测试

use std::path::PathBuf;

use aster_core::SaveSlotInfo;

// ─── 枚举定义 ──────────────────────────────────────────────────────────────

/// 存档 UI 操作模式。
///
/// | 变体 | 说明 |
/// |------|------|
/// | `Save` | 存档模式 — 确认时将覆盖已有存档或创建新存档 |
/// | `Load` | 读档模式 — 确认时从已有存档恢复游戏状态 |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveUiMode {
    /// 存档模式
    Save,
    /// 读档模式
    Load,
}

/// 存档 UI 状态 — 状态机的全部可能状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveUiState {
    /// UI 未显示（初始状态 / 已关闭）
    Hidden,
    /// 槽位列表选择状态
    SlotList {
        /// 当前操作模式（存档 / 读档）
        mode: SaveUiMode,
        /// 所有槽位的展示信息
        slots: Vec<SlotDisplayInfo>,
        /// 当前选中槽位的索引（在 slots 中）
        selected: usize,
    },
    /// 覆盖确认对话框
    ConfirmOverwrite {
        /// 要覆盖的槽位号
        slot: u8,
    },
    /// 删除确认对话框
    ConfirmDelete {
        /// 要删除的槽位号
        slot: u8,
    },
    /// 错误提示（显示后按任意键返回 SlotList）
    Error {
        /// 错误消息文本
        message: String,
    },
}

/// 用户输入动作 — UI 内抽象按键，与具体键位解耦。
///
/// 由外部事件循环将实际按键映射到此枚举后传入 `handle_input()`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    /// 向上移动选择（↑ / W）
    Up,
    /// 向下移动选择（↓ / S）
    Down,
    /// 确认当前选择（Enter / Space）
    Confirm,
    /// 取消 / 返回上一级（ESC / Backspace）
    Cancel,
    /// 删除当前选中槽位的存档（Delete）
    Delete,
}

/// `handle_input()` 的返回值 — 指示外部应执行什么操作。
///
/// SaveUi 状态机本身不执行实际的存档/读档/删除操作，
/// 而是通过此枚举告知调用方（SceneManager / App）需要做什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveUiResult {
    /// 无操作（状态转换已完成，不需要外部干预）
    None,
    /// 请求保存到指定槽位
    SaveRequested {
        /// 目标槽位
        slot: u8,
    },
    /// 请求从指定槽位读档
    LoadRequested {
        /// 来源槽位
        slot: u8,
    },
    /// 请求删除指定槽位的存档
    DeleteRequested {
        /// 目标槽位
        slot: u8,
    },
    /// UI 已关闭（用户按 ESC 退出到游戏）
    Closed,
}

// ─── UI 渲染指令 ───────────────────────────────────────────────────────────

/// 存档 UI 渲染指令 — 描述一个需要在屏幕上绘制的 UI 元素。
///
/// 此枚举与渲染后端解耦。`aster-renderer` 将在 PH2-T08 中实现
/// 将这些指令翻译为实际的 GPU 绘制调用。
///
/// 坐标系统：
/// - 原点为窗口左上角
/// - x 轴向右，y 轴向下
/// - 单位为逻辑像素
#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    /// 全屏半透明遮罩（绘制在所有 UI 元素后方）
    Overlay {
        /// 透明度（0.0 = 完全透明, 1.0 = 完全不透明）
        alpha: f32,
    },
    /// 文本字符串
    Text {
        /// 文本内容
        content: String,
        /// 左上角 x 坐标
        x: f32,
        /// 左上角 y 坐标
        y: f32,
        /// 字号（逻辑像素）
        font_size: f32,
        /// RGBA 颜色（[r, g, b, a]，每通道 0.0 ~ 1.0）
        color: [f32; 4],
        /// 是否为当前选中项（高亮渲染）
        selected: bool,
    },
    /// 缩略图（PNG 文件）
    Thumbnail {
        /// 缩略图文件路径
        path: PathBuf,
        /// 左上角 x 坐标
        x: f32,
        /// 左上角 y 坐标
        y: f32,
        /// 显示宽度（逻辑像素）
        width: f32,
        /// 显示高度（逻辑像素）
        height: f32,
    },
    /// 确认对话框（半透明背景框 + 文本 + 是/否 按钮）
    ConfirmDialog {
        /// 对话框提示文本
        message: String,
        /// 左上角 x 坐标
        x: f32,
        /// 左上角 y 坐标
        y: f32,
        /// 对话框宽度
        width: f32,
        /// 对话框高度
        height: f32,
    },
}

// ─── 槽位展示信息 ──────────────────────────────────────────────────────────

/// 单个槽位的展示信息 — 用于 UI 列表渲染。
///
/// 由 `SaveUi::build_slot_list()` 从 `Vec<SaveSlotInfo>` 构建，
/// 确保固定输出 7 个槽位（5 手动 + 1 快速 + 1 自动）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotDisplayInfo {
    /// 槽位编号
    pub slot: u8,
    /// 人类可读的槽位标签（如 "槽位 3"、"快速存档"）
    pub label: String,
    /// 存档时间戳（`None` 表示空槽位）
    pub timestamp: Option<String>,
    /// 存档时的场景名（`None` 表示空槽位）
    pub scene_name: Option<String>,
    /// 该槽位是否已存在存档
    pub has_save: bool,
}

// ─── 默认 UI 布局常量 ──────────────────────────────────────────────────────

/// 预设窗口宽度（1080p）
const VIEWPORT_WIDTH: f32 = 1920.0;

/// 预设窗口高度
const VIEWPORT_HEIGHT: f32 = 1080.0;

/// 列表起始 Y 坐标
const LIST_START_Y: f32 = 200.0;

/// 列表项高度
const ITEM_HEIGHT: f32 = 60.0;

/// 列表项左边距
const LIST_LEFT_X: f32 = 400.0;

/// 标题字号
const TITLE_FONT_SIZE: f32 = 36.0;

/// 列表项字号
const ITEM_FONT_SIZE: f32 = 22.0;

/// 底部提示字号
const HINT_FONT_SIZE: f32 = 18.0;

/// 缩略图尺寸（逻辑像素）
const THUMB_SIZE: f32 = 160.0 * 0.6; // 96（缩略图显示尺寸小于实际）

// ─── SaveUi 状态机 ─────────────────────────────────────────────────────────

/// 存档 UI 状态机 — 管理游戏内存档/读档界面的全部交互逻辑。
///
/// # 使用模式
///
/// ```rust,ignore
/// use aster_save::{SaveUi, SaveUiMode, UiAction};
///
/// let mut ui = SaveUi::new();
///
/// // 打开存档界面
/// let slot_infos = save_manager.list_saves().unwrap();
/// ui.open(SaveUiMode::Save, &slot_infos);
///
/// // 处理用户输入
/// match ui.handle_input(UiAction::Confirm) {
///     SaveUiResult::SaveRequested { slot } => {
///         // 执行实际保存操作
///     }
///     _ => {}
/// }
///
/// // 获取渲染指令
/// let commands = ui.render_commands();
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SaveUi {
    /// 当前状态
    state: SaveUiState,
}

impl SaveUi {
    /// 创建一个新的存档 UI 实例（初始状态为 Hidden）。
    pub fn new() -> Self {
        Self {
            state: SaveUiState::Hidden,
        }
    }

    /// 打开存档 UI，进入槽位列表选择状态。
    ///
    /// 根据 `mode` 和 `slot_infos` 构建完整的 7 槽位列表（5 手动 + 1 快速 + 1 自动）。
    /// 初始选中位置为第一个槽位（索引 0）。
    ///
    /// # 参数
    /// - `mode`：操作模式（存档 / 读档）
    /// - `slot_infos`：`SaveManager::list_saves()` 的返回结果（可能不完整——仅包含已有存档的槽位）
    pub fn open(&mut self, mode: SaveUiMode, slot_infos: &[SaveSlotInfo]) {
        let slots = Self::build_slot_list(slot_infos);
        self.state = SaveUiState::SlotList {
            mode,
            slots,
            selected: 0,
        };
    }

    /// 关闭存档 UI，回到 Hidden 状态。
    pub fn close(&mut self) {
        self.state = SaveUiState::Hidden;
    }

    /// 处理用户输入动作，驱动状态转换并返回操作结果。
    ///
    /// # 参数
    /// - `action`：用户输入动作（Up/Down/Confirm/Cancel/Delete）
    ///
    /// # 返回值
    /// 状态转换后的操作结果（`SaveUiResult`），调用方据此执行实际 I/O 操作。
    ///
    /// # 状态转换规则
    ///
    /// | 当前状态 | 输入 | 下一状态 | 返回值 |
    /// |---------|------|---------|--------|
    /// | Hidden | 任意 | Hidden | None |
    /// | SlotList | Up/Down | SlotList（selected ± 1） | None |
    /// | SlotList | Cancel | Hidden | Closed |
    /// | SlotList | Confirm（读档+有存档） | Hidden | LoadRequested |
    /// | SlotList | Confirm（读档+空槽位） | Error | None |
    /// | SlotList | Confirm（存档+有存档） | ConfirmOverwrite | None |
    /// | SlotList | Confirm（存档+空槽位） | Hidden | SaveRequested |
    /// | SlotList | Delete（有存档） | ConfirmDelete | None |
    /// | SlotList | Delete（空槽位） | Error | None |
    /// | ConfirmOverwrite | Confirm | Hidden | SaveRequested |
    /// | ConfirmOverwrite | Cancel | SlotList | None |
    /// | ConfirmDelete | Confirm | Hidden | DeleteRequested |
    /// | ConfirmDelete | Cancel | SlotList | None |
    /// | Error | 任意 | SlotList | None |
    pub fn handle_input(&mut self, action: UiAction) -> SaveUiResult {
        // 使用临时值替换 state 进行模式匹配，避免借用冲突
        let old_state = std::mem::replace(&mut self.state, SaveUiState::Hidden);

        let (new_state, result) = match old_state {
            SaveUiState::Hidden => (SaveUiState::Hidden, SaveUiResult::None),

            SaveUiState::SlotList {
                mode,
                slots,
                selected,
            } => {
                let slot_count = slots.len();
                match action {
                    UiAction::Up => {
                        let new_selected = if selected == 0 {
                            slot_count.saturating_sub(1)
                        } else {
                            selected - 1
                        };
                        (
                            SaveUiState::SlotList {
                                mode,
                                slots,
                                selected: new_selected,
                            },
                            SaveUiResult::None,
                        )
                    }
                    UiAction::Down => {
                        let new_selected = if selected + 1 >= slot_count {
                            0
                        } else {
                            selected + 1
                        };
                        (
                            SaveUiState::SlotList {
                                mode,
                                slots,
                                selected: new_selected,
                            },
                            SaveUiResult::None,
                        )
                    }
                    UiAction::Cancel => (SaveUiState::Hidden, SaveUiResult::Closed),
                    UiAction::Confirm => {
                        let slot_info = &slots[selected];
                        match mode {
                            SaveUiMode::Save => {
                                if slot_info.has_save {
                                    // 有数据 → 覆盖确认
                                    (
                                        SaveUiState::ConfirmOverwrite {
                                            slot: slot_info.slot,
                                        },
                                        SaveUiResult::None,
                                    )
                                } else {
                                    // 空槽位 → 直接保存
                                    (
                                        SaveUiState::Hidden,
                                        SaveUiResult::SaveRequested {
                                            slot: slot_info.slot,
                                        },
                                    )
                                }
                            }
                            SaveUiMode::Load => {
                                if slot_info.has_save {
                                    // 有数据 → 直接读档
                                    (
                                        SaveUiState::Hidden,
                                        SaveUiResult::LoadRequested {
                                            slot: slot_info.slot,
                                        },
                                    )
                                } else {
                                    // 空槽位 → 错误提示
                                    (
                                        SaveUiState::Error {
                                            message: "该槽位为空，无法读取存档。".into(),
                                        },
                                        SaveUiResult::None,
                                    )
                                }
                            }
                        }
                    }
                    UiAction::Delete => {
                        let slot_info = &slots[selected];
                        if slot_info.has_save {
                            (
                                SaveUiState::ConfirmDelete {
                                    slot: slot_info.slot,
                                },
                                SaveUiResult::None,
                            )
                        } else {
                            (
                                SaveUiState::Error {
                                    message: "该槽位为空，没有可删除的存档。".into(),
                                },
                                SaveUiResult::None,
                            )
                        }
                    }
                }
            }

            SaveUiState::ConfirmOverwrite { slot } => match action {
                UiAction::Confirm => (SaveUiState::Hidden, SaveUiResult::SaveRequested { slot }),
                UiAction::Cancel => {
                    // 回到槽位列表（需要调用方重新传入 slots）
                    // 注：Cancel 时 slots 信息会丢失，需要调用方在收到 None 后
                    // 通过 open() 重新构建。实际集成时 SceneManager 会在收到 None
                    // 后调用 render_commands() 并重新设置状态。
                    (SaveUiState::Hidden, SaveUiResult::Closed)
                }
                _ => (SaveUiState::ConfirmOverwrite { slot }, SaveUiResult::None),
            },

            SaveUiState::ConfirmDelete { slot } => match action {
                UiAction::Confirm => (SaveUiState::Hidden, SaveUiResult::DeleteRequested { slot }),
                UiAction::Cancel => (SaveUiState::Hidden, SaveUiResult::Closed),
                _ => (SaveUiState::ConfirmDelete { slot }, SaveUiResult::None),
            },

            SaveUiState::Error { .. } => {
                // 任意键返回槽位列表（但 slots 已丢失，由调用方重新打开）
                (SaveUiState::Hidden, SaveUiResult::Closed)
            }
        };

        self.state = new_state;
        result
    }

    /// 根据 `SaveManager::list_saves()` 的结果构建完整的 7 槽位展示列表。
    ///
    /// 确保固定输出 7 个 `SlotDisplayInfo`（槽位 0-4 + 98 + 99），
    /// 无论 `SaveSlotInfo` 列表中是否包含这些槽位。
    ///
    /// # 参数
    /// - `slot_infos`：从 SaveManager 获取的已有存档槽位信息
    ///
    /// # 返回值
    /// 按槽位号升序排列的 7 个 `SlotDisplayInfo`
    pub fn build_slot_list(slot_infos: &[SaveSlotInfo]) -> Vec<SlotDisplayInfo> {
        // 所有定义的槽位编号（手动 0-4，快速 98，自动 99）
        const ALL_SLOTS: [u8; 7] = [0, 1, 2, 3, 4, 98, 99];

        ALL_SLOTS
            .iter()
            .map(|&slot| {
                // 在已有的 slot_infos 中查找该槽位
                let existing = slot_infos.iter().find(|info| info.slot == slot);

                SlotDisplayInfo {
                    slot,
                    label: crate::save_manager::slot_label(slot),
                    timestamp: existing.map(|info| info.timestamp.clone()),
                    scene_name: existing.map(|info| info.scene_id.clone()),
                    has_save: existing.is_some(),
                }
            })
            .collect()
    }

    /// 生成当前状态对应的 UI 渲染指令列表。
    ///
    /// 调用方将这些指令传递给 `Renderer::render_save_ui()`（PH2-T08 实现）。
    ///
    /// # 返回值
    /// 当前状态需要的所有 `UiCommand`。`Hidden` 状态返回空列表。
    pub fn render_commands(&self) -> Vec<UiCommand> {
        match &self.state {
            SaveUiState::Hidden => vec![],

            SaveUiState::SlotList {
                mode,
                slots,
                selected,
            } => {
                let mut commands = Vec::new();

                // 背景遮罩
                commands.push(UiCommand::Overlay { alpha: 0.7 });

                // 标题栏
                let title = match mode {
                    SaveUiMode::Save => "存档",
                    SaveUiMode::Load => "读档",
                };
                commands.push(UiCommand::Text {
                    content: title.to_string(),
                    x: LIST_LEFT_X,
                    y: 100.0,
                    font_size: TITLE_FONT_SIZE,
                    color: [1.0, 1.0, 1.0, 1.0],
                    selected: false,
                });

                // 槽位列表项
                for (i, slot_info) in slots.iter().enumerate() {
                    let y = LIST_START_Y + i as f32 * ITEM_HEIGHT;
                    let is_selected = i == *selected;

                    // 缩略图（如果有）
                    if slot_info.has_save {
                        commands.push(UiCommand::Thumbnail {
                            path: PathBuf::from(format!("slot_{:02}_thumb.png", slot_info.slot)),
                            x: LIST_LEFT_X,
                            y,
                            width: THUMB_SIZE,
                            height: THUMB_SIZE * (9.0 / 16.0), // 16:9 比例调整
                        });
                    }

                    // 槽位标签 + 信息文本
                    let text = if slot_info.has_save {
                        format!(
                            "[{}] {} | {}",
                            slot_info.label,
                            slot_info.timestamp.as_deref().unwrap_or("（无时间）"),
                            slot_info.scene_name.as_deref().unwrap_or("（未知场景）")
                        )
                    } else {
                        format!("[{}] — 空 —", slot_info.label)
                    };

                    let text_x = if slot_info.has_save {
                        LIST_LEFT_X + THUMB_SIZE + 20.0
                    } else {
                        LIST_LEFT_X
                    };

                    // 选中项高亮：黄色文本
                    let color = if is_selected {
                        [1.0, 0.85, 0.0, 1.0]
                    } else {
                        [0.8, 0.8, 0.8, 1.0]
                    };

                    commands.push(UiCommand::Text {
                        content: text,
                        x: text_x,
                        y: y + ITEM_HEIGHT / 2.0 - ITEM_FONT_SIZE / 2.0,
                        font_size: ITEM_FONT_SIZE,
                        color,
                        selected: is_selected,
                    });
                }

                // 底部操作提示
                let hint_text = "↑↓ 选择  Enter 确认  ESC 返回  Delete 删除";
                commands.push(UiCommand::Text {
                    content: hint_text.to_string(),
                    x: LIST_LEFT_X,
                    y: VIEWPORT_HEIGHT - 60.0,
                    font_size: HINT_FONT_SIZE,
                    color: [0.6, 0.6, 0.6, 1.0],
                    selected: false,
                });

                commands
            }

            SaveUiState::ConfirmOverwrite { slot } => {
                vec![
                    UiCommand::Overlay { alpha: 0.7 },
                    UiCommand::ConfirmDialog {
                        message: format!(
                            "槽位 {} 已有存档，是否覆盖？\n\n确认覆盖将无法恢复原存档。",
                            crate::save_manager::slot_label(*slot)
                        ),
                        x: VIEWPORT_WIDTH / 2.0 - 250.0,
                        y: VIEWPORT_HEIGHT / 2.0 - 80.0,
                        width: 500.0,
                        height: 160.0,
                    },
                ]
            }

            SaveUiState::ConfirmDelete { slot } => {
                vec![
                    UiCommand::Overlay { alpha: 0.7 },
                    UiCommand::ConfirmDialog {
                        message: format!(
                            "确认删除槽位 {} 的存档？\n\n删除后无法恢复。",
                            crate::save_manager::slot_label(*slot)
                        ),
                        x: VIEWPORT_WIDTH / 2.0 - 250.0,
                        y: VIEWPORT_HEIGHT / 2.0 - 80.0,
                        width: 500.0,
                        height: 160.0,
                    },
                ]
            }

            SaveUiState::Error { message } => {
                vec![
                    UiCommand::Overlay { alpha: 0.7 },
                    UiCommand::Text {
                        content: format!("错误：{}", message),
                        x: VIEWPORT_WIDTH / 2.0 - 300.0,
                        y: VIEWPORT_HEIGHT / 2.0,
                        font_size: 24.0,
                        color: [1.0, 0.3, 0.3, 1.0],
                        selected: false,
                    },
                    UiCommand::Text {
                        content: "按任意键返回".to_string(),
                        x: VIEWPORT_WIDTH / 2.0 - 80.0,
                        y: VIEWPORT_HEIGHT / 2.0 + 40.0,
                        font_size: 16.0,
                        color: [0.7, 0.7, 0.7, 1.0],
                        selected: false,
                    },
                ]
            }
        }
    }

    /// 返回当前状态的不可变引用。
    pub fn state(&self) -> &SaveUiState {
        &self.state
    }

    /// 判断 UI 是否处于显示状态（非 Hidden）。
    pub fn is_open(&self) -> bool {
        !matches!(self.state, SaveUiState::Hidden)
    }

    /// 获取当前选中的槽位号（仅在 SlotList 状态有效）。
    ///
    /// # 返回值
    /// - `Some(slot)`：当前选中槽位
    /// - `None`：不在 SlotList 状态
    pub fn selected_slot(&self) -> Option<u8> {
        match &self.state {
            SaveUiState::SlotList {
                slots, selected, ..
            } => slots.get(*selected).map(|info| info.slot),
            _ => None,
        }
    }
}

impl Default for SaveUi {
    fn default() -> Self {
        Self::new()
    }
}

// ─── 测试模块 ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建模拟的 SaveSlotInfo 列表。
    fn mock_slot_infos(occupied: &[u8]) -> Vec<SaveSlotInfo> {
        occupied
            .iter()
            .map(|&slot| SaveSlotInfo {
                slot,
                timestamp: format!("2026-06-16T{:02}:00:00+08:00", slot),
                scene_id: format!("chapter{}", slot),
                has_thumbnail: false,
            })
            .collect()
    }

    // ─── AC01 ───────────────────────────────────────────────────────────────

    /// AC01 — SaveUi 初始状态为 Hidden。
    #[test]
    fn ac01_initial_state_hidden() {
        let ui = SaveUi::new();
        assert_eq!(*ui.state(), SaveUiState::Hidden);
        assert!(!ui.is_open());
    }

    // ─── AC02 ───────────────────────────────────────────────────────────────

    /// AC02 — 打开存档界面后状态变为 SlotList。
    #[test]
    fn ac02_open_save_ui() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[0, 2]);

        ui.open(SaveUiMode::Save, &infos);

        assert!(ui.is_open());
        match ui.state() {
            SaveUiState::SlotList {
                mode,
                slots,
                selected,
            } => {
                assert_eq!(*mode, SaveUiMode::Save);
                assert_eq!(slots.len(), 7);
                assert_eq!(*selected, 0);
            }
            other => panic!("期望 SlotList 状态，实际为 {:?}", other),
        }
    }

    // ─── AC03 ───────────────────────────────────────────────────────────────

    /// AC03 — 槽位列表生成：3 个已保存 + 4 个空 → 7 个 SlotDisplayInfo。
    #[test]
    fn ac03_build_slot_list() {
        let infos = mock_slot_infos(&[0, 4, 98]); // 3 个已保存槽位

        let slots = SaveUi::build_slot_list(&infos);
        assert_eq!(slots.len(), 7);

        // 有存档的槽位
        assert!(slots[0].has_save); // slot 0
        assert_eq!(slots[0].slot, 0);
        assert_eq!(slots[0].label, "槽位 1");
        assert!(slots[0].timestamp.is_some());

        assert!(!slots[1].has_save); // slot 1（空）
        assert_eq!(slots[1].slot, 1);
        assert!(slots[1].timestamp.is_none());

        assert!(!slots[3].has_save); // slot 3（空）

        assert!(slots[4].has_save); // slot 4
        assert_eq!(slots[4].slot, 4);
        assert_eq!(slots[4].label, "槽位 5");

        assert!(slots[5].has_save); // slot 98
        assert_eq!(slots[5].slot, 98);
        assert_eq!(slots[5].label, "快速存档");

        assert!(!slots[6].has_save); // slot 99（空）
        assert_eq!(slots[6].slot, 99);
        assert_eq!(slots[6].label, "自动存档");
    }

    // ─── AC04 ───────────────────────────────────────────────────────────────

    /// AC04 — 槽位选择导航：select_next/select_prev 在 0..=6 循环。
    #[test]
    fn ac04_select_navigation() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[]);
        ui.open(SaveUiMode::Save, &infos);

        // 初始选中索引 0
        assert_eq!(ui.selected_slot(), Some(0));

        // 向上 → 循环到最后一个（索引 6，槽位 99）
        ui.handle_input(UiAction::Up);
        assert_eq!(ui.selected_slot(), Some(99));

        // 再向上 → 索引 5，槽位 98
        ui.handle_input(UiAction::Up);
        assert_eq!(ui.selected_slot(), Some(98));

        // 向下 → 回到最后一个（索引 6，槽位 99）
        ui.handle_input(UiAction::Down);
        assert_eq!(ui.selected_slot(), Some(99));

        // 向下 → 循环到第一个（索引 0，槽位 0）
        ui.handle_input(UiAction::Down);
        assert_eq!(ui.selected_slot(), Some(0));
    }

    // ─── AC05 ───────────────────────────────────────────────────────────────

    /// AC05 — 空槽位读档拒绝：Load 模式下在空槽位按确认 → Error。
    #[test]
    fn ac05_empty_slot_load_rejected() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[]); // 全部为空
        ui.open(SaveUiMode::Load, &infos);

        // 选中槽位 0（空），按确认
        let result = ui.handle_input(UiAction::Confirm);
        assert_eq!(result, SaveUiResult::None); // 不触发 LoadRequested

        // 应进入 Error 状态
        match ui.state() {
            SaveUiState::Error { message } => {
                assert!(message.contains("为空"), "错误消息应包含'为空'");
            }
            other => panic!("期望 Error 状态，实际为 {:?}", other),
        }
    }

    // ─── AC06 ───────────────────────────────────────────────────────────────

    /// AC06 — 覆盖确认触发：Save 模式下在有数据的槽位按确认 → ConfirmOverwrite。
    #[test]
    fn ac06_overwrite_confirm_triggered() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[0]); // 槽位 0 有数据
        ui.open(SaveUiMode::Save, &infos);

        // 选中槽位 0（有数据），按确认
        let result = ui.handle_input(UiAction::Confirm);
        assert_eq!(result, SaveUiResult::None);

        // 应进入 ConfirmOverwrite 状态
        match ui.state() {
            SaveUiState::ConfirmOverwrite { slot } => {
                assert_eq!(*slot, 0);
            }
            other => panic!("期望 ConfirmOverwrite 状态，实际为 {:?}", other),
        }
    }

    // ─── AC07 ───────────────────────────────────────────────────────────────

    /// AC07 — 覆盖确认后执行保存：Confirm → SaveRequested。
    #[test]
    fn ac07_confirm_save_executed() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[0]);
        ui.open(SaveUiMode::Save, &infos);

        // 触发覆盖确认
        ui.handle_input(UiAction::Confirm);
        // 在 ConfirmOverwrite 状态按确认
        let result = ui.handle_input(UiAction::Confirm);

        assert_eq!(result, SaveUiResult::SaveRequested { slot: 0 });
        assert_eq!(*ui.state(), SaveUiState::Hidden);
    }

    // ─── AC08 ───────────────────────────────────────────────────────────────

    /// AC08 — 删除确认：Delete → ConfirmDelete → 确认 → DeleteRequested。
    #[test]
    fn ac08_delete_confirmed() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[2]);
        ui.open(SaveUiMode::Save, &infos);

        // 选中槽位 0 → 移到槽位 2（索引 2）
        ui.handle_input(UiAction::Down); // idx 1
        ui.handle_input(UiAction::Down); // idx 2
        assert_eq!(ui.selected_slot(), Some(2));

        // 按 Delete → ConfirmDelete
        let result = ui.handle_input(UiAction::Delete);
        assert_eq!(result, SaveUiResult::None);
        match ui.state() {
            SaveUiState::ConfirmDelete { slot } => assert_eq!(*slot, 2),
            other => panic!("期望 ConfirmDelete，实际为 {:?}", other),
        }

        // 确认删除
        let result = ui.handle_input(UiAction::Confirm);
        assert_eq!(result, SaveUiResult::DeleteRequested { slot: 2 });
    }

    // ─── AC09 ───────────────────────────────────────────────────────────────

    /// AC09 — ESC 返回：SlotList 按 Cancel → Hidden → Closed。
    #[test]
    fn ac09_esc_returns() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[]);
        ui.open(SaveUiMode::Save, &infos);

        let result = ui.handle_input(UiAction::Cancel);
        assert_eq!(result, SaveUiResult::Closed);
        assert_eq!(*ui.state(), SaveUiState::Hidden);
        assert!(!ui.is_open());
    }

    // ─── AC11 ───────────────────────────────────────────────────────────────

    /// AC11 — 快速/自动槽位 label 正确。
    #[test]
    fn ac11_slot_labels() {
        use crate::save_manager::slot_label;

        assert_eq!(slot_label(0), "槽位 1");
        assert_eq!(slot_label(4), "槽位 5");
        assert_eq!(slot_label(98), "快速存档");
        assert_eq!(slot_label(99), "自动存档");
    }

    // ─── 补充测试 ──────────────────────────────────────────────────────────

    /// 验证 SaveUiMode::Load 下打开 UI。
    #[test]
    fn test_open_load_mode() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[1, 3]);
        ui.open(SaveUiMode::Load, &infos);

        match ui.state() {
            SaveUiState::SlotList { mode, .. } => {
                assert_eq!(*mode, SaveUiMode::Load);
            }
            other => panic!("期望 SlotList，实际为 {:?}", other),
        }
    }

    /// 验证 Load 模式下有数据的槽位直接触发 LoadRequested。
    #[test]
    fn test_load_existing_slot() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[0]);
        ui.open(SaveUiMode::Load, &infos);

        let result = ui.handle_input(UiAction::Confirm);
        assert_eq!(result, SaveUiResult::LoadRequested { slot: 0 });
        assert!(!ui.is_open());
    }

    /// 验证 Save 模式下空槽位直接触发 SaveRequested。
    #[test]
    fn test_save_empty_slot() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[]); // 全部为空
        ui.open(SaveUiMode::Save, &infos);

        let result = ui.handle_input(UiAction::Confirm);
        assert_eq!(result, SaveUiResult::SaveRequested { slot: 0 });
    }

    /// 验证空槽位按 Delete 返回 Error。
    #[test]
    fn test_delete_empty_slot_error() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[]);
        ui.open(SaveUiMode::Save, &infos);

        let result = ui.handle_input(UiAction::Delete);
        assert_eq!(result, SaveUiResult::None);

        match ui.state() {
            SaveUiState::Error { message } => {
                assert!(message.contains("没有可删除"));
            }
            other => panic!("期望 Error，实际为 {:?}", other),
        }
    }

    /// 验证 ConfirmOverwrite 中按 Cancel 会关闭 UI。
    #[test]
    fn test_overwrite_cancel() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[0]);
        ui.open(SaveUiMode::Save, &infos);
        ui.handle_input(UiAction::Confirm); // → ConfirmOverwrite

        let result = ui.handle_input(UiAction::Cancel);
        assert_eq!(result, SaveUiResult::Closed);
    }

    /// 验证 ConfirmDelete 中按 Cancel 关闭 UI。
    #[test]
    fn test_delete_cancel() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[0]);
        ui.open(SaveUiMode::Save, &infos);
        ui.handle_input(UiAction::Delete); // → ConfirmDelete

        let result = ui.handle_input(UiAction::Cancel);
        assert_eq!(result, SaveUiResult::Closed);
    }

    /// 验证 render_commands 在 SlotList 状态下返回非空命令列表。
    #[test]
    fn test_render_commands_slot_list() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[0, 2]);
        ui.open(SaveUiMode::Save, &infos);

        let commands = ui.render_commands();
        assert!(!commands.is_empty(), "SlotList 应产生渲染指令");

        // 应包含 Overlay
        let has_overlay = commands
            .iter()
            .any(|c| matches!(c, UiCommand::Overlay { .. }));
        assert!(has_overlay, "应包含背景遮罩");

        // 应包含标题
        let has_title = commands.iter().any(|c| {
            if let UiCommand::Text { content, .. } = c {
                content == "存档"
            } else {
                false
            }
        });
        assert!(has_title, "应包含标题'存档'");

        // 应包含底部提示
        let has_hint = commands.iter().any(|c| {
            if let UiCommand::Text { content, .. } = c {
                content.contains("↑↓ 选择")
            } else {
                false
            }
        });
        assert!(has_hint, "应包含操作提示");
    }

    /// 验证 Hidden 状态 render_commands 返回空列表。
    #[test]
    fn test_render_commands_hidden() {
        let ui = SaveUi::new();
        let commands = ui.render_commands();
        assert!(commands.is_empty());
    }

    /// 验证 ConfirmOverwrite 状态 render_commands 包含对话框。
    #[test]
    fn test_render_commands_confirm_overwrite() {
        let mut ui = SaveUi::new();
        let infos = mock_slot_infos(&[0]);
        ui.open(SaveUiMode::Save, &infos);
        ui.handle_input(UiAction::Confirm);

        let commands = ui.render_commands();
        let has_dialog = commands
            .iter()
            .any(|c| matches!(c, UiCommand::ConfirmDialog { .. }));
        assert!(has_dialog, "ConfirmOverwrite 应包含确认对话框");
    }

    /// 验证 close() 方法将状态恢复到 Hidden。
    #[test]
    fn test_close() {
        let mut ui = SaveUi::new();
        ui.open(SaveUiMode::Save, &mock_slot_infos(&[]));
        assert!(ui.is_open());

        ui.close();
        assert!(!ui.is_open());
        assert_eq!(*ui.state(), SaveUiState::Hidden);
    }
}
