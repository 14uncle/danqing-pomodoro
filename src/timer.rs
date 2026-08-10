//! @author 十四叔
//! @date 2026/07/23

//! 番茄钟状态机 (纯逻辑)。
//!
//! 时间由外部注入 (`Duration` 累计值，通常来自 `AnimationCtx::elapsed`),
//! 不读 wall-clock, 可完整单元测试。语义：
//! - 专注 25:00 / 短休息 5:00, 每自然完成 4 轮专注进入长休息 15:00,
//!   阶段结束自动流转并自动开始下一阶段;
//! - `toggle` 在开始 / 暂停间切换 (开始即恢复);
//! - `reset` 回到专注 25:00 停止态，轮次计数清零;
//! - `skip` 手动跳阶段：不算完成，不推进轮次计数;
//! - tick 越过终点时余量带入下一阶段 (晚到的帧不吃时间)。

use std::time::Duration;

use serde::{Deserialize, Serialize};

// === 阶段时长常量 ===
/// 每自然完成多少轮专注进入一次长休息。
pub const CYCLE_LENGTH: u8 = 4;

/// 默认专注时长（秒）。
pub const DEFAULT_FOCUS_SECS: u64 = 25 * 60;
/// 默认短休息时长（秒）。
pub const DEFAULT_BREAK_SECS: u64 = 5 * 60;
/// 默认长休息时长（秒）。
pub const DEFAULT_LONG_BREAK_SECS: u64 = 15 * 60;

/// 可定制的计时时长配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerConfig {
    /// 专注时长（秒）。
    pub focus_secs: u64,
    /// 短休息时长（秒）。
    pub break_secs: u64,
    /// 长休息时长（秒）。
    pub long_break_secs: u64,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            focus_secs: DEFAULT_FOCUS_SECS,
            break_secs: DEFAULT_BREAK_SECS,
            long_break_secs: DEFAULT_LONG_BREAK_SECS,
        }
    }
}

impl TimerConfig {
    /// 约束到有效范围 [1 分钟, 3 小时]。
    pub fn clamp(mut self) -> Self {
        self.focus_secs = self.focus_secs.clamp(60, 10_800);
        self.break_secs = self.break_secs.clamp(60, 10_800);
        self.long_break_secs = self.long_break_secs.clamp(60, 10_800);
        self
    }
}

/// 计时阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// 专注 (25 分钟)。
    Focus,
    /// 短休息 (5 分钟)。
    Break,
    /// 长休息 (15 分钟，每 4 轮专注后)。
    LongBreak,
}

impl Phase {
    /// 阶段时长（由 config 定制）。
    pub fn duration(self, config: &TimerConfig) -> Duration {
        match self {
            Self::Focus => Duration::from_secs(config.focus_secs),
            Self::Break => Duration::from_secs(config.break_secs),
            Self::LongBreak => Duration::from_secs(config.long_break_secs),
        }
    }

    /// 下一阶段 (自然完成语义): 返回 (新阶段，新轮次计数)。
    /// Focus 完成计一次专注：满 `CYCLE_LENGTH` 进 `LongBreak` 且轮次清零，
    /// 否则进 `Break`; 两类休息完成都回 `Focus`, 轮次计数不变。
    pub fn next(self, completed_focus: u8) -> (Self, u8) {
        match self {
            Self::Focus => {
                let done = completed_focus + 1;
                if done >= CYCLE_LENGTH {
                    (Self::LongBreak, 0)
                } else {
                    (Self::Break, done)
                }
            }
            Self::Break | Self::LongBreak => (Self::Focus, completed_focus),
        }
    }

    /// 手动跳过的目标相位：skip 不算完成，不推进轮次计数。
    fn skip_target(self) -> Self {
        match self {
            Self::Focus => Self::Break,
            Self::Break | Self::LongBreak => Self::Focus,
        }
    }

    /// 中文标注。
    pub fn label(self) -> &'static str {
        match self {
            Self::Focus => "专注",
            Self::Break => "休息",
            Self::LongBreak => "长休息",
        }
    }
}

/// 运行状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// 停止 (未开始或被重置)。
    Idle,
    /// 计时中。
    Running,
    /// 暂停 (剩余时间已快照)。
    Paused,
}

/// `tick` 报告：阶段是否流转 + 本帧自然完成的专注数。
/// `focus_completions` 是轮次计数与今日计数的统一数据源; skip 不经过它。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TickReport {
    /// 是否发生了阶段流转。
    pub advanced: bool,
    /// 本帧自然完成的专注阶段数 (huge overshoot 可能 >1)。
    pub focus_completions: u8,
    /// 最后一次自然完成的专注在大循环内的轮次 (1..=CYCLE_LENGTH)。
    /// 无完成时为 0。供会话历史记录「轮次」字段 (2026-08-01 数据层新增)。
    pub completed_round: u8,
}

/// 番茄钟状态机。
#[derive(Debug, Clone)]
pub struct Pomodoro {
    phase: Phase,
    run: Run,
    /// 非 Running 时的剩余时间快照; Running 时由 deadline 推算。
    remaining: Duration,
    /// Running 时的截止点 (注入时间轴上的绝对位置)。
    deadline: Option<Duration>,
    /// 当前大循环内已自然完成的专注数 (0..CYCLE_LENGTH)。
    completed_focus: u8,
    /// 可定制的计时时长配置。
    config: TimerConfig,
}

impl Pomodoro {
    /// 创建番茄钟：专注 25:00 停止态，使用默认配置。
    pub fn new() -> Self {
        Self::with_config(TimerConfig::default())
    }

    /// 用自定义配置创建番茄钟。
    pub fn with_config(config: TimerConfig) -> Self {
        let config = config.clamp();
        Self {
            phase: Phase::Focus,
            run: Run::Idle,
            remaining: Phase::Focus.duration(&config),
            deadline: None,
            completed_focus: 0,
            config,
        }
    }

    /// 从持久化恢复任意状态 (用于跨重启恢复)。
    pub fn restore(
        phase: Phase,
        run: Run,
        remaining: Duration,
        deadline: Option<Duration>,
        completed_focus: u8,
        config: TimerConfig,
    ) -> Self {
        Self {
            phase,
            run,
            remaining,
            deadline,
            completed_focus,
            config: config.clamp(),
        }
    }

    /// 当前阶段。
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// 当前运行状态。
    pub fn run(&self) -> Run {
        self.run
    }

    /// 是否计时中。
    pub fn is_running(&self) -> bool {
        self.run == Run::Running
    }

    /// 当前计时配置。
    pub fn config(&self) -> &TimerConfig {
        &self.config
    }

    /// 更新计时配置。Running/Paused 时当前 phase 保持旧时长（下一 phase 才生效）;
    /// Idle 时立即刷新 remaining 快照。
    pub fn update_config(&mut self, config: TimerConfig) {
        self.config = config.clamp();
        if self.run == Run::Idle {
            self.remaining = self.phase.duration(&self.config);
        }
    }

    /// 当前大循环内已自然完成的专注数 (0..CYCLE_LENGTH)。
    pub fn completed_focus(&self) -> u8 {
        self.completed_focus
    }

    /// 开始 / 暂停切换：Idle 或 Paused 进入计时，Running 快照剩余并暂停。
    pub fn toggle(&mut self, now: Duration) {
        match self.run {
            Run::Idle | Run::Paused => {
                self.deadline = Some(now + self.remaining);
                self.run = Run::Running;
            }
            Run::Running => {
                self.remaining = self.remaining_at(now);
                self.deadline = None;
                self.run = Run::Paused;
            }
        }
    }

    /// 重置：回到专注停止态，轮次计数清零，保留时长配置。
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        let config = self.config;
        *self = Self::with_config(config);
    }

    /// 立即跳过当前阶段剩余时间，进入下一阶段。
    /// skip 不算完成：不推进轮次计数 (Focus 被跳过一律去 `Break`)。
    /// - Running: `deadline = now + next_phase.duration` (从满量重新开始)
    /// - Paused / Idle: `remaining = next_phase.duration`, `deadline = None`
    ///
    /// 返回是否发生阶段切换 (始终为 true, 排除自身相等)。
    pub fn skip(&mut self, now: Duration) -> bool {
        self.phase = self.phase.skip_target();
        let next_duration = self.phase.duration(&self.config);
        match self.run {
            Run::Running => {
                self.deadline = Some(now + next_duration);
            }
            Run::Paused | Run::Idle => {
                self.remaining = next_duration;
                self.deadline = None;
            }
        }
        true
    }

    /// 推进计时; 越过阶段终点时自动流转并自动开始下一阶段。
    ///
    /// 返回 [`TickReport`]: `advanced` 表示是否发生阶段流转;
    /// `focus_completions` 为本帧自然完成的专注数 (轮次/今日计数数据源)。
    /// 余量带入下一阶段 (deadline 顺延), 连续越过多个终点时循环处理。
    pub fn tick(&mut self, now: Duration) -> TickReport {
        let mut report = TickReport::default();
        while self.run == Run::Running && now >= self.deadline.unwrap_or(now) {
            let deadline = self.deadline.unwrap_or(now);
            if self.phase == Phase::Focus {
                report.focus_completions += 1;
                // 记录本次完成的轮次 (pre-advance 的 completed_focus + 1)。
                report.completed_round = self.completed_focus + 1;
            }
            let (next_phase, next_count) = self.phase.next(self.completed_focus);
            self.phase = next_phase;
            self.completed_focus = next_count;
            self.deadline = Some(deadline + self.phase.duration(&self.config));
            report.advanced = true;
        }
        if report.advanced {
            self.remaining = self.remaining_at(now);
        }
        report
    }

    /// 当前剩余时间。
    pub fn remaining(&self, now: Duration) -> Duration {
        match self.run {
            Run::Running => self.remaining_at(now),
            _ => self.remaining,
        }
    }

    /// `mm:ss` 显示 (剩余秒数向下取整)。
    pub fn display(&self, now: Duration) -> String {
        let secs = self.remaining(now).as_secs();
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }

    /// Running 状态下由 deadline 推算剩余 (饱和减法)。
    fn remaining_at(&self, now: Duration) -> Duration {
        self.deadline.unwrap_or(now).saturating_sub(now)
    }
}

impl Default for Pomodoro {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn new_is_focus_idle() {
        let p = Pomodoro::new();
        assert_eq!(p.phase(), Phase::Focus);
        assert!(!p.is_running());
        assert_eq!(p.remaining(secs(0)), secs(DEFAULT_FOCUS_SECS));
        assert_eq!(p.display(secs(0)), "25:00");
    }

    #[test]
    fn toggle_starts_then_pauses() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        assert!(p.is_running());
        let pause_at = DEFAULT_FOCUS_SECS / 2;
        p.toggle(secs(pause_at));
        assert!(!p.is_running());
        assert_eq!(
            p.remaining(secs(pause_at)),
            secs(DEFAULT_FOCUS_SECS - pause_at)
        );
    }

    #[test]
    fn paused_remaining_is_frozen() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        p.toggle(secs(DEFAULT_FOCUS_SECS / 2));
        // 暂停后时间推移不改变剩余。
        let remaining_at_pause = DEFAULT_FOCUS_SECS - DEFAULT_FOCUS_SECS / 2;
        assert_eq!(p.remaining(secs(999)), secs(remaining_at_pause));
    }

    #[test]
    fn resume_continues_from_paused_remaining() {
        let mut p = Pomodoro::new();
        let pause_at = DEFAULT_FOCUS_SECS / 2;
        p.toggle(secs(0));
        p.toggle(secs(pause_at));
        p.toggle(secs(999)); // 很久后恢复
        assert!(p.is_running());
        // 恢复后 1s 内剩余 = 暂停时剩余 - 1
        assert_eq!(
            p.remaining(secs(1000)),
            secs(DEFAULT_FOCUS_SECS - pause_at - 1)
        );
    }

    #[test]
    fn tick_before_deadline_does_not_advance() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        assert!(!p.tick(secs(DEFAULT_FOCUS_SECS - 1)).advanced);
        assert_eq!(p.phase(), Phase::Focus);
    }

    #[test]
    fn tick_past_deadline_auto_advances_and_keeps_running() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        assert!(p.tick(secs(DEFAULT_FOCUS_SECS)).advanced);
        assert_eq!(p.phase(), Phase::Break);
        assert!(p.is_running());
        assert_eq!(
            p.remaining(secs(DEFAULT_FOCUS_SECS)),
            secs(DEFAULT_BREAK_SECS)
        );
    }

    #[test]
    fn overshoot_carries_into_next_phase() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        // 帧晚到 3 秒：下一阶段从原终点顺延，余量不亏。
        let overshoot = 3u64;
        let tick_at = DEFAULT_FOCUS_SECS + overshoot;
        assert!(p.tick(secs(tick_at)).advanced);
        assert_eq!(
            p.remaining(secs(tick_at)),
            secs(DEFAULT_BREAK_SECS - overshoot)
        );
    }

    #[test]
    fn break_completion_returns_to_focus() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        p.tick(secs(DEFAULT_FOCUS_SECS));
        let cycle = DEFAULT_FOCUS_SECS + DEFAULT_BREAK_SECS;
        assert!(p.tick(secs(cycle)).advanced);
        assert_eq!(p.phase(), Phase::Focus);
        assert!(p.is_running());
        assert_eq!(p.remaining(secs(cycle)), secs(DEFAULT_FOCUS_SECS));
    }

    #[test]
    fn huge_overshoot_rolls_multiple_phases() {
        // 在第二轮 break, 剩 2 分钟 (focus 2: 30-55, break 2: 55-60, 60-58=2)
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        let cycle = DEFAULT_FOCUS_SECS + DEFAULT_BREAK_SECS;
        let tick_at = 2 * cycle - 2 * 60; // 2 个完整 cycle 减 2 分钟 = 58 分钟 = 3480s
        assert!(p.tick(secs(tick_at)).advanced);
        assert_eq!(p.phase(), Phase::Break);
        assert_eq!(p.remaining(secs(tick_at)), secs(2 * 60));
    }

    #[test]
    fn reset_returns_to_focus_idle() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        p.tick(secs(DEFAULT_FOCUS_SECS));
        p.reset();
        assert_eq!(p.phase(), Phase::Focus);
        assert!(!p.is_running());
        assert_eq!(p.remaining(secs(999)), secs(DEFAULT_FOCUS_SECS));
    }

    #[test]
    fn display_formats_mm_ss() {
        let mut p = Pomodoro::new();
        assert_eq!(p.display(secs(0)), "25:00");
        p.toggle(secs(0));
        assert_eq!(p.display(secs(1)), "24:59");
    }

    #[test]
    fn phase_labels_are_chinese() {
        assert_eq!(Phase::Focus.label(), "专注");
        assert_eq!(Phase::Break.label(), "休息");
    }

    #[test]
    fn restore_preserves_all_fields() {
        let p = Pomodoro::restore(
            Phase::Break,
            Run::Paused,
            Duration::from_secs(120),
            None,
            2,
            TimerConfig::default(),
        );
        assert_eq!(p.phase(), Phase::Break);
        assert_eq!(p.run(), Run::Paused);
        assert!(!p.is_running());
        assert_eq!(p.completed_focus(), 2);
        assert_eq!(
            p.remaining(Duration::from_secs(9999)),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn restore_cycle_count_continues_cycle() {
        // 恢复第 4 轮 (completed_focus=3): 自然完成 Focus 应直接进 LongBreak。
        let mut p = Pomodoro::restore(
            Phase::Focus,
            Run::Running,
            secs(60),
            Some(secs(60)),
            3,
            TimerConfig::default(),
        );
        p.tick(secs(60));
        assert_eq!(p.phase(), Phase::LongBreak);
    }

    #[test]
    fn restore_running_with_deadline_resumes_correctly() {
        let now = Duration::from_secs(1000);
        let p = Pomodoro::restore(
            Phase::Focus,
            Run::Running,
            Duration::from_secs(600),
            Some(now + Duration::from_secs(600)),
            0,
            TimerConfig::default(),
        );
        assert!(p.is_running());
        assert_eq!(p.remaining(now), Duration::from_secs(600));
    }

    #[test]
    fn skip_in_running_advances_deadline() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        let skip_at = DEFAULT_FOCUS_SECS / 2;
        assert!(p.skip(secs(skip_at)));
        assert_eq!(p.phase(), Phase::Break);
        assert!(p.is_running());
        // 新阶段从 now 起算，满量 DEFAULT_BREAK_SECS
        assert_eq!(p.remaining(secs(skip_at)), secs(DEFAULT_BREAK_SECS));
    }

    #[test]
    fn skip_in_paused_advances_remaining() {
        let mut p = Pomodoro::new();
        let skip_at = DEFAULT_FOCUS_SECS / 2;
        p.toggle(secs(0));
        p.toggle(secs(skip_at)); // 暂停
        assert!(p.skip(secs(skip_at)));
        assert_eq!(p.phase(), Phase::Break);
        assert!(!p.is_running());
        // 暂停态下 remaining = 下一阶段满量
        assert_eq!(p.remaining(secs(999)), secs(DEFAULT_BREAK_SECS));
    }

    #[test]
    fn skip_in_idle_advances_phase_and_remaining() {
        let mut p = Pomodoro::new();
        assert!(p.skip(secs(0)));
        assert_eq!(p.phase(), Phase::Break);
        assert!(!p.is_running());
        assert_eq!(p.remaining(secs(999)), secs(DEFAULT_BREAK_SECS));
    }

    #[test]
    fn skip_consecutive_cycles_through_phases() {
        let mut p = Pomodoro::new();
        assert!(p.skip(secs(0)));
        assert_eq!(p.phase(), Phase::Break);
        assert!(p.skip(secs(0)));
        assert_eq!(p.phase(), Phase::Focus);
        assert!(p.skip(secs(0)));
        assert_eq!(p.phase(), Phase::Break);
    }

    // === 长休息 + 轮次 (打磨 WS2) ===

    const LONG_DEFAULT_BREAK_SECS: u64 = 15 * 60; // 15 分钟

    /// 测试辅助：从当前时刻推进一个 Focus + Break 短周期 (各在终点 tick 一次)。
    fn run_short_cycle(p: &mut Pomodoro, now: &mut u64) {
        *now += DEFAULT_FOCUS_SECS;
        p.tick(secs(*now));
        *now += DEFAULT_BREAK_SECS;
        p.tick(secs(*now));
    }

    #[test]
    fn fourth_focus_completion_enters_long_break() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        let mut now = 0u64;
        // 前三轮：Focus 完成 → Break
        for _ in 0..3 {
            run_short_cycle(&mut p, &mut now);
            assert_eq!(p.phase(), Phase::Focus);
        }
        // 第四个 Focus 完成 → LongBreak (满 15 分钟)
        now += DEFAULT_FOCUS_SECS;
        let report = p.tick(secs(now));
        assert_eq!(report.focus_completions, 1);
        assert_eq!(p.phase(), Phase::LongBreak);
        assert_eq!(p.remaining(secs(now)), secs(LONG_DEFAULT_BREAK_SECS));
    }

    #[test]
    fn long_break_completion_returns_to_focus_and_resets_cycle() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        let mut now = 0u64;
        for _ in 0..3 {
            run_short_cycle(&mut p, &mut now);
        }
        now += DEFAULT_FOCUS_SECS;
        p.tick(secs(now));
        assert_eq!(p.phase(), Phase::LongBreak);
        // 长休息完成：回 Focus, 不算专注完成，轮次已清零
        now += LONG_DEFAULT_BREAK_SECS;
        let report = p.tick(secs(now));
        assert_eq!(report.focus_completions, 0);
        assert_eq!(p.phase(), Phase::Focus);
        assert_eq!(p.completed_focus(), 0);
        // 新一轮从第 1 轮计起：再次完成 Focus 应去 Break 而非 LongBreak
        now += DEFAULT_FOCUS_SECS;
        p.tick(secs(now));
        assert_eq!(p.phase(), Phase::Break);
    }

    #[test]
    fn skip_focus_goes_to_break_without_counting() {
        // 完成 3 轮后 skip 第 4 个 Focus: 去 Break 而非 LongBreak, 轮次计数保持。
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        let mut now = 0u64;
        for _ in 0..3 {
            run_short_cycle(&mut p, &mut now);
        }
        assert_eq!(p.completed_focus(), 3);
        p.skip(secs(now + 60));
        assert_eq!(p.phase(), Phase::Break);
        assert_eq!(p.completed_focus(), 3);
        // 该 Break 完成后回 Focus; 自然完成仍应触发 LongBreak (第 4 轮)。
        now += 60 + DEFAULT_BREAK_SECS;
        p.tick(secs(now));
        assert_eq!(p.phase(), Phase::Focus);
        now += DEFAULT_FOCUS_SECS;
        p.tick(secs(now));
        assert_eq!(p.phase(), Phase::LongBreak);
    }

    #[test]
    fn reset_clears_cycle_count() {
        // 完成 1 轮后 reset: 重新走满 4 轮才进 LongBreak。
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        p.tick(secs(DEFAULT_FOCUS_SECS));
        assert_eq!(p.completed_focus(), 1);
        p.reset();
        assert_eq!(p.completed_focus(), 0);
        p.toggle(secs(0));
        let mut now = 0u64;
        for _ in 0..3 {
            run_short_cycle(&mut p, &mut now);
        }
        now += DEFAULT_FOCUS_SECS;
        p.tick(secs(now));
        assert_eq!(p.phase(), Phase::LongBreak);
    }

    #[test]
    fn overshoot_accumulates_focus_completions() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        // 一次 tick 跨越 2 个 Focus: F(1500) + B(300) + F(1500) = 3300。
        let report = p.tick(secs(3300));
        assert_eq!(report.focus_completions, 2);
        assert_eq!(p.phase(), Phase::Break);
    }

    #[test]
    fn tick_report_defaults_when_no_advance() {
        let mut p = Pomodoro::new();
        let report = p.tick(secs(100));
        assert!(!report.advanced);
        assert_eq!(report.focus_completions, 0);
        assert_eq!(report.completed_round, 0, "无完成时 completed_round 应为 0");
    }

    #[test]
    fn completed_round_reflects_round_within_cycle() {
        // 完成 2 个 Focus (第 1、2 轮): completed_round 应分别为 1、2。
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        let r1 = p.tick(secs(DEFAULT_FOCUS_SECS));
        assert_eq!(r1.focus_completions, 1);
        assert_eq!(r1.completed_round, 1);
        let r2 = p.tick(secs(
            DEFAULT_FOCUS_SECS + DEFAULT_BREAK_SECS + DEFAULT_FOCUS_SECS,
        ));
        assert_eq!(r2.focus_completions, 1);
        assert_eq!(r2.completed_round, 2);
    }

    #[test]
    fn completed_round_fourth_focus_is_four() {
        // 第 4 轮 Focus 完成 → LongBreak, completed_round 应为 4 (非清零后的 0)。
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        let mut now = 0u64;
        for _ in 0..3 {
            run_short_cycle(&mut p, &mut now);
        }
        now += DEFAULT_FOCUS_SECS;
        let report = p.tick(secs(now));
        assert_eq!(p.phase(), Phase::LongBreak);
        assert_eq!(report.completed_round, 4);
    }

    #[test]
    fn completed_round_multi_completion_reports_last_round() {
        // huge overshoot 一次跨 2 个 Focus: completed_round 为最后一次的轮次。
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        let report = p.tick(secs(3300)); // F + B + F
        assert_eq!(report.focus_completions, 2);
        assert_eq!(report.completed_round, 2);
    }

    #[test]
    fn long_break_label_and_duration() {
        assert_eq!(Phase::LongBreak.label(), "长休息");
        assert_eq!(
            Phase::LongBreak.duration(&TimerConfig::default()),
            secs(DEFAULT_LONG_BREAK_SECS)
        );
    }

    #[test]
    fn phase_next_transition_matrix() {
        assert_eq!(Phase::Focus.next(0), (Phase::Break, 1));
        assert_eq!(Phase::Focus.next(2), (Phase::Break, 3));
        assert_eq!(Phase::Focus.next(3), (Phase::LongBreak, 0));
        assert_eq!(Phase::Break.next(1), (Phase::Focus, 1));
        assert_eq!(Phase::LongBreak.next(0), (Phase::Focus, 0));
    }

    // === 自定义计时配置 ===

    #[test]
    fn timer_config_default_is_25_5_15() {
        let c = TimerConfig::default();
        assert_eq!(c.focus_secs, 25 * 60);
        assert_eq!(c.break_secs, 5 * 60);
        assert_eq!(c.long_break_secs, 15 * 60);
    }

    #[test]
    fn timer_config_clamp_below_minimum() {
        let c = TimerConfig {
            focus_secs: 30,
            break_secs: 10,
            long_break_secs: 5,
        }
        .clamp();
        assert_eq!(c.focus_secs, 60);
        assert_eq!(c.break_secs, 60);
        assert_eq!(c.long_break_secs, 60);
    }

    #[test]
    fn timer_config_clamp_above_maximum() {
        let c = TimerConfig {
            focus_secs: 20_000,
            break_secs: 20_000,
            long_break_secs: 20_000,
        }
        .clamp();
        assert_eq!(c.focus_secs, 10_800);
        assert_eq!(c.break_secs, 10_800);
        assert_eq!(c.long_break_secs, 10_800);
    }

    #[test]
    fn custom_config_changes_phase_duration() {
        let config = TimerConfig {
            focus_secs: 10 * 60,
            break_secs: 3 * 60,
            long_break_secs: 12 * 60,
        };
        let p = Pomodoro::with_config(config);
        assert_eq!(p.phase(), Phase::Focus);
        assert_eq!(p.remaining(secs(0)), secs(10 * 60));
    }

    #[test]
    fn update_config_idle_takes_effect_immediately() {
        let mut p = Pomodoro::new();
        let config = TimerConfig {
            focus_secs: 10 * 60,
            break_secs: 2 * 60,
            long_break_secs: 8 * 60,
        };
        p.update_config(config);
        assert_eq!(p.config().focus_secs, 10 * 60);
        // Idle 状态下 remaining 立即刷新
        assert_eq!(p.remaining(secs(0)), secs(10 * 60));
    }

    #[test]
    fn update_config_mid_session_preserves_current_phase() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        // 跑了 10 分钟，剩 15 分钟
        p.tick(secs(10 * 60));
        assert_eq!(p.remaining(secs(10 * 60)), secs(15 * 60));
        // 中途改 config：当前 phase 剩余不变
        let custom = TimerConfig {
            focus_secs: 5 * 60,
            break_secs: 1 * 60,
            long_break_secs: 2 * 60,
        };
        p.update_config(custom);
        // 剩余仍以旧时长 (25min) 为基准
        assert_eq!(p.remaining(secs(10 * 60)), secs(15 * 60));
    }

    #[test]
    fn update_config_applied_to_next_phase() {
        let mut p = Pomodoro::new();
        p.toggle(secs(0));
        // 改 config 为 10min focus / 2min break
        let custom = TimerConfig {
            focus_secs: 10 * 60,
            break_secs: 2 * 60,
            long_break_secs: 15 * 60,
        };
        p.update_config(custom);
        // tick 到 25min（原 focus 终点）：新 phase 用 custom.break_secs
        p.tick(secs(25 * 60));
        assert_eq!(p.phase(), Phase::Break);
        assert_eq!(p.remaining(secs(25 * 60)), secs(2 * 60));
    }

    #[test]
    fn reset_preserves_config() {
        let mut p = Pomodoro::with_config(TimerConfig {
            focus_secs: 30 * 60,
            break_secs: 10 * 60,
            long_break_secs: 20 * 60,
        });
        p.toggle(secs(0));
        p.tick(secs(30 * 60)); // 完成一个 focus
        p.reset();
        assert_eq!(p.config().focus_secs, 30 * 60);
        assert_eq!(p.phase(), Phase::Focus);
        assert!(!p.is_running());
        assert_eq!(p.remaining(secs(999)), secs(30 * 60));
    }

    #[test]
    fn restore_with_config_preserves_all() {
        let config = TimerConfig {
            focus_secs: 15 * 60,
            break_secs: 3 * 60,
            long_break_secs: 10 * 60,
        };
        let p = Pomodoro::restore(
            Phase::Break,
            Run::Paused,
            Duration::from_secs(90),
            None,
            2,
            config,
        );
        assert_eq!(p.config().focus_secs, 15 * 60);
        assert_eq!(p.config().break_secs, 3 * 60);
        assert_eq!(p.config().long_break_secs, 10 * 60);
        assert_eq!(p.phase(), Phase::Break);
        assert!(!p.is_running());
    }

    #[test]
    fn skip_uses_config_for_next_phase() {
        let mut p = Pomodoro::with_config(TimerConfig {
            focus_secs: 10 * 60,
            break_secs: 2 * 60,
            long_break_secs: 8 * 60,
        });
        p.skip(secs(0));
        assert_eq!(p.phase(), Phase::Break);
        assert_eq!(p.remaining(secs(0)), secs(2 * 60));
    }
}
