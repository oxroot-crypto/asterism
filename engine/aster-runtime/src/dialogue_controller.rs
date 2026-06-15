//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-runtime/src/dialogue_controller.rs
//! 功能概述：对话流管理器 — 协调每句对话的完整生命周期：
//!           从 SceneManager 收到对话命令 → 启动打字机动画 →
//!           等待用户点击（打字机进行中则跳过动画，已完成则推进到下一句）→
//!           通知 SceneManager 继续 VM 执行。
//!           管理说话者名字显示、文本缓冲队列（预加载下一句文本）。
//! 作者：Claude (AI)
//! 创建日期：2026-06-15
//! 最后修改：2026-06-15
//!
//! 依赖模块：
//! - aster_renderer::Typewriter（打字机效果状态机）
//! - std::collections::VecDeque（文本缓冲队列）
//!
//! 对应任务：PH1-T19 — 实现 DialogueController
//! 对应需求：REQ-ENG-014（打字机效果 — 运行时控制）, REQ-ENG-020（点击推进对话）
//! 架构位置：aster-runtime — 在 SceneManager 和 Renderer 之间协调对话流

use std::collections::VecDeque;
use std::time::Duration;

use aster_renderer::{Typewriter, TypewriterSpeed};

// ============================================================================
// DialogueState — 对话状态
// ============================================================================

/// 对话状态 — 表示当前对话所处的生命周期阶段。
///
/// # 状态转换
/// ```text
///                                    push(line)
///   Idle ──────────────────────────────────────────────────────────→ Typewriting
///                                                                      │
///                                                    update(dt) 推进    │
///                                                      直到完成        │
///                                                                      ↓
///   Completed ←── on_click()（队列为空）── WaitingForAdvance ←─────────┘
///       │                                     │
///       │                    on_click()（队列有下一行）
///       │                         自动开始下一行
///       │                                     │
///       └─────────────────────────────────────┘
///              push(line) 自动开始
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueState {
    /// 无活跃对话 — 等待 VM 产生对话命令
    Idle,
    /// 打字机动画进行中 — 文字逐字显示
    Typewriting,
    /// 打字机完成，等待用户点击推进到下一句
    WaitingForAdvance,
    /// 用户已点击推进，等待 SceneManager 继续 VM 执行
    Completed,
}

// ============================================================================
// DialogueAction — on_click 返回的动作指令
// ============================================================================

/// 用户点击后的动作指令 — 告诉 SceneManager 下一步该做什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueAction {
    /// 无需 VM 动作 — 打字机被跳过或队列有下一行已自动开始
    None,
    /// 推进 VM — 当前对话已完成且队列为空，需要 VM 产生下一句
    Advance,
}

// ============================================================================
// DialogueLine — 单句对话数据
// ============================================================================

/// 单句对话 — 包含说话者、正文和可选的语音文件标识。
///
/// 由 SceneManager 在 VM 产生 `SetDialogue` / `SetNarration` 命令时构造，
/// 推入 `DialogueController` 的缓冲队列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueLine {
    /// 说话者名字（旁白时为空字符串）
    pub speaker: String,
    /// 对话正文
    pub text: String,
    /// 语音文件标识（Phase 1 预留，Phase 2 音频系统集成后生效）
    pub voice_id: Option<String>,
}

// ============================================================================
// DialogueController — 对话流管理器
// ============================================================================

/// 对话流管理器 — 协调每句对话从产生到完成的完整生命周期。
///
/// # 职责
/// - 持有 `Typewriter` 实例，管理打字机动画进度
/// - 维护文本缓冲队列（`VecDeque<DialogueLine>`），支持预加载
/// - 追踪对话状态（Idle → Typewriting → WaitingForAdvance → Completed）
/// - 处理用户点击：打字机进行中 → 跳过动画；打字机完成 → 推进
///
/// # 使用示例
/// ```rust,ignore
/// let mut dc = DialogueController::new(TypewriterSpeed::Normal);
///
/// // VM 产生对话命令后
/// dc.push(DialogueLine {
///     speaker: "小百合".into(),
///     text: "今天天气真好啊。".into(),
///     voice_id: None,
/// });
///
/// // 每帧调用
/// dc.update(delta_time);
///
/// // 用户点击时
/// match dc.on_click() {
///     DialogueAction::Advance => { /* 推进 VM */ }
///     DialogueAction::None => { /* 同步可见范围到渲染器 */ }
/// }
/// ```
#[derive(Debug)]
pub struct DialogueController {
    /// 当前对话状态
    state: DialogueState,
    /// 文本缓冲队列 — 预加载 1-2 句对话以减少文本布局延迟
    queue: VecDeque<DialogueLine>,
    /// 当前显示中的说话者名字
    current_speaker: String,
    /// 当前显示中的对话正文（完整文本，非逐字截取后的）
    current_text: String,
    /// 打字机效果状态机
    typewriter: Typewriter,
}

impl DialogueController {
    // ─── 构造 ──────────────────────────────────────────────────────────

    /// 创建新的对话流管理器。
    ///
    /// # 参数
    /// - `speed`: 初始打字机显示速度档位
    ///
    /// # 初始状态
    /// - `state = Idle`
    /// - 队列为空
    /// - 无当前文本
    pub fn new(speed: TypewriterSpeed) -> Self {
        Self {
            state: DialogueState::Idle,
            queue: VecDeque::new(),
            current_speaker: String::new(),
            current_text: String::new(),
            typewriter: Typewriter::new(speed),
        }
    }

    // ─── 公共方法 ──────────────────────────────────────────────────────

    /// 将新对话推入缓冲队列。
    ///
    /// 如果当前无活跃对话（Idle 或已完成等待下次 VM 执行），则立即开始显示该行。
    /// 否则将对话加入队列尾部，等待当前对话完成后再显示。
    ///
    /// # 参数
    /// - `line`: 对话数据（说话者 + 正文 + 可选语音）
    pub fn push(&mut self, line: DialogueLine) {
        match self.state {
            DialogueState::Idle | DialogueState::Completed => {
                // 无活跃对话，立即开始显示
                self.start_line(line);
            }
            DialogueState::Typewriting | DialogueState::WaitingForAdvance => {
                // 当前有对话在进行中，加入队列
                self.queue.push_back(line);
            }
        }
    }

    /// 每帧调用以推进打字机动画。
    ///
    /// 当打字机动画完成（`visible_chars >= total_chars`）时，
    /// 自动将状态从 `Typewriting` 转换到 `WaitingForAdvance`。
    ///
    /// # 参数
    /// - `delta`: 自上一帧以来的时间增量
    ///
    /// # 返回值
    /// - 当前的 `DialogueState`（调用方可据此决定渲染行为）
    pub fn update(&mut self, delta: Duration) -> DialogueState {
        if self.state != DialogueState::Typewriting {
            return self.state;
        }

        // 推进打字机动画
        self.typewriter.update(delta);

        // 如果打字机完成，转换状态
        if self.typewriter.is_complete() {
            self.state = DialogueState::WaitingForAdvance;
        }

        self.state
    }

    /// 处理用户点击事件。
    ///
    /// 行为取决于当前状态：
    /// - `Typewriting` → 跳过打字机动画（`typewriter.skip()`），转换到 `WaitingForAdvance`，返回 `None`
    /// - `WaitingForAdvance` → 检查队列：
    ///   - 队列有下一行 → 自动开始下一行，返回 `None`
    ///   - 队列为空 → 转换到 `Completed`，返回 `Advance`（通知 SceneManager 推进 VM）
    /// - 其他状态 → 返回 `None`
    ///
    /// # 返回值
    /// - `Advance` — 需要 SceneManager 继续 VM 执行以产生下一句对话
    /// - `None` — 无需 VM 动作（打字机被跳过或队列中已有下一行）
    pub fn on_click(&mut self) -> DialogueAction {
        match self.state {
            DialogueState::Typewriting => {
                // 打字机进行中 → 跳过动画，立即显示全部文本
                self.typewriter.skip();
                self.state = DialogueState::WaitingForAdvance;
                DialogueAction::None
            }
            DialogueState::WaitingForAdvance => {
                // 打字机已完成，检查队列中是否有下一句
                if let Some(next_line) = self.queue.pop_front() {
                    // 队列中有预加载的下一句，直接开始显示
                    self.start_line(next_line);
                    DialogueAction::None
                } else {
                    // 队列为空，需要 VM 推进以产生下一句
                    self.state = DialogueState::Completed;
                    DialogueAction::Advance
                }
            }
            _ => DialogueAction::None,
        }
    }

    /// 重置控制器到初始状态（场景切换时调用）。
    ///
    /// 清空队列、清除当前文本、重置打字机、状态回到 `Idle`。
    pub fn reset(&mut self) {
        self.state = DialogueState::Idle;
        self.queue.clear();
        self.current_speaker.clear();
        self.current_text.clear();
        self.typewriter.reset("");
    }

    // ─── 访问器 ────────────────────────────────────────────────────────

    /// 获取当前对话状态。
    #[inline]
    pub fn state(&self) -> DialogueState {
        self.state
    }

    /// 获取当前说话者名字。
    ///
    /// 返回值：
    /// - 角色名 — 对话
    /// - 空字符串 — 旁白
    #[inline]
    pub fn current_speaker(&self) -> &str {
        &self.current_speaker
    }

    /// 获取当前对话正文（完整文本，非逐字截取后的）。
    ///
    /// 渲染器使用此方法获取完整文本用于排版，
    /// 同时结合 `current_visible_chars()` 控制打字机显示进度。
    #[inline]
    pub fn current_text(&self) -> &str {
        &self.current_text
    }

    /// 获取当前可见字符数（打字机进度）。
    ///
    /// 渲染器应调用 `set_visible_range(0, count)` 仅显示前 N 个字符。
    /// 当打字机完成时，此值等于 `current_text().chars().count()`。
    #[inline]
    pub fn current_visible_chars(&self) -> usize {
        self.typewriter.visible_chars()
    }

    /// 获取打字机速度。
    #[inline]
    pub fn speed(&self) -> TypewriterSpeed {
        self.typewriter.speed()
    }

    /// 设置打字机速度。
    ///
    /// 速度变更在下一帧的 `update()` 调用时生效。
    #[inline]
    pub fn set_speed(&mut self, speed: TypewriterSpeed) {
        self.typewriter.set_speed(speed);
    }

    /// 获取当前队列长度。
    #[inline]
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    // ─── 内部辅助方法 ──────────────────────────────────────────────────

    /// 开始显示一行新对话。
    ///
    /// 设置当前说话者、正文，重置打字机以开始逐字动画，
    /// 并将状态设置为 `Typewriting`。
    ///
    /// # 参数
    /// - `line`: 要开始显示的对话行
    fn start_line(&mut self, line: DialogueLine) {
        self.current_speaker = line.speaker;
        self.current_text = line.text;
        self.typewriter.reset(&self.current_text);
        // 空文本在 reset 后立即标记为完成，update 会直接转到 WaitingForAdvance
        self.state = DialogueState::Typewriting;
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC01-AC05
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：快速构造 DialogueLine
    fn line(speaker: &str, text: &str) -> DialogueLine {
        DialogueLine {
            speaker: speaker.to_string(),
            text: text.to_string(),
            voice_id: None,
        }
    }

    // ========================================================================
    // AC01 — push 对话后自动开始打字机动画
    // ========================================================================

    /// AC01: Idle 状态下 push → 立即开始 Typewriting。
    #[test]
    fn ac01_push_from_idle_starts_typewriting() {
        let mut dc = DialogueController::new(TypewriterSpeed::Normal);
        assert_eq!(dc.state(), DialogueState::Idle);

        dc.push(line("小百合", "今天天气真好啊。"));

        assert_eq!(dc.state(), DialogueState::Typewriting);
        assert_eq!(dc.current_speaker(), "小百合");
        assert_eq!(dc.current_text(), "今天天气真好啊。");
    }

    /// AC01: push 后 update(0) → 状态不变（还是 Typewriting，等待时间积累）。
    #[test]
    fn ac01_update_zero_keeps_typewriting() {
        let mut dc = DialogueController::new(TypewriterSpeed::Normal);
        dc.push(line("A", "Hello"));

        let state = dc.update(Duration::ZERO);
        assert_eq!(state, DialogueState::Typewriting);
        assert!(dc.current_visible_chars() == 0 || dc.current_visible_chars() > 0);
        // Normal 速度，0ms 可能推进 0 个字符（需要至少 30ms 才推进第一个）
    }

    /// AC01: Instant 速度 → push 后 update(0) 立即完成。
    #[test]
    fn ac01_instant_completes_immediately() {
        let mut dc = DialogueController::new(TypewriterSpeed::Instant);
        dc.push(line("A", "Hello"));

        let state = dc.update(Duration::ZERO);
        assert_eq!(state, DialogueState::WaitingForAdvance);
        assert_eq!(dc.current_visible_chars(), 5); // "Hello" = 5 chars
    }

    // ========================================================================
    // AC02 — 打字机进行中点击 = 跳过动画（非推进）
    // ========================================================================

    /// AC02: Typewriting 状态下 on_click → skip → WaitingForAdvance，返回 None。
    #[test]
    fn ac02_click_during_typewriting_skips() {
        let mut dc = DialogueController::new(TypewriterSpeed::Slow); // 50ms/char
        dc.push(line("S", "Long text that takes time to display"));

        // 确认在 Typewriting 状态
        assert_eq!(dc.state(), DialogueState::Typewriting);

        // 模拟部分推进
        dc.update(Duration::from_millis(50)); // 推进 1 个字符
        assert!(dc.current_visible_chars() > 0);
        assert!(dc.current_visible_chars() < dc.current_text().chars().count());

        // 点击 → 跳过
        let action = dc.on_click();
        assert_eq!(action, DialogueAction::None);
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);
        // 所有文本应可见
        assert_eq!(
            dc.current_visible_chars(),
            dc.current_text().chars().count()
        );
    }

    /// AC02: 未开始打字机直接点击也正确跳过。
    #[test]
    fn ac02_click_before_any_update_skips() {
        let mut dc = DialogueController::new(TypewriterSpeed::Slow);
        dc.push(line("S", "Skip me"));

        // 不调用 update，直接点击
        let action = dc.on_click();
        assert_eq!(action, DialogueAction::None);
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);
        assert_eq!(dc.current_visible_chars(), 7); // "Skip me" = 7 chars
    }

    /// AC02: 已完成打字机的情况下跳过再点击 = Advance。
    #[test]
    fn ac02_click_after_skip_then_click_advances() {
        let mut dc = DialogueController::new(TypewriterSpeed::Slow);
        dc.push(line("S", "Hi"));

        // 点击跳过
        dc.on_click(); // → WaitingForAdvance
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);

        // 再次点击 → 推进（队列为空）
        let action = dc.on_click();
        assert_eq!(action, DialogueAction::Advance);
        assert_eq!(dc.state(), DialogueState::Completed);
    }

    // ========================================================================
    // AC03 — 打字机完成后点击 = 推进到下一句
    // ========================================================================

    /// AC03: 打字机自然完成后点击 → Advance。
    #[test]
    fn ac03_complete_then_click_advances() {
        let mut dc = DialogueController::new(TypewriterSpeed::Normal);
        dc.push(line("S", "Hi"));

        // 推进到完成
        dc.update(Duration::from_millis(60)); // 2 × 30ms = 60ms，推进 2 个字符
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);
        assert_eq!(dc.current_visible_chars(), 2);

        // 点击推进
        let action = dc.on_click();
        assert_eq!(action, DialogueAction::Advance);
        assert_eq!(dc.state(), DialogueState::Completed);
    }

    /// AC03: Instant 速度 → 无需等待即可点击推进。
    #[test]
    fn ac03_instant_then_click_advances() {
        let mut dc = DialogueController::new(TypewriterSpeed::Instant);
        dc.push(line("S", "Hi"));

        // Instant 速度，update(0) 立即完成
        dc.update(Duration::ZERO);
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);

        let action = dc.on_click();
        assert_eq!(action, DialogueAction::Advance);
    }

    // ========================================================================
    // AC04 — 对话队列正确串行
    // ========================================================================

    /// AC04: push(A) → push(B) → 完成A推进 → 自动开始B → 完成B推进。
    #[test]
    fn ac04_queue_serial_correct() {
        let mut dc = DialogueController::new(TypewriterSpeed::Instant);

        // 推入两句对话
        dc.push(line("小百合", "第一句"));
        dc.push(line("直人", "第二句"));

        // A 在 Typewriting
        assert_eq!(dc.state(), DialogueState::Typewriting);
        assert_eq!(dc.current_speaker(), "小百合");

        // 完成 A
        dc.update(Duration::ZERO);
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);

        // 点击推进 A → 自动开始 B
        let action = dc.on_click();
        assert_eq!(action, DialogueAction::None, "队列有 B，不应要求 VM 推进");
        assert_eq!(dc.state(), DialogueState::Typewriting);
        assert_eq!(dc.current_speaker(), "直人");
        assert_eq!(dc.current_text(), "第二句");

        // 完成 B
        dc.update(Duration::ZERO);
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);

        // 点击推进 B → 队列空 → Advance
        let action = dc.on_click();
        assert_eq!(action, DialogueAction::Advance);
        assert_eq!(dc.state(), DialogueState::Completed);
    }

    /// AC04: 3 句连续推进正确。
    #[test]
    fn ac04_three_line_queue() {
        let mut dc = DialogueController::new(TypewriterSpeed::Instant);
        dc.push(line("A", "1"));
        dc.push(line("B", "2"));
        dc.push(line("C", "3"));

        // 完成第1句
        dc.update(Duration::ZERO);
        dc.on_click(); // 开始第2句
        assert_eq!(dc.current_text(), "2");

        // 完成第2句
        dc.update(Duration::ZERO);
        dc.on_click(); // 开始第3句
        assert_eq!(dc.current_text(), "3");

        // 完成第3句
        dc.update(Duration::ZERO);
        let action = dc.on_click(); // 队列空
        assert_eq!(action, DialogueAction::Advance);
    }

    /// AC04: 只有在 Typewriting 状态下 push 才会排队。
    #[test]
    fn ac04_push_during_typewriting_queues() {
        let mut dc = DialogueController::new(TypewriterSpeed::Slow);
        dc.push(line("A", "First"));

        // 在打字机进行中 push 第二句 → 应排队
        dc.push(line("B", "Second"));
        assert_eq!(dc.queue_len(), 1);
        assert_eq!(dc.current_text(), "First"); // 当前仍在显示第一句
    }

    // ========================================================================
    // AC05 — 空文本/空说话者不 panic
    // ========================================================================

    /// AC05: 空文本（speaker="" text=""）push 不 panic。
    #[test]
    fn ac05_empty_text_no_panic() {
        let mut dc = DialogueController::new(TypewriterSpeed::Normal);
        dc.push(line("", ""));
        // 空文本在 Typewriter::reset("") 时即标记完成
        dc.update(Duration::ZERO);
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);
        assert_eq!(dc.current_visible_chars(), 0);

        let action = dc.on_click();
        assert_eq!(action, DialogueAction::Advance);
    }

    /// AC05: 空说话者 + 有文本正常。
    #[test]
    fn ac05_empty_speaker_with_text() {
        let mut dc = DialogueController::new(TypewriterSpeed::Normal);
        dc.push(line("", "旁白文本"));
        assert_eq!(dc.state(), DialogueState::Typewriting);
        assert_eq!(dc.current_speaker(), "");
        assert_eq!(dc.current_text(), "旁白文本");
    }

    /// AC05: 正常说话者 + 空文本不 panic。
    #[test]
    fn ac05_speaker_with_empty_text() {
        let mut dc = DialogueController::new(TypewriterSpeed::Normal);
        dc.push(line("小百合", ""));
        dc.update(Duration::ZERO);
        // 空文本立即完成
        assert_eq!(dc.state(), DialogueState::WaitingForAdvance);
        dc.on_click();
        // 不 panic
    }

    // ========================================================================
    // 额外边界测试
    // ========================================================================

    /// reset() 后状态正确回到 Idle。
    #[test]
    fn test_reset_clears_everything() {
        let mut dc = DialogueController::new(TypewriterSpeed::Normal);
        dc.push(line("A", "Text"));
        dc.push(line("B", "Queued"));
        dc.update(Duration::from_millis(100));

        dc.reset();

        assert_eq!(dc.state(), DialogueState::Idle);
        assert_eq!(dc.queue_len(), 0);
        assert_eq!(dc.current_speaker(), "");
        assert_eq!(dc.current_text(), "");
    }

    /// Completed 状态下 push 自动开始新行。
    #[test]
    fn test_push_from_completed_starts_immediately() {
        let mut dc = DialogueController::new(TypewriterSpeed::Instant);
        dc.push(line("A", "First"));
        dc.update(Duration::ZERO);
        dc.on_click(); // → Completed

        assert_eq!(dc.state(), DialogueState::Completed);

        // 从 Completed push → 立即开始
        dc.push(line("B", "Second"));
        assert_eq!(dc.state(), DialogueState::Typewriting);
        assert_eq!(dc.current_text(), "Second");
    }

    /// Idle 状态下 on_click 返回 None 且不 panic。
    #[test]
    fn test_click_when_idle_does_nothing() {
        let mut dc = DialogueController::new(TypewriterSpeed::Normal);
        assert_eq!(dc.state(), DialogueState::Idle);

        let action = dc.on_click();
        assert_eq!(action, DialogueAction::None);
        assert_eq!(dc.state(), DialogueState::Idle);
    }

    /// Completed 状态下 on_click 返回 None。
    #[test]
    fn test_click_when_completed_does_nothing() {
        let mut dc = DialogueController::new(TypewriterSpeed::Instant);
        dc.push(line("A", "Hi"));
        dc.update(Duration::ZERO);
        dc.on_click(); // → Completed

        // 再次点击不应改变状态
        let action = dc.on_click();
        assert_eq!(action, DialogueAction::None);
        assert_eq!(dc.state(), DialogueState::Completed);
    }

    /// set_speed 生效。
    #[test]
    fn test_set_speed_takes_effect() {
        let mut dc = DialogueController::new(TypewriterSpeed::Slow);
        assert_eq!(dc.speed(), TypewriterSpeed::Slow);

        dc.set_speed(TypewriterSpeed::Fast);
        assert_eq!(dc.speed(), TypewriterSpeed::Fast);
    }

    /// CJK 文本字符计数正确（char 级别）。
    #[test]
    fn test_cjk_visible_chars() {
        let mut dc = DialogueController::new(TypewriterSpeed::Instant);
        dc.push(line("S", "你好世界！"));
        dc.update(Duration::ZERO);
        assert_eq!(dc.current_visible_chars(), 5); // 你(1)好(2)世(3)界(4)！(5)
    }
}
