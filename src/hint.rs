//! @author 十四叔
//! @date 2026/07/26

//! 首次启动快捷键提示: 一次性角落淡入/淡出动画状态机。
//!
//! 时间轴 (从 `triggered_at` 起):
//!   `[0, 1.5s)`           静默等待 (避免刚启动就抢戏)
//!   `[1.5s, 1.8s)`        淡入 300ms (cubic ease-out)
//!   `[1.8s, 6.8s)`        停留 5s (alpha = 1)
//!   `[6.8s, 7.3s)`        淡出 500ms (cubic ease-in)
//!   `>= 7.3s`             返回 None (提示结束)
//!
//! 时间由外部注入 (`AnimationCtx.elapsed`), 不读 wall-clock, 可完整单元测试。

use std::time::Duration;

/// 淡入前的静默延迟 (避免启动瞬间抢戏, 让用户先看到主界面)。
const FADE_IN_DELAY: Duration = Duration::from_millis(1500);
/// 淡入动画时长。
const FADE_IN_DURATION: Duration = Duration::from_millis(300);
/// 满 alpha 停留时长。
const HOLD_DURATION: Duration = Duration::from_millis(5000);
/// 淡出动画时长。
const FADE_OUT_DURATION: Duration = Duration::from_millis(500);
/// 总动画时长 (= FADE_IN_DELAY + FADE_IN_DURATION + HOLD_DURATION + FADE_OUT_DURATION)。
const TOTAL_DURATION: Duration = Duration::from_millis(7300);

/// 首次启动快捷键提示状态机。
#[derive(Debug, Clone)]
pub struct ShortcutHintOverlay {
    /// 触发起点 (注入时间轴); None = 未激活。
    triggered_at: Option<Duration>,
}

impl ShortcutHintOverlay {
    /// 创建未激活的提示 (默认不显示)。
    pub fn idle() -> Self {
        Self { triggered_at: None }
    }

    /// 在指定时刻触发提示 (通常传入 `boot_elapsed_offset`, 让动画从窗口启动那一刻起算)。
    pub fn triggered_at(at: Duration) -> Self {
        Self {
            triggered_at: Some(at),
        }
    }

    /// 当前 alpha (0.0 = 透明, 1.0 = 不透明)。
    /// 未激活或已结束返回 None (调用方应让文字完全不可见)。
    pub fn progress(&self, now: Duration) -> Option<f32> {
        let start = self.triggered_at?;
        let elapsed = now.saturating_sub(start);
        if elapsed >= TOTAL_DURATION {
            return None;
        }
        let e_ms = elapsed.as_secs_f32();
        let delay = FADE_IN_DELAY.as_secs_f32();
        let fade_in = FADE_IN_DURATION.as_secs_f32();
        let hold = HOLD_DURATION.as_secs_f32();
        let fade_out = FADE_OUT_DURATION.as_secs_f32();

        let alpha = if e_ms < delay {
            0.0
        } else if e_ms < delay + fade_in {
            let t = (e_ms - delay) / fade_in;
            ease_out_cubic(t)
        } else if e_ms < delay + fade_in + hold {
            1.0
        } else {
            let t = (e_ms - delay - fade_in - hold) / fade_out;
            1.0 - ease_in_cubic(t)
        };
        Some(alpha)
    }
}

/// 三次方缓出 (start fast, end slow): 用于淡入。
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// 三次方缓入 (start slow, end fast): 用于淡出。
fn ease_in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t.powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn idle_overlay_returns_none() {
        let hint = ShortcutHintOverlay::idle();
        assert!(hint.progress(ms(0)).is_none());
        assert!(hint.progress(ms(999_999)).is_none());
    }

    #[test]
    fn triggered_at_zero_is_zero_before_delay() {
        let hint = ShortcutHintOverlay::triggered_at(ms(0));
        assert_eq!(hint.progress(ms(0)), Some(0.0));
        assert_eq!(hint.progress(ms(1499)), Some(0.0));
    }

    #[test]
    fn fade_in_is_ease_out_and_reaches_full_at_1800ms() {
        let hint = ShortcutHintOverlay::triggered_at(ms(0));
        // 1.5s 时 alpha = 0; 1.65s 时约 0.5 (中点 ease-out 应略过半)
        assert_eq!(hint.progress(ms(1500)), Some(0.0));
        let mid = hint.progress(ms(1650)).unwrap();
        assert!(
            mid > 0.5 && mid < 0.9,
            "淡入中点应高于线性中点 0.5, 实际 {mid}"
        );
        // 1.8s 满 alpha
        assert_eq!(hint.progress(ms(1800)), Some(1.0));
    }

    #[test]
    fn hold_phase_keeps_alpha_at_one() {
        let hint = ShortcutHintOverlay::triggered_at(ms(0));
        assert_eq!(hint.progress(ms(1800)), Some(1.0));
        assert_eq!(hint.progress(ms(4000)), Some(1.0));
        assert_eq!(hint.progress(ms(6799)), Some(1.0));
    }

    #[test]
    fn fade_out_is_ease_in_drops_to_zero_at_7300ms() {
        let hint = ShortcutHintOverlay::triggered_at(ms(0));
        // 6.8s 仍为满
        assert_eq!(hint.progress(ms(6800)), Some(1.0));
        // 7.05s 是淡出窗口中点。ease-in 是「起手慢、收尾快」, 中点 alpha 应仍高 (≈0.875),
        // 与线性 (0.5) / ease-out (<0.5) 显著区分。
        let mid = hint.progress(ms(7050)).unwrap();
        assert!(
            mid > 0.7,
            "ease-in 淡出中点 alpha 应高于线性中点 (≈0.875), 实际 {mid}"
        );
        // 7.3s 完全透明 → 返回 None (动画结束)
        assert!(hint.progress(ms(7300)).is_none());
    }

    #[test]
    fn progress_returns_none_after_total_duration() {
        let hint = ShortcutHintOverlay::triggered_at(ms(0));
        assert!(hint.progress(ms(7300)).is_none());
        assert!(hint.progress(ms(999_999)).is_none());
    }

    #[test]
    fn triggered_at_nonzero_offset_shifts_curve() {
        // 触发点在 1000ms, 整条曲线向后平移 1s。
        let hint = ShortcutHintOverlay::triggered_at(ms(1000));
        assert_eq!(hint.progress(ms(1000)), Some(0.0));
        assert_eq!(hint.progress(ms(2499)), Some(0.0));
        assert_eq!(hint.progress(ms(2800)), Some(1.0));
        assert!(hint.progress(ms(8300)).is_none());
    }
}
