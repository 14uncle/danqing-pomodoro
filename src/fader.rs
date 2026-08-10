//! @author 十四叔
//! @date 2026/07/23

//! 场景淡化器 (纯逻辑)。
//!
//! 场景切换不是瞬时跳变: 旧场景与新场景在 ~800ms 内交叉淡化,
//! 文字/玻璃等 token 也按同一进度在两块调色板间插值 (潮汐式色调流动)。
//! 时间由外部注入 (`Duration` 累计值), 不读 wall-clock, 可完整单元测试。
//!
//! 中途再切换 (打断): 无法以单一索引表示混合中的画面,
//! 按进度占优的一侧吸附为新起点, 少数派贡献被舍弃 (有轻微跳变, POC 可接受)。

use std::time::Duration;

/// 场景交叉淡化状态。
#[derive(Debug, Clone)]
pub struct SceneFader {
    /// 淡化起点场景 (静止时与 `to` 相同)。
    from: usize,
    /// 淡化终点场景 (当前目标)。
    to: usize,
    /// 本次淡化开始时刻 (注入时间轴)。
    start: Duration,
    /// 淡化时长。
    duration: Duration,
}

impl SceneFader {
    /// 创建静止于某场景的淡化器。
    pub fn new(scene: usize, duration: Duration) -> Self {
        Self {
            from: scene,
            to: scene,
            start: Duration::ZERO,
            duration,
        }
    }

    /// 当前目标场景 (淡化结束后的场景)。
    pub fn current(&self) -> usize {
        self.to
    }

    /// 切换到目标场景; 若正在淡化中, 按进度占优侧吸附为新起点。
    pub fn switch_to(&mut self, target: usize, now: Duration) {
        if target == self.to {
            return;
        }
        let dominant = if self.progress(now) >= 0.5 {
            self.to
        } else {
            self.from
        };
        self.from = dominant;
        self.to = target;
        self.start = now;
    }

    /// 原始进度 (0..=1, 饱和); 静止时恒为 1。
    pub fn progress(&self, now: Duration) -> f32 {
        if self.from == self.to {
            return 1.0;
        }
        (now.saturating_sub(self.start).as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
    }

    /// 背景帧三元组 (from, to, eased fade)。
    ///
    /// fade 经缓动曲线整形, 端点精确为 0/1 (静止时 from == to, fade = 1)。
    pub fn frame(&self, now: Duration, easing: impl Fn(f32) -> f32) -> (usize, usize, f32) {
        (self.from, self.to, easing(self.progress(now)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn idle_fader_stays_on_initial_scene() {
        let f = SceneFader::new(2, ms(800));
        assert_eq!(f.current(), 2);
        assert!(f.progress(ms(0)) >= 1.0);
        assert!(f.progress(ms(999_999)) >= 1.0);
        assert_eq!(f.frame(ms(123), |t| t), (2, 2, 1.0));
    }

    #[test]
    fn switch_starts_fade_from_zero() {
        let mut f = SceneFader::new(0, ms(800));
        f.switch_to(1, ms(1000));
        assert_eq!(f.current(), 1);
        assert!(f.progress(ms(1000)) < 1.0);
        assert_eq!(f.progress(ms(1000)), 0.0);
        assert_eq!(f.frame(ms(1000), |t| t), (0, 1, 0.0));
    }

    #[test]
    fn fade_progresses_linearly_before_easing() {
        let mut f = SceneFader::new(0, ms(800));
        f.switch_to(1, ms(1000));
        assert!((f.progress(ms(1400)) - 0.5).abs() < 1e-6);
        assert_eq!(f.frame(ms(1400), |t| t), (0, 1, 0.5));
    }

    #[test]
    fn fade_completes_exactly_at_duration() {
        let mut f = SceneFader::new(0, ms(800));
        f.switch_to(1, ms(1000));
        assert_eq!(f.progress(ms(1800)), 1.0);
        assert!(f.progress(ms(1800)) >= 1.0);
        assert!(f.progress(ms(999_999)) >= 1.0);
    }

    #[test]
    fn frame_applies_easing_curve() {
        let mut f = SceneFader::new(0, ms(800));
        f.switch_to(1, ms(1000));
        // 中点 raw=0.5, ease-in-out 仍为 0.5; 1/4 处 raw=0.25, ease-in-out 更低。
        let (_, _, fade) = f.frame(ms(1200), |t| 4.0 * t * t * t);
        assert!((fade - 0.0625).abs() < 1e-6);
    }

    #[test]
    fn switch_to_same_scene_is_noop() {
        let mut f = SceneFader::new(0, ms(800));
        f.switch_to(1, ms(1000));
        f.switch_to(1, ms(1200));
        assert!((f.progress(ms(1400)) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn interrupt_early_snaps_back_to_origin() {
        let mut f = SceneFader::new(0, ms(800));
        f.switch_to(1, ms(1000));
        // 进度 0.25 (<0.5): 占优侧仍是 from=0, 新淡化从 0 起。
        f.switch_to(2, ms(1200));
        assert_eq!(f.frame(ms(1200), |t| t), (0, 2, 0.0));
    }

    #[test]
    fn interrupt_late_snaps_forward_to_target() {
        let mut f = SceneFader::new(0, ms(800));
        f.switch_to(1, ms(1000));
        // 进度 0.75 (>=0.5): 占优侧是 to=1, 新淡化从 1 起。
        f.switch_to(2, ms(1600));
        assert_eq!(f.frame(ms(1600), |t| t), (1, 2, 0.0));
    }
}
