//! @author 十四叔
//! @date 2026/07/25

//! 完成反馈视觉: 阶段流转时全屏 accent 色脉冲衰减。
//!
//! 时间由外部注入 (AnimationCtx.elapsed), 不读 wall-clock, 可完整单元测试。
//! 进行中触发会被忽略 (避免视觉抖), 多次跨过阶段终点只产生一次脉冲。

use std::time::Duration;

/// 完成反馈状态: None 表示未激活。
#[derive(Debug, Clone)]
pub struct FlashOverlay {
    /// 脉冲起点 (注入时间轴); None = 未激活或已结束。
    started: Option<Duration>,
    /// 脉冲总时长。
    duration: Duration,
}

impl FlashOverlay {
    /// 创建未激活的脉冲器, 默认 600ms 衰减。
    pub fn new(duration: Duration) -> Self {
        Self {
            started: None,
            duration,
        }
    }

    /// 触发一次脉冲。若当前正在脉冲中, 忽略 (避免视觉抖)。
    /// 已结束的脉冲可以再次触发 (允许连续阶段流转刷新)。
    pub fn trigger(&mut self, now: Duration) {
        if self.is_active(now) {
            return;
        }
        self.started = Some(now);
    }

    /// 当前脉冲进度: 1.0 (起点满 alpha) → 0.0 (终点透明); 线性衰减。
    /// 未激活或已结束 (超过 duration) 返回 None。
    pub fn progress(&self, now: Duration) -> Option<f32> {
        let start = self.started?;
        let elapsed = now.saturating_sub(start);
        let total = self.duration.as_secs_f32();
        if total <= 0.0 {
            return None;
        }
        if elapsed >= self.duration {
            return None;
        }
        Some(1.0 - elapsed.as_secs_f32() / total)
    }

    /// 脉冲是否正在播放 (用于 `trigger` 重复保护 + 调试日志)。
    pub fn is_active(&self, now: Duration) -> bool {
        self.progress(now).is_some()
    }
}

impl Default for FlashOverlay {
    fn default() -> Self {
        Self::new(Duration::from_millis(600))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn idle_overlay_returns_none() {
        let f = FlashOverlay::default();
        assert!(f.progress(ms(0)).is_none());
        assert!(f.progress(ms(999_999)).is_none());
        assert!(!f.is_active(ms(0)));
    }

    #[test]
    fn trigger_starts_at_full_progress() {
        let mut f = FlashOverlay::default();
        f.trigger(ms(1000));
        assert_eq!(f.progress(ms(1000)), Some(1.0));
    }

    #[test]
    fn progress_decays_linearly() {
        let mut f = FlashOverlay::new(ms(1000));
        f.trigger(ms(0));
        assert_eq!(f.progress(ms(0)), Some(1.0));
        assert_eq!(f.progress(ms(500)), Some(0.5));
        assert_eq!(f.progress(ms(750)), Some(0.25));
    }

    #[test]
    fn progress_returns_none_after_duration() {
        let mut f = FlashOverlay::new(ms(600));
        f.trigger(ms(0));
        assert!(f.progress(ms(600)).is_none());
        assert!(f.progress(ms(999_999)).is_none());
    }

    #[test]
    fn trigger_during_active_is_ignored() {
        let mut f = FlashOverlay::new(ms(1000));
        f.trigger(ms(0));
        // 100ms 后试图再次触发, 进度仍从 0 起算, 不被重置
        f.trigger(ms(100));
        assert_eq!(f.progress(ms(100)), Some(0.9));
    }

    #[test]
    fn trigger_after_duration_is_accepted() {
        // 验证: 已结束的脉冲可以再次触发 (不允许首次后永久死锁)。
        let mut f = FlashOverlay::new(ms(100));
        f.trigger(ms(0));
        // 100ms 后首次触发已结束
        assert!(f.progress(ms(100)).is_none());
        // 200ms 处再次触发, 应从满 alpha 起算
        f.trigger(ms(200));
        assert_eq!(f.progress(ms(200)), Some(1.0));
    }
}
