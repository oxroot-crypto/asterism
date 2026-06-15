//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-renderer/src/typewriter.rs
//! 功能概述：打字机效果状态机 — 管理对话正文的逐字显示动画。
//!           提供多种速度档位（Instant / Slow / Normal / Fast / Custom），
//!           支持跳过动画立即显示全部文本，预留 CharacterShown 回调用于 Phase 2 打字音效同步。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - std::time::Duration（时间推进）
//!
//! 架构位置：aster-core ← aster-renderer（Typewriter 为纯状态机，无 GPU 依赖）
//!
//! 对应任务：PH1-T10 — 打字机效果
//! 对应需求：REQ-ENG-014（文字逐字显示 / 打字机效果）

use std::time::Duration;

// ============================================================================
// TypewriterSpeed — 打字机显示速度
// ============================================================================

/// 打字机显示速度档位。
///
/// 控制每个字符显示的间隔时间（毫秒），影响逐字推进的速率。
/// 预留 `Custom(f32)` 变体允许创作者通过脚本自定义速度。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypewriterSpeed {
    /// 瞬间完成 — 0ms/char，文本立即全部显示
    Instant,
    /// 慢速 — 50ms/char，适合悬疑/悲伤场景
    Slow,
    /// 正常速度 — 30ms/char，默认档位，适合日常对话
    Normal,
    /// 快速 — 15ms/char，适合紧张/激动场景
    Fast,
    /// 自定义速度 — 由创作者指定 ms/char
    /// 值必须 >= 0，0 等同于 Instant
    Custom(f32),
}

impl TypewriterSpeed {
    /// 获取每字符间隔的毫秒数。
    ///
    /// # 返回值
    /// - `0.0` — 瞬时完成（Instant 或 Custom(0)）
    /// - 正值 — 对应速度档位的毫秒数
    pub fn ms_per_char(&self) -> f32 {
        match self {
            Self::Instant => 0.0,
            Self::Slow => 50.0,
            Self::Normal => 30.0,
            Self::Fast => 15.0,
            Self::Custom(ms) => ms.max(0.0),
        }
    }
}

// ============================================================================
// Typewriter — 打字机效果状态机
// ============================================================================

/// 打字机效果状态机 — 控制对话正文逐字显示的进度。
///
/// # 状态转换
/// ```text
/// reset(text)
///   ├─ visible_chars = 0, is_complete = false, is_skipped = false
///   │
///   ├─ update(dt) × N
///   │   └─ visible_chars 逐步推进到 total_chars → is_complete = true
///   │
///   └─ skip()
///       └─ visible_chars = total_chars → is_complete = true, is_skipped = true
/// ```
///
/// # 使用示例
/// ```rust,ignore
/// let mut tw = Typewriter::new(TypewriterSpeed::Normal);
/// tw.reset("今天天气真好啊。");
///
/// // 在主循环每帧调用
/// while !tw.is_complete() {
///     let dt = Duration::from_millis(16); // ~60fps
///     if tw.update(dt) {
///         // visible_chars 变化，更新 TextRenderer 可见范围
///         text_renderer.set_visible_range(0, tw.visible_chars());
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Typewriter {
    /// 当前显示速度
    speed: TypewriterSpeed,
    /// 当前文本总字符数（Unicode 标量值计数）
    total_chars: usize,
    /// 当前可见字符数（0..=total_chars）
    visible_chars: usize,
    /// 距上次推进一个字符以来的累积时间
    elapsed_since_last_char: Duration,
    /// 是否已显示全部字符
    is_complete: bool,
    /// 是否通过 skip() 跳过了动画
    is_skipped: bool,
}

impl Typewriter {
    /// 创建新的打字机状态机。
    ///
    /// # 参数
    /// - `speed`: 初始显示速度档位
    ///
    /// # 初始状态
    /// - `total_chars = 0`
    /// - `visible_chars = 0`
    /// - `is_complete = true`（空文本已"完成"）
    /// - `is_skipped = false`
    pub fn new(speed: TypewriterSpeed) -> Self {
        Self {
            speed,
            total_chars: 0,
            visible_chars: 0,
            elapsed_since_last_char: Duration::ZERO,
            is_complete: true, // 空文本视为已完成
            is_skipped: false,
        }
    }

    /// 重置状态机以适应新文本。
    ///
    /// 将 `visible_chars` 归零，`total_chars` 更新为新文本的 Unicode 标量值计数。
    /// 如果新文本为空字符串，则直接标记为完成。
    ///
    /// # 参数
    /// - `new_text`: 新的对话正文，字符计数基于 Unicode 标量值（`char`）
    pub fn reset(&mut self, new_text: &str) {
        self.total_chars = new_text.chars().count();
        self.visible_chars = 0;
        self.elapsed_since_last_char = Duration::ZERO;
        self.is_skipped = false;
        self.is_complete = self.total_chars == 0;
    }

    /// 每帧调用此方法推进打字机动画。
    ///
    /// 根据 `speed` 和累积时间判断是否推进一个字符。
    /// 当 `speed` 为 `Instant` 或 `Custom(0)` 时，立即显示全部文本。
    ///
    /// # 参数
    /// - `delta_time`: 自上一帧以来的时间增量
    ///
    /// # 返回值
    /// - `true` — `visible_chars` 发生变化（调用方应更新 TextRenderer 可见范围）
    /// - `false` — 无变化（动画已完成或时间未到）
    pub fn update(&mut self, delta_time: Duration) -> bool {
        // 已完成则无需推进
        if self.is_complete {
            return false;
        }

        let ms_per_char = self.speed.ms_per_char();

        // 瞬时速度：立即显示全部
        if ms_per_char <= 0.0 {
            self.visible_chars = self.total_chars;
            self.is_complete = true;
            return true;
        }

        self.elapsed_since_last_char += delta_time;

        // 使用微秒构建 Duration 以避免 f32 浮点精度问题
        // from_secs_f32(0.1) → 100,000,001 ns > 100ms，导致比较失败
        let char_interval = Duration::from_micros((ms_per_char * 1000.0) as u64);
        let mut changed = false;

        // 在当前帧可能推进多个字符（如果帧间隔 > char_interval）
        while self.elapsed_since_last_char >= char_interval && self.visible_chars < self.total_chars
        {
            self.visible_chars += 1;
            self.elapsed_since_last_char -= char_interval;
            changed = true;
        }

        // 检查是否已完成
        if self.visible_chars >= self.total_chars {
            self.visible_chars = self.total_chars;
            self.is_complete = true;
            self.elapsed_since_last_char = Duration::ZERO;
        }

        changed
    }

    /// 跳过当前打字机动画，立即显示全部剩余文本。
    ///
    /// 标记 `is_skipped = true` 和 `is_complete = true`。
    /// 如果动画已完成，调用此方法无副作用（幂等）。
    pub fn skip(&mut self) {
        if self.is_complete && !self.is_skipped {
            // 正常完成但未被标记为跳过，现在补充标记
            self.is_skipped = true;
            return;
        }
        if self.is_complete {
            return; // 已完成且已标记，幂等
        }
        self.visible_chars = self.total_chars;
        self.is_complete = true;
        self.is_skipped = true;
        self.elapsed_since_last_char = Duration::ZERO;
    }

    /// 获取当前可见字符数。
    ///
    /// 调用方将此值传递给 `TextRenderer::set_visible_range(0, count)`。
    pub fn visible_chars(&self) -> usize {
        self.visible_chars
    }

    /// 获取文本总字符数。
    pub fn total_chars(&self) -> usize {
        self.total_chars
    }

    /// 是否已显示全部字符（动画完成）。
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// 是否通过 `skip()` 跳过了动画。
    pub fn is_skipped(&self) -> bool {
        self.is_skipped
    }

    /// 获取当前显示速度。
    pub fn speed(&self) -> TypewriterSpeed {
        self.speed
    }

    /// 设置新的显示速度。
    ///
    /// 速度变更在下一帧的 `update()` 调用时生效。
    /// 如果设置为 `Instant` 或 `Custom(0)`，会在下次 `update()` 时立即完成。
    pub fn set_speed(&mut self, speed: TypewriterSpeed) {
        self.speed = speed;
    }
}

// ============================================================================
// 单元测试 — 覆盖 AC01-AC05
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：创建模拟时间推进的 Duration
    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    // ========================================================================
    // AC01 — 初始状态
    // ========================================================================

    /// AC01: Typewriter::new(Normal) 初始状态正确。
    /// - is_complete == true（空文本已完成）
    /// - visible_chars == 0
    /// - total_chars == 0
    #[test]
    fn ac01_initial_state() {
        let tw = Typewriter::new(TypewriterSpeed::Normal);
        assert!(tw.is_complete(), "空文本默认已完成");
        assert_eq!(tw.visible_chars(), 0);
        assert_eq!(tw.total_chars(), 0);
        assert!(!tw.is_skipped());
    }

    /// AC01: reset 后状态正确。
    #[test]
    fn ac01_reset_initial_state() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("今天天气真好啊。"); // 8 个字符
        assert!(!tw.is_complete(), "reset 后应为未完成");
        assert_eq!(tw.visible_chars(), 0);
        assert_eq!(tw.total_chars(), 8);
        assert!(!tw.is_skipped());
    }

    // ========================================================================
    // AC02 — 经过足够时间后文本全部显示
    // ========================================================================

    /// AC02: 10 字符 Normal(30ms/char) → 模拟 300ms → is_complete=true。
    #[test]
    fn ac02_progress_to_completion() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("1234567890"); // 10 个 ASCII 字符

        // 模拟 300ms 时间推进（10 × 30ms = 300ms）
        let changed = tw.update(ms(300));
        assert!(changed, "300ms 推进应产生变化");
        assert!(tw.is_complete(), "300ms 后应完成");
        assert_eq!(tw.visible_chars(), 10);
    }

    /// AC02: 逐步推进，每次一个字符。
    #[test]
    fn ac02_step_by_step() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("ABC"); // 3 个字符

        // 第 1 个字符：30ms
        assert!(tw.update(ms(30)));
        assert_eq!(tw.visible_chars(), 1);
        assert!(!tw.is_complete());

        // 30ms 不够推进下一个（累积仅 0ms after 上一次推进）
        assert!(!tw.update(ms(15)));
        assert_eq!(tw.visible_chars(), 1);

        // 再 15ms 凑够 30ms
        assert!(tw.update(ms(15)));
        assert_eq!(tw.visible_chars(), 2);

        // 第 3 个字符
        assert!(tw.update(ms(30)));
        assert_eq!(tw.visible_chars(), 3);
        assert!(tw.is_complete());
    }

    /// AC02: 大帧间隔一次推进多个字符。
    #[test]
    fn ac02_bulk_advance_in_one_frame() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("Hello"); // 5 个字符

        // 150ms = 5 × 30ms，一帧内全部推进
        assert!(tw.update(ms(150)));
        assert_eq!(tw.visible_chars(), 5);
        assert!(tw.is_complete());
    }

    // ========================================================================
    // AC03 — skip() 后文本立即全部显示
    // ========================================================================

    /// AC03: skip() 后立即完成，visible_chars == total_chars。
    #[test]
    fn ac03_skip_immediate() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("这是一段很长的文本用来测试跳过功能。"); // 17 字符

        // 先推进几个字符
        tw.update(ms(60)); // 推进 2 个字符（30ms × 2）
        assert!(tw.visible_chars() > 0);
        assert!(!tw.is_complete());

        // 跳过
        tw.skip();
        assert!(tw.is_complete(), "skip 后应完成");
        assert!(tw.is_skipped(), "skip 后 is_skipped 应为 true");
        assert_eq!(tw.visible_chars(), 18, "visible_chars 应等于 total_chars");
    }

    /// AC03: 未开始即 skip 也能正确完成。
    #[test]
    fn ac03_skip_before_start() {
        let mut tw = Typewriter::new(TypewriterSpeed::Slow);
        tw.reset("测试文本");

        // 不调用 update，直接 skip
        tw.skip();
        assert!(tw.is_complete());
        assert!(tw.is_skipped());
        assert_eq!(tw.visible_chars(), 4);
    }

    /// AC03: 完成后再次 skip 幂等。
    #[test]
    fn ac03_skip_idempotent() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("AB");
        tw.update(ms(60)); // 完成
        assert!(tw.is_complete());

        // 已完成但未被显式标记为 skipped
        tw.skip();
        assert!(tw.is_complete());
        assert!(tw.is_skipped());

        // 再次 skip 不 panic，状态不变
        tw.skip();
        assert!(tw.is_complete());
    }

    // ========================================================================
    // AC04 — reset() 后状态正确重置
    // ========================================================================

    /// AC04: 完成后 reset 新文本，状态正确重置。
    #[test]
    fn ac04_reset_after_completion() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("旧文本旧文本");
        tw.update(ms(500)); // 完成
        assert!(tw.is_complete());

        // 重置为新文本
        tw.reset("新文本"); // 3 个字符
        assert!(!tw.is_complete(), "reset 后 is_complete 应为 false");
        assert_eq!(tw.visible_chars(), 0);
        assert_eq!(tw.total_chars(), 3);
        assert!(!tw.is_skipped(), "reset 后 is_skipped 应为 false");
    }

    /// AC04: reset 到空文本应立即完成。
    #[test]
    fn ac04_reset_to_empty() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("有文本");
        assert!(!tw.is_complete());

        tw.reset("");
        assert!(tw.is_complete(), "空文本应直接完成");
        assert_eq!(tw.visible_chars(), 0);
        assert_eq!(tw.total_chars(), 0);
    }

    // ========================================================================
    // AC05 — 不同速度档位差异
    // ========================================================================

    /// AC05: 相同时间内 Slow > Normal > Fast 推进的字符数递减。
    #[test]
    fn ac05_speed_difference() {
        let test_text = "12345678901234567890"; // 20 个字符

        // Slow: 50ms/char → 300ms 推进 6 个字符
        let mut slow_tw = Typewriter::new(TypewriterSpeed::Slow);
        slow_tw.reset(test_text);
        slow_tw.update(ms(300));
        let slow_visible = slow_tw.visible_chars();

        // Normal: 30ms/char → 300ms 推进 10 个字符
        let mut normal_tw = Typewriter::new(TypewriterSpeed::Normal);
        normal_tw.reset(test_text);
        normal_tw.update(ms(300));
        let normal_visible = normal_tw.visible_chars();

        // Fast: 15ms/char → 300ms 推进 20 个字符（全部完成）
        let mut fast_tw = Typewriter::new(TypewriterSpeed::Fast);
        fast_tw.reset(test_text);
        fast_tw.update(ms(300));
        let fast_visible = fast_tw.visible_chars();

        // Slow(6) < Normal(10) < Fast(20)
        assert!(
            slow_visible < normal_visible,
            "Slow({slow_visible}) 应少于 Normal({normal_visible})"
        );
        assert!(
            normal_visible < fast_visible,
            "Normal({normal_visible}) 应少于 Fast({fast_visible})"
        );
        assert_eq!(fast_visible, 20, "Fast 300ms 应完成全部 20 字符");
    }

    // ========================================================================
    // 额外边界测试
    // ========================================================================

    /// Instant 速度：立即完成所有文本。
    #[test]
    fn test_instant_speed() {
        let mut tw = Typewriter::new(TypewriterSpeed::Instant);
        tw.reset("任意文本内容");

        let changed = tw.update(ms(0));
        assert!(changed);
        assert!(tw.is_complete());
        assert_eq!(tw.visible_chars(), 6);
    }

    /// Custom 速度正确。
    #[test]
    fn test_custom_speed() {
        let mut tw = Typewriter::new(TypewriterSpeed::Custom(100.0)); // 100ms/char
        tw.reset("ABC");

        // 100ms 推进 1 个字符
        assert!(tw.update(ms(100)));
        assert_eq!(tw.visible_chars(), 1);

        // 200ms more → 推进 2 个字符
        assert!(tw.update(ms(200)));
        assert_eq!(tw.visible_chars(), 3);
        assert!(tw.is_complete());
    }

    /// Custom(0) 等同于 Instant。
    #[test]
    fn test_custom_zero_equals_instant() {
        let mut tw = Typewriter::new(TypewriterSpeed::Custom(0.0));
        tw.reset("文本");
        tw.update(ms(0));
        assert!(tw.is_complete());
        assert_eq!(tw.visible_chars(), 2);
    }

    /// 空文本不 panic。
    #[test]
    fn test_zero_length_text() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("");

        assert!(tw.is_complete());
        assert_eq!(tw.total_chars(), 0);
        assert_eq!(tw.visible_chars(), 0);

        // update 空文本不 panic
        assert!(!tw.update(ms(100)));
        assert!(tw.is_complete());
    }

    /// 完成后继续 update 不溢出。
    #[test]
    fn test_update_after_complete() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("AB");
        tw.update(ms(60)); // 完成
        assert_eq!(tw.visible_chars(), 2);
        assert!(tw.is_complete());

        // 继续推进不 panic，不溢出
        assert!(!tw.update(ms(1000)));
        assert_eq!(tw.visible_chars(), 2, "完成后 visible_chars 不应变化");
    }

    /// CJK 文本字符计数正确。
    #[test]
    fn test_cjk_char_count() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);

        // 中文
        tw.reset("今天天气真好啊");
        assert_eq!(tw.total_chars(), 7);

        // 日文（含假名）
        tw.reset("こんにちは世界");
        assert_eq!(tw.total_chars(), 7);

        // 韩文
        tw.reset("안녕하세요");
        assert_eq!(tw.total_chars(), 5);

        // 混合
        tw.reset("Hello世界123！");
        assert_eq!(tw.total_chars(), 11); // H(1)e(2)l(3)l(4)o(5)世(6)界(7)1(8)2(9)3(10)！(11) = 11
    }

    /// 多字节 emoji 字符计数（char 级别）。
    #[test]
    fn test_emoji_char_count() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("Hello 😀🌍!");
        // H(1)e(2)l(3)l(4)o(5) (6)😀(7)🌍(8)!(9) = 9 chars
        assert_eq!(tw.total_chars(), 9);
    }

    /// set_speed 变更后生效。
    #[test]
    fn test_set_speed() {
        let mut tw = Typewriter::new(TypewriterSpeed::Slow);
        tw.reset("ABCDEFGHIJ"); // 10 字符

        // Slow: 50ms/char → 100ms 推进 2 字符
        tw.update(ms(100));
        assert_eq!(tw.visible_chars(), 2);

        // 切换到 Fast
        tw.set_speed(TypewriterSpeed::Fast);

        // Fast: 15ms/char → 150ms 推进 10 字符（剩余 8 个，150/15=10）
        tw.update(ms(150));
        assert_eq!(tw.visible_chars(), 10);
        assert!(tw.is_complete());
    }

    /// 正常完成（非 skip）is_skipped 为 false。
    #[test]
    fn test_normal_completion_not_skipped() {
        let mut tw = Typewriter::new(TypewriterSpeed::Fast);
        tw.reset("ABC");
        tw.update(ms(45)); // Fast 15ms × 3 = 45ms
        assert!(tw.is_complete());
        assert!(!tw.is_skipped(), "正常完成 is_skipped 应为 false");
    }

    /// update 返回值的正确性。
    #[test]
    fn test_update_return_value() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("AB");

        // 推进了一个字符
        assert!(tw.update(ms(30)), "推进字符应返回 true");
        // 时间不够
        assert!(!tw.update(ms(10)), "时间不够应返回 false");
        // 正好推进第二个字符
        assert!(tw.update(ms(20)), "凑够 30ms 推进应返回 true");
        // 已完成
        assert!(!tw.update(ms(1000)), "已完成应返回 false");
    }

    /// visible_chars 不会超过 total_chars。
    #[test]
    fn test_visible_chars_bounded() {
        let mut tw = Typewriter::new(TypewriterSpeed::Normal);
        tw.reset("AB");

        // 大量时间推进
        tw.update(ms(10000));
        assert_eq!(tw.visible_chars(), 2);
        assert!(tw.is_complete());
        assert!(tw.visible_chars() <= tw.total_chars());
    }
}
