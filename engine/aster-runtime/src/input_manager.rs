//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/input_manager.rs
//! 功能概述：输入管理器 — 将 winit 原始窗口事件（鼠标点击、键盘按键、窗口关闭等）
//!           映射为语义化的游戏动作（GameAction）。提供去抖（debounce）逻辑，
//!           按输入源（按键/鼠标按钮）独立计时，防止长按/快速连点导致意外重复触发。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - winit（窗口事件类型：WindowEvent / Key / MouseButton / ElementState）

use std::collections::HashMap;
use std::time::{Duration, Instant};

use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::Key;
use winit::keyboard::NamedKey;

/// 游戏动作 — winit 原始输入事件映射后的语义化动作。
///
/// 所有用户输入（鼠标/键盘/窗口事件）经过 InputManager 处理后，
/// 统一映射为此枚举的某个变体。调用方（主事件循环）根据 GameAction
/// 执行对应的游戏逻辑，无需关心底层 winit 事件细节。
///
/// Phase 1 只实现了 Advance / OpenMenu / Quit / None 的映射，
/// 其余变体（Skip / Auto / QuickSave / QuickLoad / ToggleFullscreen）
/// 已预留，将在后续 Phase 实现对应功能时启用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameAction {
    /// 推进对话/确认选择（鼠标左键 / Enter / Space）
    Advance,
    /// 打开菜单（Esc / 右键）— Phase 1 预留，暂无菜单功能
    OpenMenu,
    /// 快进模式（Ctrl 键按下）— Phase 4 实现完整 Skip 逻辑
    Skip,
    /// 自动模式切换（A 键）— Phase 4 实现
    Auto,
    /// 快速存档（F5）— Phase 2 实现
    QuickSave,
    /// 快速读档（F9）— Phase 2 实现
    QuickLoad,
    /// 切换全屏（Alt+Enter）— Phase 1 预留
    ToggleFullscreen,
    /// 退出游戏（窗口关闭按钮 / Alt+F4）
    Quit,
    /// 无操作（未识别的输入或去抖拦截）
    None,
}

/// 输入管理器 — winit 事件 → GameAction 映射 + 去抖。
///
/// ## 职责
/// 1. 将 winit `WindowEvent` 映射为语义化 `GameAction`
/// 2. 按输入源（按键/鼠标按钮）独立去抖，默认间隔 200ms
/// 3. 窗口关闭事件不去抖（立即处理）
///
/// ## 去抖设计
/// - 同一按键在 200ms 内的连续触发只产生一次 `GameAction`（防长按重复触发）
/// - 不同按键独立计时：Enter → Space 两次触发互不干扰
/// - 鼠标按钮与键盘按键独立计时
/// - 后续 Phase 可通过构造函数参数自定义去抖间隔
///
/// ## 使用示例
/// ```rust,ignore
/// let mut im = InputManager::new();
/// match im.process_event(&event) {
///     GameAction::Advance => scene_manager.on_click(Some(&mut renderer))?,
///     GameAction::Quit => el.exit(),
///     _ => {}
/// }
/// ```
pub struct InputManager {
    /// 按键级别最后触发时间（用于去抖，不同按键独立计时）
    key_last_trigger: HashMap<Key, Instant>,
    /// 鼠标按钮级别最后触发时间（用于去抖，不同按钮独立计时）
    mouse_last_trigger: HashMap<MouseButton, Instant>,
    /// 去抖间隔（默认 200ms）
    debounce_interval: Duration,
}

impl InputManager {
    /// 创建输入管理器，使用默认去抖间隔（200ms）。
    pub fn new() -> Self {
        Self {
            key_last_trigger: HashMap::new(),
            mouse_last_trigger: HashMap::new(),
            debounce_interval: Duration::from_millis(200),
        }
    }

    /// 处理 winit 窗口事件，返回对应的游戏动作。
    ///
    /// ## 事件映射（Phase 1 默认绑定）
    ///
    /// | winit 事件 | 条件 | GameAction |
    /// |-----------|------|-----------|
    /// | `CloseRequested` | — | `Quit`（不去抖） |
    /// | `KeyboardInput` + `Pressed` | Enter / Space | `Advance` |
    /// | `KeyboardInput` + `Pressed` | Escape | `OpenMenu` |
    /// | `MouseInput` + `Pressed` + Left | — | `Advance` |
    /// | 其他 | — | `None` |
    ///
    /// ## 去抖规则
    /// - `CloseRequested` 不去抖，立即返回 `Quit`
    /// - 键盘事件：同一 `Key` 在 `debounce_interval` 内的重复触发被拦截
    /// - 鼠标事件：同一 `MouseButton` 在 `debounce_interval` 内的重复触发被拦截
    ///
    /// ## 参数
    /// - `event`: winit 产生的窗口事件引用
    ///
    /// ## 返回值
    /// 映射后的 `GameAction`，被去抖拦截时返回 `GameAction::None`
    pub fn process_event(&mut self, event: &WindowEvent) -> GameAction {
        match event {
            // 窗口关闭事件 — 不去抖，立即处理
            WindowEvent::CloseRequested => GameAction::Quit,

            // 键盘事件
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                // 只处理按键按下事件，忽略释放
                if key_event.state != ElementState::Pressed {
                    return GameAction::None;
                }

                // 忽略操作系统的按键重复事件（长按产生的合成事件）
                // 这些事件由 OS 在按键持续按下时以固定频率生成，必须过滤
                if key_event.repeat {
                    return GameAction::None;
                }

                let action = Self::map_key_to_action(&key_event.logical_key);
                if matches!(action, GameAction::None) {
                    return GameAction::None;
                }

                // 去抖检查：同一按键在间隔内重复触发则拦截
                if self.is_key_debounced(&key_event.logical_key) {
                    return GameAction::None;
                }

                // 更新该按键的最后触发时间
                self.key_last_trigger
                    .insert(key_event.logical_key.clone(), Instant::now());

                action
            }

            // 鼠标事件
            WindowEvent::MouseInput { state, button, .. } => {
                // 只处理按键按下事件
                if *state != ElementState::Pressed {
                    return GameAction::None;
                }

                let action = Self::map_mouse_to_action(button);
                if matches!(action, GameAction::None) {
                    return GameAction::None;
                }

                // 去抖检查：同一鼠标按钮在间隔内重复触发则拦截
                if self.is_mouse_debounced(button) {
                    return GameAction::None;
                }

                // 更新该鼠标按钮的最后触发时间
                self.mouse_last_trigger.insert(*button, Instant::now());

                action
            }

            // 未映射的事件类型
            _ => GameAction::None,
        }
    }

    /// 键盘按键 → 游戏动作映射（Phase 1 默认绑定）。
    ///
    /// Phase 1 默认绑定：
    /// - Enter / Space → `Advance`
    /// - Escape → `OpenMenu`
    fn map_key_to_action(key: &Key) -> GameAction {
        match key {
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => GameAction::Advance,
            Key::Named(NamedKey::Escape) => GameAction::OpenMenu,
            _ => GameAction::None,
        }
    }

    /// 鼠标按钮 → 游戏动作映射（Phase 1 默认绑定）。
    ///
    /// Phase 1 默认绑定：
    /// - Left → `Advance`
    fn map_mouse_to_action(button: &MouseButton) -> GameAction {
        match button {
            MouseButton::Left => GameAction::Advance,
            _ => GameAction::None,
        }
    }

    /// 检查指定按键是否在去抖间隔内（已被去抖拦截）。
    ///
    /// 如果该按键从未触发过，或距上次触发已超过 `debounce_interval`，
    /// 返回 `false`（不去抖，允许触发）。
    fn is_key_debounced(&self, key: &Key) -> bool {
        self.key_last_trigger
            .get(key)
            .is_some_and(|last| last.elapsed() < self.debounce_interval)
    }

    /// 检查指定鼠标按钮是否在去抖间隔内（已被去抖拦截）。
    ///
    /// 如果该按钮从未触发过，或距上次触发已超过 `debounce_interval`，
    /// 返回 `false`（不去抖，允许触发）。
    fn is_mouse_debounced(&self, button: &MouseButton) -> bool {
        self.mouse_last_trigger
            .get(button)
            .is_some_and(|last| last.elapsed() < self.debounce_interval)
    }
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use winit::event::KeyEvent;

    /// 构造一个用于测试的 DeviceId。
    ///
    /// winit 0.30 的 `DeviceId` 无法在安全代码中构造（无公开构造函数）。
    /// 使用 `unsafe { std::mem::zeroed() }` 是 winit 生态中单元测试的通行做法：
    /// `DeviceId` 在事件 match 分支中从不被读取（仅用于透传），其值不影响逻辑。
    fn dummy_device_id() -> winit::event::DeviceId {
        // SAFETY: DeviceId 在测试中仅作为占位值，不会被读取。全零位模式对其安全。
        unsafe { std::mem::zeroed() }
    }

    /// 构造一个用于测试的 `KeyEvent`。
    ///
    /// winit 0.30 的 `KeyEvent` 包含 `pub(crate) platform_specific` 私有字段，
    /// 无法通过 struct literal 在外部 crate 构造。使用 unsafe zeroed 初始化后
    /// 设置公开字段是唯一可行的单元测试方式。
    fn make_key_event(key: Key, state: ElementState) -> KeyEvent {
        // SAFETY: KeyEvent 的所有公开字段在 zeroed 后被重新赋值，
        // `platform_specific` 保持零值 — 在测试中从不被读取，安全。
        let mut event: KeyEvent = unsafe { std::mem::zeroed() };
        event.physical_key = winit::keyboard::PhysicalKey::Unidentified(
            winit::keyboard::NativeKeyCode::Unidentified,
        );
        event.logical_key = key;
        event.state = state;
        event.location = winit::keyboard::KeyLocation::Standard;
        event.repeat = false;
        event
    }

    /// 构造一个按下的键盘事件用于测试。
    fn make_key_pressed(key: Key) -> WindowEvent {
        WindowEvent::KeyboardInput {
            device_id: dummy_device_id(),
            event: make_key_event(key, ElementState::Pressed),
            is_synthetic: false,
        }
    }

    /// 构造一个释放的键盘事件用于测试。
    fn make_key_released(key: Key) -> WindowEvent {
        WindowEvent::KeyboardInput {
            device_id: dummy_device_id(),
            event: make_key_event(key, ElementState::Released),
            is_synthetic: false,
        }
    }

    /// 构造一个按下的鼠标事件用于测试。
    fn make_mouse_pressed(button: MouseButton) -> WindowEvent {
        WindowEvent::MouseInput {
            device_id: dummy_device_id(),
            state: ElementState::Pressed,
            button,
        }
    }

    // ── AC01 ──────────────────────────────────────────────

    #[test]
    /// AC01: 左键按下映射为 Advance
    fn test_ac01_mouse_left_press_maps_to_advance() {
        let mut im = InputManager::new();
        let result = im.process_event(&make_mouse_pressed(MouseButton::Left));
        assert_eq!(result, GameAction::Advance);
    }

    // ── AC02 ──────────────────────────────────────────────

    #[test]
    /// AC02: Enter 键按下映射为 Advance
    fn test_ac02_enter_press_maps_to_advance() {
        let mut im = InputManager::new();
        let result = im.process_event(&make_key_pressed(Key::Named(NamedKey::Enter)));
        assert_eq!(result, GameAction::Advance);
    }

    #[test]
    /// AC02 补充: Space 键按下映射为 Advance
    fn test_ac02_space_press_maps_to_advance() {
        let mut im = InputManager::new();
        let result = im.process_event(&make_key_pressed(Key::Named(NamedKey::Space)));
        assert_eq!(result, GameAction::Advance);
    }

    // ── AC03 ──────────────────────────────────────────────

    #[test]
    /// AC03: 200ms 内连续两次相同按键只产生一次 Advance
    fn test_ac03_same_key_debounced() {
        let mut im = InputManager::new();
        // 第一次 Enter → Advance
        let first = im.process_event(&make_key_pressed(Key::Named(NamedKey::Enter)));
        assert_eq!(first, GameAction::Advance);
        // 立即第二次 Enter（间隔远小于 200ms）→ None（被去抖拦截）
        let second = im.process_event(&make_key_pressed(Key::Named(NamedKey::Enter)));
        assert_eq!(second, GameAction::None);
    }

    // ── AC04 ──────────────────────────────────────────────

    #[test]
    /// AC04: 不同按键不互相去抖 — Enter → Space 两次都返回 Advance
    fn test_ac04_different_keys_not_debounced() {
        let mut im = InputManager::new();
        // Enter → Advance
        let first = im.process_event(&make_key_pressed(Key::Named(NamedKey::Enter)));
        assert_eq!(first, GameAction::Advance);
        // Space → Advance（不被 Enter 的去抖影响）
        let second = im.process_event(&make_key_pressed(Key::Named(NamedKey::Space)));
        assert_eq!(second, GameAction::Advance);
    }

    #[test]
    /// AC04 补充: 键盘和鼠标不互相去抖
    fn test_ac04_keyboard_and_mouse_not_debounced() {
        let mut im = InputManager::new();
        // Enter → Advance
        let first = im.process_event(&make_key_pressed(Key::Named(NamedKey::Enter)));
        assert_eq!(first, GameAction::Advance);
        // 鼠标左键 → Advance（不被键盘去抖影响）
        let second = im.process_event(&make_mouse_pressed(MouseButton::Left));
        assert_eq!(second, GameAction::Advance);
    }

    // ── AC05 ──────────────────────────────────────────────

    #[test]
    /// AC05: WindowEvent::CloseRequested 映射为 Quit
    fn test_ac05_close_requested_maps_to_quit() {
        let mut im = InputManager::new();
        let result = im.process_event(&WindowEvent::CloseRequested);
        assert_eq!(result, GameAction::Quit);
    }

    // ── 补充测试 ──────────────────────────────────────────

    #[test]
    /// Escape 键按下映射为 OpenMenu
    fn test_escape_maps_to_open_menu() {
        let mut im = InputManager::new();
        let result = im.process_event(&make_key_pressed(Key::Named(NamedKey::Escape)));
        assert_eq!(result, GameAction::OpenMenu);
    }

    #[test]
    /// 按键释放不产生 GameAction
    fn test_key_release_returns_none() {
        let mut im = InputManager::new();
        let event = make_key_released(Key::Named(NamedKey::Enter));
        assert_eq!(im.process_event(&event), GameAction::None);
    }

    #[test]
    /// 鼠标释放不产生 GameAction
    fn test_mouse_release_returns_none() {
        let mut im = InputManager::new();
        let result = im.process_event(&WindowEvent::MouseInput {
            device_id: dummy_device_id(),
            state: ElementState::Released,
            button: MouseButton::Left,
        });
        assert_eq!(result, GameAction::None);
    }

    #[test]
    /// 按键重复事件（长按）被忽略 — OS 合成 repeat 事件不应触发 GameAction
    fn test_key_repeat_returns_none() {
        let mut im = InputManager::new();
        let mut event = make_key_event(Key::Named(NamedKey::Enter), ElementState::Pressed);
        event.repeat = true;
        let window_event = WindowEvent::KeyboardInput {
            device_id: dummy_device_id(),
            event,
            is_synthetic: false,
        };
        // 即使第一次触发（无去抖记录），repeat 事件也应被忽略
        assert_eq!(im.process_event(&window_event), GameAction::None);
    }

    #[test]
    /// 未映射的按键返回 None
    fn test_unmapped_key_returns_none() {
        let mut im = InputManager::new();
        // 'A' 键在 Phase 1 未被映射
        let result = im.process_event(&make_key_pressed(Key::Character("a".into())));
        assert_eq!(result, GameAction::None);
    }

    #[test]
    /// CloseRequested 不去抖 — 连续两次都应返回 Quit
    fn test_close_requested_not_debounced() {
        let mut im = InputManager::new();
        assert_eq!(
            im.process_event(&WindowEvent::CloseRequested),
            GameAction::Quit
        );
        // 窗口关闭事件不去抖，第二次仍然返回 Quit
        assert_eq!(
            im.process_event(&WindowEvent::CloseRequested),
            GameAction::Quit
        );
    }

    #[test]
    /// InputManager 实现 Default trait
    fn test_input_manager_default() {
        let im = InputManager::default();
        assert_eq!(im.debounce_interval, Duration::from_millis(200));
    }
}
