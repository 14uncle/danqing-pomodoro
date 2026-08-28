//! @author 十四叔
//! @date 2026/07/23

//! 丹青番茄钟 POC —— 专注陪伴工具 × 场景沉浸。
//!
//! 最小番茄钟 (固定 25/5, 开始/暂停/重置) + 场景沉浸：
//! 场景大图为主角，中央大字倒计时，底部玻璃胶囊控件条，
//! 场景 前/后 切换带 800ms 交叉淡化，色调随场景调色板流动。

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod ambient;
mod audio;
mod fader;
mod flash;
mod hint;
mod motion;
mod scenes;
mod state;
mod stats;
mod timer;
mod today;
mod tray;

use chrono::Datelike;

use std::process::ExitCode;
use std::time::Duration;

use danqing::widget::{
    self, Box as UiBox, Button, Center, CloseButton, Column, LogoKind, MultiPanel, Node, Padding,
    Row, Stack, Text, TitleBar,
};
use danqing::{
    AnimationCtx, App, BackgroundConfig, BackgroundFrame, Color, Easing, Edges, Event, Key,
    LightTheme, NamedKey, ScaleMode, ScenePalette, SceneTheme, Size, Theme, WindowAction,
    WindowConfig, WindowEventSender, hotkey_ids, shortcut_for_id, tray_action_ids,
};
use fader::SceneFader;
use flash::FlashOverlay;
use hint::ShortcutHintOverlay;
use scenes::SCENES;
use state::{PomodoroState, RunState, current_wall_secs, load_state, save_state};
use stats::{FocusHistory, SessionRecord};
use timer::{Phase, Pomodoro, Run};
use tray::build_menu;

/// 完成反馈视觉脉冲时长 (头部满 → 尾部透明)。
const FLASH_DURATION: Duration = Duration::from_millis(600);

/// 场景交叉淡化时长 (spec: 600~1000ms)。
const FADE_DURATION: Duration = Duration::from_millis(800);
/// 持久化节流间隔：state_dirty 为 true 时，距上次保存超过此间隔才落盘。
const SAVE_THROTTLE: Duration = Duration::from_secs(1);

/// 淡化缓动曲线 (淡入淡出两端柔和)。
const FADE_EASING: Easing = Easing::EaseInOut;
/// 设置面板卡片宽度。
const SETTINGS_CARD_WIDTH: f32 = 300.0;
/// 报告面板卡片宽度 (略宽于统计卡，容纳近 12 月趋势)。
const REPORT_CARD_WIDTH: f32 = 360.0;
/// 设置面板标题与关闭按钮间距。
const SETTINGS_HEADER_GAP: f32 = 150.0;
/// 设置面板步进器数值显示宽度。
const STEPPER_VALUE_WIDTH: f32 = 72.0;
/// 减号按钮相对标签的偏移量：把 [-] 从标签右侧推开 15px (视觉微调)。
const STEPPER_MINUS_OFFSET: f32 = 15.0;

/// 番茄钟应用状态。
struct PomodoroApp {
    /// 计时状态机 (纯逻辑)。
    timer: Pomodoro,
    /// 注入时间轴：自应用启动的累计时间 (由 tick 心跳推进)。
    now: Duration,
    /// 场景交叉淡化器 (含当前场景索引)。
    fader: SceneFader,
    /// 启动时 elapsed 偏移 (持久化恢复); 0 表示全新会话。
    now_offset: Duration,
    /// 状态脏旗标：update 触发，tick 节流落盘后清零。
    state_dirty: bool,
    /// 最近一次成功落盘的 now 值 (节流基准)。
    last_save_at: Duration,
    /// 最近一次跨日检查的 now 值 (1Hz 节流基准)。
    last_date_check: Duration,
    /// 完成反馈视觉脉冲 (阶段流转触发)。
    flash: FlashOverlay,
    /// 首次启动快捷键提示 (一过性 fade-in/hold/fade-out 状态机)。
    hint: ShortcutHintOverlay,
    /// 当前持久化的「已见过快捷键提示」旗标 (snapshot_state 直接读)。
    has_seen_shortcut_hint: bool,
    /// 今日计数所属日期 (YYYY-MM-DD, 与 today_count 配对)。
    today_date: String,
    /// 今日已自然完成的专注数 (skip 不计，跨日归零)。
    today_count: u32,
    /// 环境音混音器 (纯逻辑：淡化权重 × 暂停沉降包络)。
    ambient_mixer: ambient::AmbientMixer,
    /// 环境音播放器 (rodio 适配层：懒初始化 + 双槽 + 静默降级)。
    ambient_player: ambient::AmbientPlayer,
    /// 全局环境音开关 (false = 静音所有场景音景)。
    sound_on: bool,
    /// 场景动效沉降包络 (纯逻辑：暂停 500ms 淡出 / 恢复淡入)。
    motion_envelope: motion::MotionEnvelope,
    /// 最近 tick 算出的动效包络值 (`background_frame` 只读)。
    motion_gain: f32,
    /// 雨钟 (秒): 雨丝下落时间轴。暂停时定格可见 (不随包络沉降),
    /// 包络只推进本钟 — 暂停 500ms 减速冻结，恢复 500ms 加速续走。
    rain_clock: f32,
    /// 窗口事件发送器 (run_app 启动时注入，App 借此控制窗口显隐 / 退出)。
    window_sender: Option<WindowEventSender>,
    /// 窗口是否已最大化 (决定标题栏按钮图标 □/□□)。
    is_maximized: bool,
    /// 设置面板是否打开。
    settings_open: bool,
    /// 统计面板是否打开。
    stats_open: bool,
    /// 年度报告面板是否打开。
    report_open: bool,
    /// 专注会话历史 (数据层：自然完成的 Focus 记录)。
    history: FocusHistory,
    /// 历史脏旗标：完成记录后置位，与状态共用 1Hz 节流落盘。
    history_dirty: bool,
    /// 导出 CSV 结果提示 (Some = 显示中)。
    export_notice: Option<String>,
    /// 提示过期时刻 (注入时间轴，now >= 此值后隐藏)。
    export_notice_until: Duration,
    /// 面板关闭后要恢复焦点的按钮 id (一次性; 框架应用后经 [`App::focus_restored`] 清除)。
    restore_focus_to: Option<&'static str>,
    /// 导出 CSV 是否已存在 (启动时查一次，导出成功后置位; 控制「打开所在目录」按钮)。
    export_file_exists: bool,
}

/// 应用消息。
#[derive(Clone, Copy)]
enum Msg {
    /// 开始 / 暂停切换。
    StartPause,
    /// 跳过当前阶段，进入下一阶段。
    Skip,
    /// 上一个场景。
    PrevScene,
    /// 下一个场景。
    NextScene,
    /// 切换窗口可见性 (全局热键 Ctrl+Shift+P)。
    ToggleVisible,
    /// 退出应用 (全局热键 Ctrl+Shift+Q)。
    Quit,
    /// 打开 / 关闭设置面板。
    ToggleSettings,
    /// 专注时长 +1 分钟。
    IncFocus,
    /// 专注时长 −1 分钟。
    DecFocus,
    /// 短休息时长 +1 分钟。
    IncBreak,
    /// 短休息时长 −1 分钟。
    DecBreak,
    /// 长休息时长 +1 分钟。
    IncLongBreak,
    /// 长休息时长 −1 分钟。
    DecLongBreak,
    /// 重置计时配置为默认值 (25/5/15)。
    ResetConfig,
    /// 打开 / 关闭统计面板。
    ToggleStats,
    /// 打开 / 关闭年度报告面板。
    ToggleReport,
    /// 切换全局环境音开/关 (静音所有场景音景)。
    ToggleSound,
    /// 导出专注数据为 CSV (明文，固定路径)。
    ExportCsv,
    /// 打开导出 CSV 所在目录 (已导出过时可用)。
    OpenExportDir,
    /// 打开 GitHub Issues 反馈页面 (预填标题前缀 + 版本 + OS)。
    OpenFeedback,
}

impl PomodoroApp {
    /// 默认会话构造：25:00 Focus Idle, 场景 0, 全部偏移为 0。
    fn new_default() -> Self {
        Self {
            timer: Pomodoro::new(),
            now: Duration::ZERO,
            fader: SceneFader::new(0, FADE_DURATION),
            now_offset: Duration::ZERO,
            state_dirty: true,
            last_save_at: Duration::ZERO,
            last_date_check: Duration::ZERO,
            flash: FlashOverlay::new(FLASH_DURATION),
            // 全新会话：触发一次性快捷键提示，同时标记为已见 (节流落盘后 JSON 持久化)。
            hint: ShortcutHintOverlay::triggered_at(Duration::ZERO),
            has_seen_shortcut_hint: true,
            today_date: today::today_string(),
            today_count: 0,
            ambient_mixer: ambient::AmbientMixer::new(),
            ambient_player: ambient::AmbientPlayer::new(),
            sound_on: true,
            motion_envelope: motion::MotionEnvelope::new(),
            motion_gain: 0.0,
            rain_clock: 0.0,
            window_sender: None,
            is_maximized: false,
            settings_open: false,
            stats_open: false,
            report_open: false,
            history: stats::load_history(),
            history_dirty: false,
            export_notice: None,
            export_notice_until: Duration::ZERO,
            restore_focus_to: None,
            export_file_exists: export_csv_path().map(|p| p.exists()).unwrap_or(false),
        }
    }

    /// 从持久化状态恢复：设置 timer / 场景 / now_offset,
    /// 状态保持 dirty 以确保一次重写。
    fn from_state(state: PomodoroState) -> Self {
        let now_offset = state.effective_now_offset();
        let run: Run = state.run.into();
        let remaining = Duration::from_secs(state.remaining_secs);
        let deadline = if matches!(run, Run::Running) {
            Some(now_offset + remaining)
        } else {
            None
        };
        let saved_config = timer::TimerConfig {
            focus_secs: state.focus_duration_secs,
            break_secs: state.break_duration_secs,
            long_break_secs: state.long_break_duration_secs,
        };
        let timer = Pomodoro::restore(
            state.phase,
            run,
            remaining,
            deadline,
            state.completed_focus,
            saved_config,
        );
        let fader = if state.current_scene < SCENES.len() {
            SceneFader::new(state.current_scene, FADE_DURATION)
        } else {
            SceneFader::new(0, FADE_DURATION)
        };
        // 一次性快捷键提示：没看过就触发一次，触发即标记为已见。
        let should_show_hint = !state.has_seen_shortcut_hint;
        // 今日计数：跨日归零恢复 (空串/过期日期一律归零)。
        let today = today::today_string();
        let today_count = today::resolve_today_count(&state.today_date, state.today_count, &today);
        Self {
            timer,
            now: now_offset,
            fader,
            now_offset,
            state_dirty: true,
            last_save_at: now_offset,
            last_date_check: now_offset,
            flash: FlashOverlay::new(FLASH_DURATION),
            hint: if should_show_hint {
                ShortcutHintOverlay::triggered_at(now_offset)
            } else {
                ShortcutHintOverlay::idle()
            },
            has_seen_shortcut_hint: true,
            today_date: today,
            today_count,
            ambient_mixer: ambient::AmbientMixer::new(),
            ambient_player: ambient::AmbientPlayer::new(),
            sound_on: state.sound_on,
            motion_envelope: motion::MotionEnvelope::new(),
            motion_gain: 0.0,
            rain_clock: 0.0,
            window_sender: None,
            is_maximized: false,
            settings_open: false,
            stats_open: false,
            report_open: false,
            history: stats::load_history(),
            history_dirty: false,
            export_notice: None,
            export_notice_until: Duration::ZERO,
            restore_focus_to: None,
            export_file_exists: export_csv_path().map(|p| p.exists()).unwrap_or(false),
        }
    }

    /// 立即落盘 (退出/异常时调用，不走节流)。
    /// 失败不 panic: 进程即将退出，错误仅供日志，重试窗口已无。
    fn flush(&mut self) {
        match save_state(&self.snapshot_state()) {
            Ok(()) => {
                self.state_dirty = false;
                self.last_save_at = self.now;
            }
            Err(err) => log::warn!("flush 状态失败：{err}"),
        }
        if self.history_dirty {
            match stats::save_history(&self.history) {
                Ok(()) => self.history_dirty = false,
                Err(err) => log::warn!("flush 专注历史失败：{err}"),
            }
        }
    }

    /// 应用当前状态为快照 (供 save_state 调用)。
    fn snapshot_state(&self) -> PomodoroState {
        let config = self.timer.config();
        PomodoroState {
            phase: self.timer.phase(),
            run: RunState::from(self.timer.run()),
            remaining_secs: self.timer.remaining(self.now).as_secs(),
            current_scene: self.fader.current(),
            saved_elapsed_secs: self.now.as_secs(),
            saved_wall_secs: current_wall_secs(),
            has_seen_shortcut_hint: self.has_seen_shortcut_hint,
            completed_focus: self.timer.completed_focus(),
            today_date: self.today_date.clone(),
            today_count: self.today_count,
            focus_duration_secs: config.focus_secs,
            break_duration_secs: config.break_secs,
            long_break_duration_secs: config.long_break_secs,
            sound_on: self.sound_on,
        }
    }

    /// 当前视觉调色板：淡化中为两端调色板的插值 (色调随画面同步流动);
    /// 暂停时整体降饱和 70% (含控件底色与文字色), 视觉上明显区分。
    fn palette(&self) -> ScenePalette {
        let (from, to, t) = self.fader.frame(self.now, |t| FADE_EASING.eval(t));
        let base = SCENES[from].palette.lerp(SCENES[to].palette, t);
        if self.timer.is_running() {
            base
        } else {
            base.desaturate(0.7)
        }
    }

    /// 当前场景主题 (颜色 token 随调色板流动)。
    fn theme(&self) -> SceneTheme {
        SceneTheme::new(self.palette())
    }

    /// 调整指定 phase 的时长 (秒级 delta, 约束到有效范围后写回 timer)。
    fn adjust_config(&mut self, phase: Phase, delta_secs: i64) {
        let mut config = *self.timer.config();
        let target = match phase {
            Phase::Focus => &mut config.focus_secs,
            Phase::Break => &mut config.break_secs,
            Phase::LongBreak => &mut config.long_break_secs,
        };
        // u64 的饱和加减
        if delta_secs >= 0 {
            *target = target.saturating_add(delta_secs as u64);
        } else {
            *target = target.saturating_sub((-delta_secs) as u64);
        }
        self.timer.update_config(config);
    }

    /// 将自然完成的专注写为会话记录 (每完成一条)。
    ///
    /// 专注时长取计划时长 (自然完成 = 计时器从满量跑到 0, 实际专注恒等于计划，
    /// 暂停冻结 remaining 不计入); 开始时刻由「完成时刻 - 计划时长」推得，保证
    /// 记录自洽 (focused ≤ completed - started)。huge overshoot 一次多条时按
    /// 完成时刻倒排错开 (i=0 为批次内最早), 轮次钳到 ≥1 (跨周期边界时无法还原
    /// 上一周期的轮次，不做越界值)。
    fn record_focus_sessions(&mut self, count: u8, round: u8) {
        let completed_ts = current_wall_secs();
        let planned_secs = self.timer.config().focus_secs;
        let scene_index = self.fader.current();
        for i in 0..count {
            let offset = count - 1 - i;
            let completed = completed_ts.saturating_sub(u64::from(offset) * planned_secs);
            self.history.push(SessionRecord {
                started_ts: completed.saturating_sub(planned_secs),
                completed_ts: completed,
                planned_secs,
                focused_secs: planned_secs,
                scene_index,
                round_in_cycle: round.saturating_sub(offset).max(1),
                completed: true,
            });
        }
        self.history_dirty = true;
    }

    /// 执行 CSV 导出并设置面板提示 (用户点击必须有可见反馈，3s 后过期)。
    /// `path = None` 表示无配置目录 (导出失败)。
    /// 执行 CSV 导出并设置面板提示 (用户点击必须有可见反馈，3s 后过期)。
    /// `path = None` 表示无配置目录 (导出失败)。返回是否成功
    /// (供调用方决定是否在文件管理器中显示导出文件)。
    fn run_export_csv(&mut self, path: Option<std::path::PathBuf>) -> bool {
        let (ok, notice) = match path {
            Some(path) => match stats::export_csv_to(&path, &self.history) {
                Ok(()) => {
                    log::info!("专注数据已导出：{}", path.display());
                    self.export_file_exists = true; // 已导出过：显示「打开所在目录」按钮
                    (true, "已导出 CSV ✓".to_string())
                }
                Err(reason) => {
                    log::warn!("导出 CSV 失败：{reason}");
                    (false, format!("导出失败：{reason}"))
                }
            },
            None => (false, "导出失败：无配置目录".to_string()),
        };
        self.export_notice = Some(notice);
        self.export_notice_until = self.now + Duration::from_secs(3);
        ok
    }
}

impl App for PomodoroApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        self.state_dirty = true;
        match msg {
            Msg::StartPause => self.timer.toggle(self.now),
            Msg::Skip => {
                self.timer.skip(self.now);
            }
            Msg::PrevScene => {
                let target = (self.fader.current() + SCENES.len() - 1) % SCENES.len();
                self.fader.switch_to(target, self.now);
            }
            Msg::NextScene => {
                let target = (self.fader.current() + 1) % SCENES.len();
                self.fader.switch_to(target, self.now);
            }
            Msg::ToggleVisible => {
                if let Some(sender) = &self.window_sender {
                    sender.toggle_visible();
                }
            }
            Msg::Quit => {
                if let Some(sender) = &self.window_sender {
                    sender.quit();
                }
            }
            Msg::ToggleSettings => {
                // 关闭设置面板：焦点回到「设置」按钮 (一次性，见 focus_request)。
                if self.settings_open {
                    self.restore_focus_to = Some("settings-button");
                }
                self.stats_open = false;
                self.report_open = false;
                self.settings_open = !self.settings_open;
            }
            Msg::IncFocus => self.adjust_config(Phase::Focus, 60),
            Msg::DecFocus => self.adjust_config(Phase::Focus, -60),
            Msg::IncBreak => self.adjust_config(Phase::Break, 60),
            Msg::DecBreak => self.adjust_config(Phase::Break, -60),
            Msg::IncLongBreak => self.adjust_config(Phase::LongBreak, 60),
            Msg::DecLongBreak => self.adjust_config(Phase::LongBreak, -60),
            Msg::ResetConfig => {
                self.timer.update_config(timer::TimerConfig::default());
            }
            Msg::ToggleStats => {
                // 关闭统计面板：焦点回到「统计」按钮 (一次性，见 focus_request)。
                if self.stats_open {
                    self.restore_focus_to = Some("stats-button");
                }
                self.settings_open = false;
                self.report_open = false;
                self.stats_open = !self.stats_open;
            }
            Msg::ToggleReport => {
                // 关闭报告面板：焦点回到「报告」按钮 (一次性，见 focus_request)。
                if self.report_open {
                    self.restore_focus_to = Some("report-button");
                }
                self.settings_open = false;
                self.stats_open = false;
                self.report_open = !self.report_open;
            }
            Msg::ToggleSound => {
                self.sound_on = !self.sound_on;
            }
            Msg::ExportCsv => {
                let path = export_csv_path();
                let exported = self.run_export_csv(path.clone());
                if exported {
                    // 导出成功：在系统文件管理器中显示文件 (回答「导到哪了」)。
                    if let Some(path) = path {
                        reveal_in_file_manager(&path);
                    }
                }
            }
            Msg::OpenExportDir => {
                // 已导出过的按钮：直接打开导出文件所在目录 (文件若被外部删除则只记日志)。
                if let Some(path) = export_csv_path() {
                    if path.exists() {
                        reveal_in_file_manager(&path);
                    } else {
                        log::warn!("导出文件不存在，跳过打开目录：{}", path.display());
                    }
                }
            }
            Msg::OpenFeedback => open_feedback(),
        }
    }

    fn view(&self) -> Node {
        let t = self.theme();
        widget::node(
            Stack::new()
                .child(
                    MultiPanel::new()
                        .child(content_column(t))
                        .child(
                            // 额外包裹一层 Padding 避免焦点路径
                            // 在主面板和设置面板间碰撞：
                            // 同索引路径会命中不同组件导致
                            // FocusOut 无法送达隐藏面板内的旧焦点。
                            Padding::new(Edges::ZERO, settings_panel(t)),
                        )
                        .child(Padding::new(Edges::ZERO, stats_panel(t)))
                        .child(Padding::new(Edges::ZERO, report_panel(t)))
                        .bind(|s: &PomodoroApp| {
                            if s.report_open {
                                3
                            } else if s.stats_open {
                                2
                            } else if s.settings_open {
                                1
                            } else {
                                0
                            }
                        }),
                )
                .child(flash_overlay_widget())
                .child(shortcut_hint_overlay_widget()),
        )
    }

    fn event(&mut self, event: &Event) {
        if let Event::Key {
            key: Key::Named(NamedKey::Escape),
            pressed: true,
            ..
        } = event
        {
            if self.settings_open {
                self.settings_open = false;
                self.restore_focus_to = Some("settings-button");
                self.state_dirty = true;
            }
            if self.stats_open {
                self.stats_open = false;
                self.restore_focus_to = Some("stats-button");
                self.state_dirty = true;
            }
            if self.report_open {
                self.report_open = false;
                self.restore_focus_to = Some("report-button");
                self.state_dirty = true;
            }
        }
    }

    fn tick(&mut self, ctx: &AnimationCtx) {
        let dt = ctx.elapsed.saturating_sub(self.now);
        self.now = ctx.elapsed;
        let report = self.timer.tick(ctx.elapsed);
        if report.advanced {
            // 阶段流转触发视觉脉冲 + 系统提示音
            self.flash.trigger(self.now);
            audio::beep();
            // 通知 Handler: 阶段流转 (用于隐藏态时自动呼出窗口)
            if let Some(sender) = &self.window_sender {
                sender.phase_advanced();
            }
        }
        // 今日计数 + 会话记录：自然完成的专注才计 (skip 不产生 focus_completions);
        // 跨日先归零再累加，并标脏触发 1Hz 节流持久化。
        if report.focus_completions > 0 {
            let today = today::today_string();
            self.today_count =
                today::resolve_today_count(&self.today_date, self.today_count, &today)
                    + u32::from(report.focus_completions);
            self.today_date = today;
            self.state_dirty = true;
            // 会话记录：每个自然完成记一条。专注时长取计划时长——自然完成意味着
            // 计时器从满量跑到 0 (暂停冻结 remaining), 实际专注恒等于计划时长;
            // 故无需逐帧累计 (dt 累加在 huge overshoot 下会把冻结/休息期摊进专注,
            // 恢复中途的 started_ts 也不真实)。开始时刻由「完成 - 计划」推得，记录自洽。
            self.record_focus_sessions(report.focus_completions, report.completed_round);
        }
        // 跨日归零 (1Hz 节流): 常驻应用过午夜后，不等下次完成即刷新副标「今日 N」。
        if self.now.saturating_sub(self.last_date_check) >= SAVE_THROTTLE {
            self.last_date_check = self.now;
            let today = today::today_string();
            if today != self.today_date {
                self.today_date = today;
                self.today_count = 0;
                self.state_dirty = true;
            }
        }
        // 1Hz 节流落盘：状态或会话历史变更后，距上次保存 ≥ 1s 才写。
        if (self.state_dirty || self.history_dirty)
            && self.now.saturating_sub(self.last_save_at) >= SAVE_THROTTLE
        {
            let mut all_ok = true;
            if self.state_dirty {
                if let Err(err) = save_state(&self.snapshot_state()) {
                    log::warn!("保存状态失败：{err}");
                    all_ok = false;
                }
            }
            if self.history_dirty {
                if let Err(err) = stats::save_history(&self.history) {
                    log::warn!("保存专注历史失败：{err}");
                    all_ok = false;
                }
            }
            if all_ok {
                self.state_dirty = false;
                self.history_dirty = false;
                self.last_save_at = self.now;
            } else {
                // 失败保留 dirty, 下次到达节流间隔时重试; 同时更新 last_save_at 避免 60fps 重复刷写。
                self.last_save_at = self.now;
            }
        }
        // 环境音：与视觉淡化同源 (from/to/fade), 300ms 增益包络;
        // 休息期 duck 沉降 (世界退远一步), 懒初始化 + 静默降级。
        let (from, to, fade) = self.fader.frame(self.now, |t| FADE_EASING.eval(t));
        let duck = match self.timer.phase() {
            Phase::Focus => 1.0,
            Phase::Break | Phase::LongBreak => ambient::BREAK_DUCK,
        };
        self.ambient_mixer.set_enabled(self.sound_on);
        let frame = self.ambient_mixer.frame_volumes(
            from,
            to,
            fade,
            self.timer.is_running(),
            duck,
            self.now,
        );
        self.ambient_player.apply(frame);
        // 场景动效：与音频同潮汐契约 — 运行全量，暂停 500ms 沉降 (视觉独立时长)。
        self.motion_gain = self.motion_envelope.gain(self.timer.is_running(), self.now);
        // 雨钟：雨丝定格可见 (2026-07-29 用户裁定：暂停显示雨丝，不随包络沉降);
        // 包络只推进下落时间 — 暂停 500ms 减速冻结，恢复 500ms 加速续走，无跳变。
        self.rain_clock += dt.as_secs_f32() * self.motion_gain;
    }

    fn background_frame(&self) -> Option<BackgroundFrame> {
        let (from, to, fade) = self.fader.frame(self.now, |t| FADE_EASING.eval(t));
        let rain = motion::rain_intensity(from, to, fade);
        let fire = motion::fire_intensity(from, to, fade, self.motion_gain);
        let sea = motion::sea_intensity(from, to, fade, self.motion_gain);
        let mountain = motion::mountain_intensity(from, to, fade, self.motion_gain);
        let forest = motion::forest_intensity(from, to, fade, self.motion_gain);
        let blacksmith = motion::blacksmith_intensity(from, to, fade, self.motion_gain);
        let cave = motion::cave_intensity(from, to, fade, self.motion_gain);
        let nightmarket = motion::nightmarket_intensity(from, to, fade, self.motion_gain);
        let train = motion::train_intensity(from, to, fade, self.motion_gain);
        Some(
            BackgroundFrame::new(from, to, fade, self.palette().base)
                .with_motion(self.now.as_secs_f32(), rain)
                .with_fire(fire)
                .with_sea(sea)
                .with_mountain(mountain)
                .with_forest(forest)
                .with_blacksmith(blacksmith)
                .with_cave(cave)
                .with_nightmarket(nightmarket)
                .with_train(train)
                .with_rain_time(self.rain_clock),
        )
    }

    fn boot_elapsed_offset(&self) -> Duration {
        self.now_offset
    }

    fn attach_window_sender(&mut self, sender: WindowEventSender) {
        self.window_sender = Some(sender);
    }

    fn hotkey(&mut self, id: u8) -> Option<Msg> {
        match id {
            hotkey_ids::TOGGLE_VISIBLE => Some(Msg::ToggleVisible),
            hotkey_ids::START_PAUSE => Some(Msg::StartPause),
            hotkey_ids::QUIT => Some(Msg::Quit),
            _ => None,
        }
    }

    fn tray_action(&mut self, id: u8) -> Option<Msg> {
        match id {
            tray_action_ids::TOGGLE_VISIBLE => Some(Msg::ToggleVisible),
            tray_action_ids::START_PAUSE => Some(Msg::StartPause),
            tray_action_ids::QUIT => Some(Msg::Quit),
            _ => None,
        }
    }

    fn tray_menu(&self) -> danqing::tray_icon::menu::Menu {
        build_menu()
    }

    fn maximized_changed(&mut self, is_maximized: bool) {
        self.is_maximized = is_maximized;
    }

    fn focus_request(&self) -> Option<&'static str> {
        self.restore_focus_to
    }

    fn focus_restored(&mut self) {
        self.restore_focus_to = None;
    }
}

/// 内容列：标题栏 + 中央倒计时 + 底部控件条 (无 flash 叠加，flash 由 Stack 在根上盖)。
fn content_column(t: SceneTheme) -> impl widget::Widget {
    Column::new()
        .cross_stretch()
        .child(
            TitleBar::themed(&t, "丹青 · 番茄钟")
                .logo_kind(LogoKind::Pomodoro)
                .bind_theme(|s: &PomodoroApp| s.theme())
                .bind_maximized(|s: &PomodoroApp| s.is_maximized)
                .on_close(|| WindowAction::Close)
                .on_minimize(|| WindowAction::Minimize)
                .on_maximize(|| WindowAction::MaximizeOrRestore)
                .on_drag(|| WindowAction::Drag),
        )
        .fill(Center::new(countdown_block(t)).fill_max(), 1)
        .child(Padding::all(t.spacing_xl(), Center::new(control_pill(t))))
}

/// 全屏 flash 叠加层：阶段流转时 accent 色脉冲衰减。
/// 未激活时 alpha = 0, 完全透明 (无视觉影响); 激活时由 `progress()` 驱动 alpha。
fn flash_overlay_widget() -> impl widget::Widget {
    UiBox::new(Color::TRANSPARENT).bind_color(|s: &PomodoroApp| {
        let alpha = s.flash.progress(s.now).unwrap_or(0.0);
        let c = s.palette().accent;
        Color::rgba(c.r, c.g, c.b, alpha)
    })
}

/// 首次启动快捷键提示叠加层：窗口右下角三行快捷键说明，由 `hint.progress()` 驱动 alpha。
/// 不激活时完全透明 (无视觉影响); 激活时按 ease-out 淡入 → 停留 → ease-in 淡出。
/// 布局策略：外层 Column 用 fill spacer 把内容推到下方; 内层 Row 用 fill spacer 把内容推到右;
/// 最后 Padding 加 `spacing_lg` 的右/下内边距, 等价于把文本锚定在窗口右下角内缩 16px。
fn shortcut_hint_overlay_widget() -> impl widget::Widget {
    let line_painter = |s: &PomodoroApp| {
        let alpha = s.hint.progress(s.now).unwrap_or(0.0);
        let c = s.palette().text_secondary;
        Color::rgba(c.r, c.g, c.b, c.a * alpha)
    };
    let t = LightTheme;
    let line_a = Text::new(format!(
        "显示/隐藏  {}",
        shortcut_for_id(hotkey_ids::TOGGLE_VISIBLE)
    ))
    .font_size(t.font_size_small())
    .bind_color(line_painter);
    let line_b = Text::new(format!(
        "暂停/开始  {}",
        shortcut_for_id(hotkey_ids::START_PAUSE)
    ))
    .font_size(t.font_size_small())
    .bind_color(line_painter);
    let line_c = Text::new(format!("退出  {}", shortcut_for_id(hotkey_ids::QUIT)))
        .font_size(t.font_size_small())
        .bind_color(line_painter);
    let text_column = Column::new().child(line_a).child(line_b).child(line_c);
    let edge = t.spacing_lg();
    let padded = Padding::new(
        Edges {
            top: 0.0,
            right: edge,
            bottom: edge,
            left: 0.0,
        },
        text_column,
    );
    Column::new().fill(UiBox::new(Color::TRANSPARENT), 1).child(
        Row::new()
            .fill(UiBox::new(Color::TRANSPARENT), 1)
            .child(padded),
    )
}

/// 副标文案 (纯逻辑，可测):
/// - Running + Focus: `专注 · 场景 · 第 N/4 轮` (轮次 = completed_focus + 1);
/// - Running + Break/LongBreak: `休息 · 场景` / `长休息 · 场景` (不带轮次);
/// - 暂停/停止: `⏸ 已暂停 · 场景`;
/// - 今日计数 ≥ 1 时所有形态追加 ` · 今日 N`。
fn subtitle_text(
    running: bool,
    phase: Phase,
    scene_name: &str,
    completed_focus: u8,
    today_count: u32,
) -> String {
    let base = if !running {
        format!("⏸ 已暂停 · {scene_name}")
    } else {
        match phase {
            Phase::Focus => format!(
                "专注 · {scene_name} · 第 {}/{} 轮",
                completed_focus + 1,
                timer::CYCLE_LENGTH
            ),
            Phase::Break | Phase::LongBreak => format!("{} · {scene_name}", phase.label()),
        }
    };
    if today_count >= 1 {
        format!("{base} · 今日 {today_count}")
    } else {
        base
    }
}

/// 中央倒计时块：大字倒计时 + 阶段/场景标注。
/// 暂停时：倒计时切 `text_secondary` + 整体降饱和 + 副标加 "已暂停" 文字。
/// 三重信号确保暂停态视觉明显，用户无需猜测。
fn countdown_block(t: SceneTheme) -> impl widget::Widget {
    Column::new()
        .cross_stretch()
        .child(Center::new(
            Text::bind(|s: &PomodoroApp| s.timer.display(s.now))
                .font_size(t.font_size_display())
                .bind_color(|s: &PomodoroApp| {
                    if s.timer.is_running() {
                        s.palette().text_primary
                    } else {
                        s.palette().text_secondary
                    }
                }),
        ))
        .child(Center::new(
            Text::bind(|s: &PomodoroApp| {
                let scene_name = SCENES[s.fader.current()].name;
                subtitle_text(
                    s.timer.is_running(),
                    s.timer.phase(),
                    scene_name,
                    s.timer.completed_focus(),
                    s.today_count,
                )
            })
            .font_size(t.font_size_body())
            .bind_color(|s: &PomodoroApp| s.palette().text_secondary),
        ))
}

/// 主操作按钮 (开始/暂停): accent 底 + 场景基调色文字 (同场景色对，对比天然成立)。
fn primary_button(t: SceneTheme) -> Button {
    Button::themed(
        &t,
        Text::bind(|s: &PomodoroApp| {
            if s.timer.is_running() {
                "暂停".into()
            } else {
                "开始".into()
            }
        })
        .bind_color(|s: &PomodoroApp| s.palette().base),
    )
    .bind_color(|s: &PomodoroApp| s.palette().accent)
    .on_click(|| Msg::StartPause)
}

/// 幽灵按钮 (重置/场景切换): 透明底，悬停浮现玻璃，文字随场景。
fn ghost_button(t: SceneTheme, label: &'static str, msg: Msg) -> Button {
    Button::themed(
        &t,
        Text::new(label).bind_color(|s: &PomodoroApp| s.palette().text_primary),
    )
    .bind_color(|_: &PomodoroApp| Color::TRANSPARENT)
    .bind_hover_color(|s: &PomodoroApp| s.palette().surface)
    .bind_focus_color(|s: &PomodoroApp| s.palette().accent)
    .on_click(move || msg)
}

/// 底部玻璃胶囊控件条。
fn control_pill(t: SceneTheme) -> impl widget::Widget {
    UiBox::new(Color::TRANSPARENT)
        .bind_color(|s: &PomodoroApp| s.palette().surface)
        .radius(t.radius_xl())
        .child(Padding::new(
            Edges::symmetric(t.spacing_sm(), t.spacing_xs()),
            Row::new()
                .gap(t.spacing_xs())
                .child(ghost_button(t, "前", Msg::PrevScene))
                .child(primary_button(t))
                .child(ghost_button(t, "跳", Msg::Skip))
                .child(ghost_button(t, "后", Msg::NextScene))
                // 面板关闭后焦点回锚点按钮 (按稳定 id, 见 focus_request)。
                .child(ghost_button(t, "统计", Msg::ToggleStats).id("stats-button"))
                .child(ghost_button(t, "报告", Msg::ToggleReport).id("report-button"))
                .child(ghost_button(t, "设置", Msg::ToggleSettings).id("settings-button")),
        ))
}

/// 全局环境音开关按钮：文字随状态 (开/关), 颜色同步 (开 = accent 活动态，关 = 次级色弱化)。
fn sound_toggle_button(t: SceneTheme) -> Button {
    Button::themed(
        &t,
        Text::bind(|s: &PomodoroApp| {
            if s.sound_on {
                "开".to_string()
            } else {
                "关".to_string()
            }
        })
        .bind_color(|s: &PomodoroApp| {
            if s.sound_on {
                s.palette().accent
            } else {
                s.palette().text_secondary
            }
        }),
    )
    .bind_color(|_: &PomodoroApp| Color::TRANSPARENT)
    .bind_hover_color(|s: &PomodoroApp| s.palette().surface)
    .bind_focus_color(|s: &PomodoroApp| s.palette().accent)
    .on_click(|| Msg::ToggleSound)
}

/// 设置面板行：全局环境音开关 (标签 + 状态按钮，与步进行同款对齐)。
fn sound_setting_row(t: SceneTheme) -> impl widget::Widget {
    Row::new()
        .cross_stretch()
        .gap(t.spacing_xs())
        .child(Center::new(
            Text::new("环境音")
                .font_size(t.font_size_body())
                .bind_color(|s: &PomodoroApp| s.palette().text_secondary),
        ))
        // 与步进行的 [-] 占位对齐 (显式 height: 空 UiBox 撑满窗体会压扁行，见 stepper_row 注释)。
        .child(
            UiBox::new(Color::TRANSPARENT)
                .width(STEPPER_MINUS_OFFSET)
                .height(1.0),
        )
        .child(Center::new(sound_toggle_button(t)))
}

/// 设置面板浮层：居中玻璃卡片，调整专注/短休/长休时长。
fn settings_panel(t: SceneTheme) -> impl widget::Widget {
    // 半透明遮罩 + 居中玻璃卡片
    Stack::new().child(UiBox::new(t.scrim()).radius(0.0)).child(
        Center::new(
            UiBox::new(Color::TRANSPARENT)
                .bind_color(|s: &PomodoroApp| s.palette().surface)
                .radius(t.radius_lg())
                .width(SETTINGS_CARD_WIDTH)
                .child(Padding::new(
                    Edges::all(t.spacing_xl()),
                    Column::new()
                        .gap(t.spacing_lg())
                        .child(settings_header(t))
                        .child(stepper_row(
                            t,
                            "专注时长",
                            |s: &PomodoroApp| s.timer.config().focus_secs / 60,
                            Msg::DecFocus,
                            Msg::IncFocus,
                        ))
                        .child(stepper_row(
                            t,
                            "\u{3000}短休息",
                            |s: &PomodoroApp| s.timer.config().break_secs / 60,
                            Msg::DecBreak,
                            Msg::IncBreak,
                        ))
                        .child(stepper_row(
                            t,
                            "\u{3000}长休息",
                            |s: &PomodoroApp| s.timer.config().long_break_secs / 60,
                            Msg::DecLongBreak,
                            Msg::IncLongBreak,
                        ))
                        .child(sound_setting_row(t))
                        .child(ghost_button(t, "重置计时", Msg::ResetConfig))
                        .child(ghost_button(t, "问题反馈", Msg::OpenFeedback))
                        .child(
                            Text::new("变更在下一阶段生效")
                                .font_size(t.font_size_small())
                                .bind_color(|s: &PomodoroApp| s.palette().text_secondary),
                        ),
                )),
        )
        .fill_max(),
    )
}

/// 设置面板标题行："计时设置" + 固定间距 + 关闭按钮。
fn settings_header(t: SceneTheme) -> impl widget::Widget {
    Row::new()
        .cross_stretch()
        .child(Center::new(
            Text::new("计时设置")
                .font_size(t.font_size_heading())
                .bind_color(|s: &PomodoroApp| s.palette().text_primary),
        ))
        .child(
            UiBox::new(Color::TRANSPARENT)
                .width(SETTINGS_HEADER_GAP)
                .height(1.0),
        )
        .child(
            CloseButton::new()
                .on_click(|| Msg::ToggleSettings)
                .bind_color(|s: &PomodoroApp| s.palette().text_primary)
                .bind_hover_color(|s: &PomodoroApp| s.palette().accent),
        )
}

/// 单行步进控件：标签 + [-] + 数值 + [+]
fn stepper_row(
    t: SceneTheme,
    label: &'static str,
    value_fn: fn(&PomodoroApp) -> u64,
    dec_msg: Msg,
    inc_msg: Msg,
) -> impl widget::Widget {
    Row::new()
        .cross_stretch()
        .gap(t.spacing_xs())
        .child(Center::new(
            Text::new(label)
                .font_size(t.font_size_body())
                .bind_color(|s: &PomodoroApp| s.palette().text_secondary),
        ))
        // 减号右移 STEPPER_MINUS_OFFSET: [-] 不贴标签，与 [+][数值] 保持呼吸。
        // 显式 height(1.0): 无子组件的 UiBox 未指定高度会取父约束上限 (Box::layout),
        // 撑满窗体把步进行顶到窗体高 (回归：设置卡片被撑到窗高只显示首行)。
        .child(
            UiBox::new(Color::TRANSPARENT)
                .width(STEPPER_MINUS_OFFSET)
                .height(1.0),
        )
        .child(Center::new(ghost_button(t, "-", dec_msg)))
        .child(
            UiBox::new(Color::TRANSPARENT)
                .width(STEPPER_VALUE_WIDTH)
                .child(Center::new(
                    Text::bind(move |s: &PomodoroApp| format!("{} 分钟", value_fn(s)))
                        .font_size(t.font_size_body())
                        .bind_color(|s: &PomodoroApp| s.palette().text_primary),
                )),
        )
        .child(Center::new(ghost_button(t, "+", inc_msg)))
}

/// 统计面板浮层：居中玻璃卡片，展示 今日 / 本周 / 累计 专注 + 导出按钮。
fn stats_panel(t: SceneTheme) -> impl widget::Widget {
    Stack::new().child(UiBox::new(t.scrim()).radius(0.0)).child(
        Center::new(
            UiBox::new(Color::TRANSPARENT)
                .bind_color(|s: &PomodoroApp| s.palette().surface)
                .radius(t.radius_lg())
                .width(SETTINGS_CARD_WIDTH)
                .child(Padding::new(
                    Edges::all(t.spacing_xl()),
                    Column::new()
                        .gap(t.spacing_lg())
                        .child(stats_header(t))
                        .child(stat_row(t, "今日", |s| format!("{} 次", s.today_count)))
                        .child(stat_row(t, "近 7 天", |s| {
                            let (count, secs) = s.history.week_stats(current_wall_secs());
                            format!("{count} 次 · {}", format_duration(secs))
                        }))
                        .child(stat_row(t, "累计", |s| {
                            let (count, secs) = s.history.total_stats();
                            format!("{count} 次 · {}", format_duration(secs))
                        }))
                        .child(export_actions(t))
                        .child(export_notice_row(t)),
                )),
        )
        .fill_max(),
    )
}

/// 统计面板导出操作区：「导出 CSV」按钮 + (已导出过时)「打开所在目录」按钮。
/// 用 MultiPanel 按 `export_file_exists` 切换：未导出过只显示导出按钮，
/// 已导出过并排显示两个 (导出 + 打开所在目录), 面板高度恒定。
fn export_actions(t: SceneTheme) -> impl widget::Widget {
    MultiPanel::new()
        .child(ghost_button(t, "导出 CSV", Msg::ExportCsv))
        .child(
            Row::new()
                .gap(t.spacing_xs())
                .child(ghost_button(t, "导出 CSV", Msg::ExportCsv))
                .child(ghost_button(t, "打开所在目录", Msg::OpenExportDir)),
        )
        .bind(|s: &PomodoroApp| usize::from(s.export_file_exists))
}

/// 统计面板底部导出提示行：固定高度，无提示时留空 (面板高度恒定，不抖动)。
/// 点击「导出 CSV」后短暂显示结果 (成功 ✓ / 失败原因), 3s 后淡出。
fn export_notice_row(t: SceneTheme) -> impl widget::Widget {
    UiBox::new(Color::TRANSPARENT)
        .height(24.0)
        .child(Center::new(
            Text::bind(|s: &PomodoroApp| match &s.export_notice {
                Some(notice) if s.now < s.export_notice_until => notice.clone(),
                _ => String::new(),
            })
            .font_size(t.font_size_small())
            .bind_color(|s: &PomodoroApp| s.palette().accent),
        ))
}

/// 统计面板标题行："专注统计" + 固定间距 + 关闭按钮。
fn stats_header(t: SceneTheme) -> impl widget::Widget {
    Row::new()
        .cross_stretch()
        .child(Center::new(
            Text::new("专注统计")
                .font_size(t.font_size_heading())
                .bind_color(|s: &PomodoroApp| s.palette().text_primary),
        ))
        .child(
            UiBox::new(Color::TRANSPARENT)
                .width(SETTINGS_HEADER_GAP)
                .height(1.0),
        )
        .child(
            CloseButton::new()
                .on_click(|| Msg::ToggleStats)
                .bind_color(|s: &PomodoroApp| s.palette().text_primary)
                .bind_hover_color(|s: &PomodoroApp| s.palette().accent),
        )
}

/// 单行统计：标签靠左 + 值靠右。
fn stat_row(
    t: SceneTheme,
    label: &'static str,
    value_fn: impl Fn(&PomodoroApp) -> String + 'static,
) -> impl widget::Widget {
    Row::new()
        .cross_stretch()
        .child(Center::new(
            Text::new(label)
                .font_size(t.font_size_body())
                .bind_color(|s: &PomodoroApp| s.palette().text_secondary),
        ))
        .fill(UiBox::new(Color::TRANSPARENT), 1)
        .child(Center::new(
            Text::bind(move |s: &PomodoroApp| value_fn(s))
                .font_size(t.font_size_body())
                .bind_color(|s: &PomodoroApp| s.palette().text_primary),
        ))
}

/// 秒数 → 人读时长 ("X 小时 Y 分" / "Y 分钟" / "Z 秒")。
fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h} 小时 {m} 分")
    } else if m > 0 {
        format!("{m} 分钟")
    } else {
        format!("{secs} 秒")
    }
}

/// 导出 CSV 的固定路径 (OS 配置目录 + danqing/focus-history.csv)。
fn export_csv_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("danqing").join("focus-history.csv"))
}

/// 在系统文件管理器中显示导出文件 (回答「导到哪了」)。
/// Win: Explorer 定位文件; mac: Finder 定位; 其它平台：打开所在目录。
/// 导出本身已成功，此处失败只记日志，不影响导出结果。
fn reveal_in_file_manager(path: &std::path::Path) {
    if let Err(err) = reveal_attempt(path) {
        log::warn!("在文件管理器中显示导出文件失败：{err}");
    }
}

#[cfg(target_os = "windows")]
fn reveal_attempt(path: &std::path::Path) -> std::io::Result<std::process::Child> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()
}

#[cfg(target_os = "macos")]
fn reveal_attempt(path: &std::path::Path) -> std::io::Result<std::process::Child> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn reveal_attempt(path: &std::path::Path) -> std::io::Result<std::process::Child> {
    let Some(dir) = path.parent() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "导出文件无父目录",
        ));
    };
    std::process::Command::new("xdg-open").arg(dir).spawn()
}

/// 打开 GitHub Issues 反馈页面：预填标题前缀 + 应用版本 + 操作系统信息。
/// 用户补充内容后直接提交，无需手动填写环境信息。
fn open_feedback() {
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let title = urlencoding::encode("[Bug] ");
    let body_raw = format!(
        "**应用版本:** {version}\n**操作系统:** {os} ({arch})\n\n**问题描述:**\n\n\n**复现步骤:**\n1. \n2. \n3. "
    );
    let body = urlencoding::encode(&body_raw);
    let url = format!(
        "https://github.com/14uncle/danqing-pomodoro/issues/new?title={title}&labels=bug&body={body}"
    );
    if let Err(err) = open::that(&url) {
        log::warn!("打开反馈页面失败：{err}");
    }
}

/// 当前本地年份 (年度报告按年聚合的锚)。
fn current_year() -> u32 {
    chrono::Local::now().year() as u32
}

/// 年度报告面板浮层：居中玻璃卡片，深度洞察
/// (当前年汇总 + 场景分布 + 近 12 月趋势)。
fn report_panel(t: SceneTheme) -> impl widget::Widget {
    Stack::new().child(UiBox::new(t.scrim()).radius(0.0)).child(
        Center::new(
            UiBox::new(Color::TRANSPARENT)
                .bind_color(|s: &PomodoroApp| s.palette().surface)
                .radius(t.radius_lg())
                .width(REPORT_CARD_WIDTH)
                .child(Padding::new(
                    Edges::all(t.spacing_xl()),
                    Column::new()
                        .gap(t.spacing_lg())
                        .child(report_header(t))
                        .child(section_label(t, "本年"))
                        .child(stat_row(t, "专注时长", |s| {
                            format_duration(s.history.year_summary(current_year()).total_secs)
                        }))
                        .child(stat_row(t, "轮次", |s| {
                            format!(
                                "{} 次",
                                s.history.year_summary(current_year()).session_count
                            )
                        }))
                        .child(stat_row(t, "活跃天数", |s| {
                            format!("{} 天", s.history.year_summary(current_year()).active_days)
                        }))
                        .child(section_label(t, "场景分布"))
                        .child(scene_distribution_rows(t))
                        .child(section_label(t, "近 12 月趋势"))
                        .child(month_trend_rows(t)),
                )),
        )
        .fill_max(),
    )
}

/// 报告面板分区标题。
fn section_label(t: SceneTheme, text: &'static str) -> impl widget::Widget {
    Text::new(text)
        .font_size(t.font_size_small())
        .bind_color(|s: &PomodoroApp| s.palette().text_secondary)
}

/// 报告面板标题行："年度报告" + 关闭按钮。
fn report_header(t: SceneTheme) -> impl widget::Widget {
    Row::new()
        .cross_stretch()
        .child(Center::new(
            Row::new().gap(t.spacing_sm()).child(
                Text::new("年度报告")
                    .font_size(t.font_size_heading())
                    .bind_color(|s: &PomodoroApp| s.palette().text_primary),
            ),
        ))
        .child(
            UiBox::new(Color::TRANSPARENT)
                .width(SETTINGS_HEADER_GAP)
                .height(1.0),
        )
        .child(
            CloseButton::new()
                .on_click(|| Msg::ToggleReport)
                .bind_color(|s: &PomodoroApp| s.palette().text_primary)
                .bind_hover_color(|s: &PomodoroApp| s.palette().accent),
        )
}

/// 场景分布：每个场景一行 (名 + 本年专注时长; 无记录显示 "—")。
fn scene_distribution_rows(t: SceneTheme) -> impl widget::Widget {
    (0..SCENES.len()).fold(Column::new().gap(t.spacing_xs()), |col, idx| {
        col.child(scene_row(t, idx))
    })
}

fn scene_row(t: SceneTheme, idx: usize) -> impl widget::Widget {
    stat_row(t, SCENES[idx].name, move |s| {
        let secs = s
            .history
            .year_summary(current_year())
            .scene_secs
            .get(idx)
            .copied()
            .unwrap_or(0);
        if secs > 0 {
            format_duration(secs)
        } else {
            "—".to_string()
        }
    })
}

/// 近 12 月趋势：逐月一行 (YYYY-MM + 当月专注时长)。
fn month_trend_rows(t: SceneTheme) -> impl widget::Widget {
    (0..12).fold(Column::new().gap(t.spacing_xs()), |col, idx| {
        col.child(trend_row(t, idx))
    })
}

fn trend_row(t: SceneTheme, idx: usize) -> impl widget::Widget {
    Row::new()
        .cross_stretch()
        .child(Center::new(
            Text::bind(move |s: &PomodoroApp| {
                let trend = s.history.month_trend(current_wall_secs(), 12);
                let (y, m, _) = trend[idx];
                format!("{y}-{m:02}")
            })
            .font_size(t.font_size_small())
            .bind_color(|s: &PomodoroApp| s.palette().text_secondary),
        ))
        .fill(UiBox::new(Color::TRANSPARENT), 1)
        .child(Center::new(
            Text::bind(move |s: &PomodoroApp| {
                let trend = s.history.month_trend(current_wall_secs(), 12);
                let (_, _, secs) = trend[idx];
                format_duration(secs)
            })
            .font_size(t.font_size_small())
            .bind_color(|s: &PomodoroApp| s.palette().text_primary),
        ))
}

fn main() -> ExitCode {
    danqing::log::init_log();

    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            log::error!("应用启动失败：{err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    // 优先加载持久化状态; 失败/不存在则新建默认会话。
    let mut app = match load_state() {
        Some(state) => {
            log::info!(
                "从持久化恢复：phase={:?} run={:?} remaining={}s scene={} now_offset={}s",
                state.phase,
                state.run,
                state.remaining_secs,
                state.current_scene,
                state.effective_now_offset().as_secs(),
            );
            PomodoroApp::from_state(state)
        }
        None => PomodoroApp::new_default(),
    };

    let background =
        BackgroundConfig::with_scenes(SCENES.iter().map(|s| s.image)).scale(ScaleMode::Cover);
    let config = WindowConfig {
        title: "丹青 · 番茄钟".into(),
        size: Size::new(960.0, 640.0),
        clear_color: SCENES[0].palette.base,
        background,
        // 常驻型应用：关闭按钮 / Alt+F4 只隐藏窗口，进程由托盘 / 全局热键退出。
        close_behavior: danqing::CloseBehavior::Hide,
        logo_name: "pomodoro".into(),
        // 专注陪伴型工具：启动即全屏沉浸 (场景大图为主角), 最大化契合视觉契约。
        maximized: true,
        // 番茄钟需要持续渲染：隐藏态仍保持 tick 推进 (计时器/音频/场景动效)。
        mode: danqing::WindowMode::Continuous,
        ..WindowConfig::default()
    };
    danqing::run_app(config, &mut app)?;
    // 退出 flush: 立即落盘一次，不走节流。
    app.flush();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_running_focus_shows_round() {
        assert_eq!(
            subtitle_text(true, Phase::Focus, "篝火", 1, 0),
            "专注 · 篝火 · 第 2/4 轮"
        );
        assert_eq!(
            subtitle_text(true, Phase::Focus, "海", 0, 0),
            "专注 · 海 · 第 1/4 轮"
        );
    }

    #[test]
    fn subtitle_running_break_and_long_break_hide_round() {
        assert_eq!(subtitle_text(true, Phase::Break, "海", 2, 0), "休息 · 海");
        assert_eq!(
            subtitle_text(true, Phase::LongBreak, "山", 0, 0),
            "长休息 · 山"
        );
    }

    #[test]
    fn subtitle_paused_keeps_paused_wording() {
        assert_eq!(
            subtitle_text(false, Phase::Focus, "雨", 3, 0),
            "⏸ 已暂停 · 雨"
        );
        assert_eq!(
            subtitle_text(false, Phase::LongBreak, "森林", 0, 0),
            "⏸ 已暂停 · 森林"
        );
    }

    #[test]
    fn subtitle_appends_today_count_when_positive() {
        assert_eq!(
            subtitle_text(true, Phase::Focus, "篝火", 1, 3),
            "专注 · 篝火 · 第 2/4 轮 · 今日 3"
        );
        assert_eq!(
            subtitle_text(true, Phase::Break, "海", 2, 1),
            "休息 · 海 · 今日 1"
        );
        assert_eq!(
            subtitle_text(false, Phase::Focus, "雨", 3, 2),
            "⏸ 已暂停 · 雨 · 今日 2"
        );
    }

    #[test]
    fn focus_completion_bumps_today_count() {
        let mut app = PomodoroApp::new_default();
        assert_eq!(app.today_count, 0);
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_secs(25 * 60));
        app.tick(&ctx);
        assert_eq!(app.today_count, 1);
        assert!(app.state_dirty, "计数变更应标脏以触发持久化");
    }

    #[test]
    fn completion_on_new_day_resets_before_bump() {
        let mut app = PomodoroApp::new_default();
        app.today_date = "2020-01-01".into();
        app.today_count = 7;
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_secs(25 * 60));
        app.tick(&ctx);
        assert_eq!(app.today_count, 1, "跨日应先归零再 +1");
        assert_eq!(app.today_date, today::today_string());
    }

    #[test]
    fn skip_does_not_bump_today_count() {
        let mut app = PomodoroApp::new_default();
        app.timer.toggle(app.now);
        app.update(Msg::Skip);
        assert_eq!(app.today_count, 0);
    }

    #[test]
    fn today_count_survives_state_roundtrip() {
        let mut app = PomodoroApp::new_default();
        app.today_count = 3;
        app.today_date = today::today_string();
        let state = app.snapshot_state();
        let restored = PomodoroApp::from_state(state);
        assert_eq!(restored.today_count, 3);
    }

    #[test]
    fn stale_date_resets_on_restore() {
        let mut app = PomodoroApp::new_default();
        app.today_count = 9;
        app.today_date = "2020-01-01".into();
        let state = app.snapshot_state();
        let restored = PomodoroApp::from_state(state);
        assert_eq!(restored.today_count, 0, "过期日期恢复时应归零");
    }

    #[test]
    fn background_frame_carries_rain_motion_when_running_on_rain_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        app.fader.switch_to(motion::RAIN_SCENE, app.now);
        app.timer.toggle(app.now); // 开始计时
        // 场景淡化 (800ms) 完成后包络才开始走 (首次 tick 边沿), 再走满 500ms。
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.rain_intensity - 1.0).abs() < 1e-6,
            "雨场景运行中雨效应全量：{}",
            frame.rain_intensity
        );
        assert!(
            (frame.time - 1.4).abs() < 1e-6,
            "动效时间应注入：{}",
            frame.time
        );
        assert!(
            frame.rain_time > 0.0,
            "运行中雨钟应推进：{}",
            frame.rain_time
        );
    }

    #[test]
    fn background_frame_rain_freezes_visible_on_pause() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test();
        app.fader.switch_to(motion::RAIN_SCENE, app.now);
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        // 暂停 (2026-07-29 用户裁定): 雨丝定格可见 — 强度不沉降，雨钟 500ms 内减速冻结。
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1650));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.rain_intensity - 1.0).abs() < 1e-6,
            "暂停边沿雨丝应全量可见：{}",
            frame.rain_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1900));
        app.tick(&ctx);
        let frozen = app.background_frame().expect("应有背景帧").rain_time;
        assert!(frozen > 0.0, "雨钟应已推进过：{frozen}");
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.rain_intensity - 1.0).abs() < 1e-6,
            "暂停 500ms 后雨丝仍全量可见：{}",
            frame.rain_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(2400));
        app.tick(&ctx);
        let later = app.background_frame().expect("应有背景帧").rain_time;
        assert!(
            (later - frozen).abs() < 1e-6,
            "暂停后雨钟应冻结：{frozen} -> {later}"
        );
        // 恢复：雨钟从冻结点续走，无跳变 (边沿帧包络为 0, 次帧起升)。
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(2900));
        app.tick(&ctx);
        let edge = app.background_frame().expect("应有背景帧").rain_time;
        assert!(
            (edge - frozen).abs() < 1e-6,
            "恢复边沿帧应连续：{frozen} -> {edge}"
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(3400));
        app.tick(&ctx);
        let resumed = app.background_frame().expect("应有背景帧").rain_time;
        assert!(
            resumed > frozen,
            "恢复后雨钟应从冻结点续走：{frozen} -> {resumed}"
        );
    }

    #[test]
    fn background_frame_rain_stays_zero_on_non_rain_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test();
        app.timer.toggle(app.now); // 运行中，但场景是篝火 (非雨)
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert_eq!(frame.rain_intensity, 0.0, "非雨场景雨效恒 0");
    }

    #[test]
    fn background_frame_carries_fire_motion_when_running_on_bonfire_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        app.fader.switch_to(motion::BONFIRE_SCENE, app.now); // 默认场景即篝火，显式锁定
        app.timer.toggle(app.now); // 开始计时
        // 场景淡化 (800ms) 完成后包络才开始走 (首次 tick 边沿), 再走满 500ms。
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.fire_intensity - 1.0).abs() < 1e-6,
            "篝火场景运行中火效应全量：{}",
            frame.fire_intensity
        );
        assert_eq!(frame.rain_intensity, 0.0, "篝火场景雨效恒 0");
    }

    #[test]
    fn background_frame_fire_settles_on_pause() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test();
        app.fader.switch_to(motion::BONFIRE_SCENE, app.now);
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        // 暂停：边沿帧连续 (仍全量), +250ms 沉降中点 0.5, +500ms 消失。
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1650));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.fire_intensity - 1.0).abs() < 1e-6,
            "暂停边沿帧应连续：{}",
            frame.fire_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1900));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.fire_intensity - 0.5).abs() < 1e-6,
            "暂停沉降中点：{}",
            frame.fire_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(2150));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            frame.fire_intensity.abs() < 1e-6,
            "暂停 500ms 后火效应消失：{}",
            frame.fire_intensity
        );
    }

    #[test]
    fn background_frame_fire_stays_zero_on_non_bonfire_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test();
        app.fader.switch_to(motion::RAIN_SCENE, app.now); // 运行中，但场景是雨 (非篝火)
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert_eq!(frame.fire_intensity, 0.0, "非篝火场景火效恒 0");
    }

    #[test]
    fn background_frame_carries_sea_motion_when_running_on_sea_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        app.fader.switch_to(motion::SEA_SCENE, app.now);
        app.timer.toggle(app.now); // 开始计时
        // 场景淡化 (800ms) 完成后包络才开始走 (首次 tick 边沿), 再走满 500ms。
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.sea_intensity - 1.0).abs() < 1e-6,
            "海场景运行中海效应全量：{}",
            frame.sea_intensity
        );
        assert_eq!(frame.rain_intensity, 0.0, "海场景雨效恒 0");
        assert_eq!(frame.fire_intensity, 0.0, "海场景火效恒 0");
    }

    #[test]
    fn background_frame_sea_settles_on_pause() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test();
        app.fader.switch_to(motion::SEA_SCENE, app.now);
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        // 暂停：边沿帧连续 (仍全量), +250ms 沉降中点 0.5, +500ms 消失。
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1650));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.sea_intensity - 1.0).abs() < 1e-6,
            "暂停边沿帧应连续：{}",
            frame.sea_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1900));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.sea_intensity - 0.5).abs() < 1e-6,
            "暂停沉降中点：{}",
            frame.sea_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(2150));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            frame.sea_intensity.abs() < 1e-6,
            "暂停 500ms 后海效应消失：{}",
            frame.sea_intensity
        );
    }

    #[test]
    fn background_frame_sea_stays_zero_on_non_sea_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test(); // 默认场景即篝火 (非海)
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert_eq!(frame.sea_intensity, 0.0, "非海场景海效恒 0");
    }

    #[test]
    fn background_frame_carries_mountain_motion_when_running_on_mountain_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        app.fader.switch_to(motion::MOUNTAIN_SCENE, app.now);
        app.timer.toggle(app.now); // 开始计时
        // 场景淡化 (800ms) 完成后包络才开始走 (首次 tick 边沿), 再走满 500ms。
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.mountain_intensity - 1.0).abs() < 1e-6,
            "山场景运行中山效应全量：{}",
            frame.mountain_intensity
        );
        assert_eq!(frame.rain_intensity, 0.0, "山场景雨效恒 0");
        assert_eq!(frame.fire_intensity, 0.0, "山场景火效恒 0");
        assert_eq!(frame.sea_intensity, 0.0, "山场景海效恒 0");
        assert_eq!(frame.forest_intensity, 0.0, "山场景森林效恒 0");
    }

    #[test]
    fn background_frame_mountain_settles_on_pause() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test();
        app.fader.switch_to(motion::MOUNTAIN_SCENE, app.now);
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        // 暂停：边沿帧连续 (仍全量), +250ms 沉降中点 0.5, +500ms 消失。
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1650));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.mountain_intensity - 1.0).abs() < 1e-6,
            "暂停边沿帧应连续：{}",
            frame.mountain_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1900));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.mountain_intensity - 0.5).abs() < 1e-6,
            "暂停沉降中点：{}",
            frame.mountain_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(2150));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            frame.mountain_intensity.abs() < 1e-6,
            "暂停 500ms 后山效应消失：{}",
            frame.mountain_intensity
        );
    }

    #[test]
    fn background_frame_mountain_stays_zero_on_non_mountain_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test(); // 默认场景即篝火 (非山)
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert_eq!(frame.mountain_intensity, 0.0, "非山场景山效恒 0");
    }

    #[test]
    fn background_frame_carries_forest_motion_when_running_on_forest_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        app.fader.switch_to(motion::FOREST_SCENE, app.now);
        app.timer.toggle(app.now); // 开始计时
        // 场景淡化 (800ms) 完成后包络才开始走 (首次 tick 边沿), 再走满 500ms。
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.forest_intensity - 1.0).abs() < 1e-6,
            "森林场景运行中森林效应全量：{}",
            frame.forest_intensity
        );
        assert_eq!(frame.rain_intensity, 0.0, "森林场景雨效恒 0");
        assert_eq!(frame.fire_intensity, 0.0, "森林场景火效恒 0");
        assert_eq!(frame.sea_intensity, 0.0, "森林场景海效恒 0");
        assert_eq!(frame.mountain_intensity, 0.0, "森林场景山效恒 0");
    }

    #[test]
    fn background_frame_forest_settles_on_pause() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test();
        app.fader.switch_to(motion::FOREST_SCENE, app.now);
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        // 暂停：边沿帧连续 (仍全量), +250ms 沉降中点 0.5, +500ms 消失。
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1650));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.forest_intensity - 1.0).abs() < 1e-6,
            "暂停边沿帧应连续：{}",
            frame.forest_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1900));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            (frame.forest_intensity - 0.5).abs() < 1e-6,
            "暂停沉降中点：{}",
            frame.forest_intensity
        );
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(2150));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert!(
            frame.forest_intensity.abs() < 1e-6,
            "暂停 500ms 后森林效应消失：{}",
            frame.forest_intensity
        );
    }

    #[test]
    fn background_frame_forest_stays_zero_on_non_forest_scene() {
        let mut app = PomodoroApp::new_default();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test(); // 默认场景即篝火 (非森林)
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(900));
        app.tick(&ctx);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_millis(1400));
        app.tick(&ctx);
        let frame = app.background_frame().expect("应有背景帧");
        assert_eq!(frame.forest_intensity, 0.0, "非森林场景森林效恒 0");
    }

    #[test]
    fn midnight_rollover_resets_today_count_without_completion() {
        // 不等下次自然完成 (评审发现：副标曾会显示昨天的「今日 N」)。
        let mut app = PomodoroApp::new_default();
        app.today_date = "2020-01-01".into();
        app.today_count = 5;
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        // 首次 tick (now=0) 距 last_date_check=0 不足 1s, 不触发检查。
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::ZERO);
        app.tick(&ctx);
        assert_eq!(app.today_count, 5, "1s 节流未到，不应检查");
        // 1s 后：触发跨日归零。
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_secs(1));
        app.tick(&ctx);
        assert_eq!(app.today_count, 0, "跨午夜应主动归零");
        assert_eq!(app.today_date, today::today_string());
        // 同日不再误清：有计数后保持。
        app.today_count = 2;
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_secs(2));
        app.tick(&ctx);
        assert_eq!(app.today_count, 2, "同日不得误清");
    }

    #[test]
    fn toggle_settings_flips_state() {
        let mut app = PomodoroApp::new_default();
        assert!(!app.settings_open);
        app.update(Msg::ToggleSettings);
        assert!(app.settings_open);
        app.update(Msg::ToggleSettings);
        assert!(!app.settings_open);
    }

    #[test]
    fn toggle_sound_flips_state() {
        let mut app = PomodoroApp::new_default();
        assert!(app.sound_on, "环境音默认开");
        app.update(Msg::ToggleSound);
        assert!(!app.sound_on);
        app.update(Msg::ToggleSound);
        assert!(app.sound_on);
    }

    #[test]
    fn snapshot_roundtrips_sound_on() {
        let mut app = PomodoroApp::new_default();
        assert!(app.snapshot_state().sound_on);
        app.update(Msg::ToggleSound);
        assert!(!app.snapshot_state().sound_on, "快照应反映关闭态");
    }

    #[test]
    fn adjust_focus_increases_by_one_minute() {
        let mut app = PomodoroApp::new_default();
        let original = app.timer.config().focus_secs;
        app.update(Msg::IncFocus);
        assert_eq!(app.timer.config().focus_secs, original + 60);
    }

    #[test]
    fn adjust_focus_clamps_to_max() {
        let mut app = PomodoroApp::new_default();
        app.timer.update_config(timer::TimerConfig {
            focus_secs: 10_800,
            break_secs: 300,
            long_break_secs: 900,
        });
        app.update(Msg::IncFocus);
        assert_eq!(app.timer.config().focus_secs, 10_800);
    }

    #[test]
    fn adjust_focus_clamps_to_min() {
        let mut app = PomodoroApp::new_default();
        app.timer.update_config(timer::TimerConfig {
            focus_secs: 60,
            break_secs: 300,
            long_break_secs: 900,
        });
        app.update(Msg::DecFocus);
        assert_eq!(app.timer.config().focus_secs, 60);
    }

    // === 数据层：专注会话记录 (2026-08-01 里程碑 0 Task C) ===

    fn fresh_app_with_empty_history() -> PomodoroApp {
        let mut app = PomodoroApp::new_default();
        app.history = FocusHistory::new();
        app
    }

    /// 以 0.5s 步进从当前 now 继续推进 (模拟逐帧运行，毫秒累加可测)。
    fn advance(app: &mut PomodoroApp, by_secs: u64) {
        let step = Duration::from_millis(500);
        let target = app.now + Duration::from_secs(by_secs);
        let mut elapsed = app.now;
        while elapsed < target {
            elapsed += step;
            let ctx = AnimationCtx::new(std::time::Instant::now(), elapsed);
            app.tick(&ctx);
        }
    }

    #[test]
    fn completion_records_focus_session() {
        let mut app = fresh_app_with_empty_history();
        app.last_save_at = Duration::from_secs(25 * 60); // 防测试触发真实落盘
        app.ambient_player.disable_for_test(); // 防测试触碰音频设备
        app.timer.toggle(app.now);
        advance(&mut app, 25 * 60);
        assert_eq!(app.history.sessions.len(), 1);
        let s = &app.history.sessions[0];
        assert_eq!(s.round_in_cycle, 1);
        assert_eq!(s.planned_secs, 25 * 60);
        assert_eq!(s.scene_index, 0);
        assert!(
            s.focused_secs.abs_diff(25 * 60) <= 2,
            "专注时长≈计划时长：{}",
            s.focused_secs
        );
        assert!(s.started_ts > 0 && s.started_ts <= s.completed_ts);
        assert!(s.completed);
        assert!(app.history_dirty, "完成记录应标脏历史以触发落盘");
    }

    #[test]
    fn completion_records_current_scene_and_round() {
        let mut app = fresh_app_with_empty_history();
        app.last_save_at = Duration::from_secs(25 * 60);
        app.ambient_player.disable_for_test();
        app.fader.switch_to(motion::SEA_SCENE, app.now);
        app.timer.toggle(app.now);
        advance(&mut app, 25 * 60);
        let s = &app.history.sessions[0];
        assert_eq!(s.scene_index, motion::SEA_SCENE);
        assert_eq!(s.round_in_cycle, 1);
    }

    #[test]
    fn fourth_focus_records_round_four_and_enters_long_break() {
        let mut app = fresh_app_with_empty_history();
        app.last_save_at = Duration::from_secs(10 * 3600);
        app.ambient_player.disable_for_test();
        app.timer.toggle(app.now);
        for round in 1..=4u8 {
            let start_len = app.history.sessions.len();
            advance(&mut app, 25 * 60);
            assert_eq!(app.history.sessions.len(), start_len + 1);
            let s = app.history.sessions.last().expect("应有会话");
            assert_eq!(s.round_in_cycle, round, "第 {round} 轮轮次应正确");
            if round < 4 {
                advance(&mut app, 5 * 60);
            }
        }
        assert_eq!(app.timer.phase(), Phase::LongBreak);
        assert_eq!(app.history.sessions.len(), 4);
    }

    #[test]
    fn skip_does_not_record_session() {
        let mut app = fresh_app_with_empty_history();
        app.last_save_at = Duration::from_secs(60 * 60);
        app.ambient_player.disable_for_test();
        app.timer.toggle(app.now);
        app.update(Msg::Skip); // skip 出 Focus: 不算完成，不记录
        advance(&mut app, 60);
        assert!(app.history.sessions.is_empty(), "skip 不应产生会话记录");
    }

    #[test]
    fn paused_focus_excludes_pause_from_focused_secs() {
        let mut app = fresh_app_with_empty_history();
        app.last_save_at = Duration::from_secs(10 * 3600);
        app.ambient_player.disable_for_test();
        app.timer.toggle(app.now);
        advance(&mut app, 10 * 60); // 专注 10 分钟
        app.timer.toggle(app.now); // 暂停
        advance(&mut app, 30 * 60); // 暂停 30 分钟 (不累计)
        app.timer.toggle(app.now); // 恢复
        advance(&mut app, 15 * 60); // 再专注 15 分钟 → 完成 (25min 总额)
        let s = &app.history.sessions[0];
        assert_eq!(
            s.focused_secs,
            25 * 60,
            "专注时长 = 计划时长 (暂停不摊入): {}",
            s.focused_secs
        );
    }

    #[test]
    fn huge_overshoot_records_each_completion_chronologically() {
        // C2 回归：单帧跨 2 个 Focus (F + B + F = 3300s)。每条必须自洽 (专注 = 计划时长),
        // 轮次按时间序 [1, 2], 完成时刻单调，不得把冻结/休息期摊进专注。
        let mut app = fresh_app_with_empty_history();
        app.last_save_at = Duration::from_secs(10 * 3600);
        app.ambient_player.disable_for_test();
        app.timer.toggle(app.now);
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_secs(3300));
        app.tick(&ctx);
        assert_eq!(app.history.sessions.len(), 2);
        assert_eq!(app.today_count, 2, "今日计数应与会话数一致");
        let s0 = &app.history.sessions[0];
        let s1 = &app.history.sessions[1];
        assert_eq!(s0.round_in_cycle, 1, "最早完成应为第 1 轮");
        assert_eq!(s1.round_in_cycle, 2, "最近完成应为第 2 轮");
        assert_eq!(s0.focused_secs, 25 * 60, "专注时长 = 计划时长");
        assert_eq!(s1.focused_secs, 25 * 60);
        assert!(s0.completed_ts <= s1.completed_ts, "完成时刻应单调递增");
        for s in &app.history.sessions {
            assert!(
                s.focused_secs <= s.completed_ts.saturating_sub(s.started_ts),
                "记录必须自洽 (专注 ≤ 完成 - 开始): {}",
                s.focused_secs
            );
        }
    }

    #[test]
    fn completion_without_prior_running_frames_still_records() {
        // I3 回归：无 running+Focus 帧即完成 (如恢复 Paused+Focus 后立即跨终点),
        // 也必须记录 — 记录不依赖会话追踪。
        let mut app = fresh_app_with_empty_history();
        app.last_save_at = Duration::from_secs(10 * 3600);
        app.ambient_player.disable_for_test();
        app.timer = timer::Pomodoro::restore(
            Phase::Focus,
            Run::Paused,
            Duration::from_secs(1),
            None,
            0,
            timer::TimerConfig::default(),
        );
        app.timer.toggle(app.now); // 恢复运行
        let ctx = AnimationCtx::new(std::time::Instant::now(), Duration::from_secs(1));
        app.tick(&ctx);
        assert_eq!(app.history.sessions.len(), 1, "无追踪帧也应记录");
        assert_eq!(app.today_count, 1);
        let s = &app.history.sessions[0];
        assert!(
            s.focused_secs <= s.completed_ts.saturating_sub(s.started_ts),
            "恢复场景记录也须自洽"
        );
    }

    // === 统计面板 (2026-08-01) ===

    #[test]
    fn toggle_stats_flips_state() {
        let mut app = PomodoroApp::new_default();
        assert!(!app.stats_open);
        app.update(Msg::ToggleStats);
        assert!(app.stats_open);
        app.update(Msg::ToggleStats);
        assert!(!app.stats_open);
    }

    #[test]
    fn stats_and_settings_mutually_exclusive() {
        let mut app = PomodoroApp::new_default();
        app.update(Msg::ToggleSettings);
        assert!(app.settings_open);
        app.update(Msg::ToggleStats);
        assert!(app.stats_open);
        assert!(!app.settings_open, "打开统计应关闭设置");
        app.update(Msg::ToggleSettings);
        assert!(app.settings_open);
        assert!(!app.stats_open, "打开设置应关闭统计");
    }

    // === 年度报告面板 (2026-08-01 里程碑 1 Task E) ===

    #[test]
    fn toggle_report_flips_state() {
        let mut app = PomodoroApp::new_default();
        assert!(!app.report_open);
        app.update(Msg::ToggleReport);
        assert!(app.report_open);
        app.update(Msg::ToggleReport);
        assert!(!app.report_open);
    }

    #[test]
    fn report_settings_stats_mutually_exclusive() {
        let mut app = PomodoroApp::new_default();
        app.update(Msg::ToggleReport);
        assert!(app.report_open);
        app.update(Msg::ToggleStats);
        assert!(app.stats_open);
        assert!(!app.report_open, "打开统计应关闭报告");
        app.update(Msg::ToggleReport);
        assert!(app.report_open);
        assert!(!app.stats_open, "打开报告应关闭统计");
        app.update(Msg::ToggleSettings);
        assert!(app.settings_open);
        assert!(!app.report_open, "打开设置应关闭报告");
    }

    #[test]
    fn escape_closes_report() {
        let mut app = PomodoroApp::new_default();
        app.update(Msg::ToggleReport);
        assert!(app.report_open);
        app.event(&Event::Key {
            key: Key::Named(NamedKey::Escape),
            pressed: true,
            shift: false,
            ctrl: false,
            alt: false,
        });
        assert!(!app.report_open, "Esc 应关闭报告面板");
        assert_eq!(
            app.focus_request(),
            Some("report-button"),
            "关闭报告应请求焦点回到「报告」按钮"
        );
    }

    #[test]
    fn format_duration_human_readable() {
        assert_eq!(format_duration(0), "0 秒");
        assert_eq!(format_duration(45), "45 秒");
        assert_eq!(format_duration(60), "1 分钟");
        assert_eq!(format_duration(3661), "1 小时 1 分");
        assert_eq!(format_duration(5400), "1 小时 30 分");
    }

    // === 导出 CSV 反馈 (2026-08-01) ===

    #[test]
    fn export_csv_success_sets_visible_notice() {
        let mut app = fresh_app_with_empty_history();
        app.history.push(SessionRecord {
            started_ts: 100,
            completed_ts: 1600,
            planned_secs: 1500,
            focused_secs: 1500,
            scene_index: 0,
            round_in_cycle: 1,
            completed: true,
        });
        app.now = Duration::from_secs(100);
        let dir = std::env::temp_dir().join("danqing-test-export-notice");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.csv");
        assert!(
            app.run_export_csv(Some(path.clone())),
            "成功导出应返回 true"
        );
        assert!(
            app.export_notice
                .as_deref()
                .is_some_and(|n| n == "已导出 CSV ✓"),
            "成功导出应设置可见提示：{:?}",
            app.export_notice
        );
        assert_eq!(
            app.export_notice_until,
            Duration::from_secs(103),
            "3s 后过期"
        );
        assert!(path.exists(), "导出文件应已写入");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_csv_failure_sets_notice_with_reason() {
        let mut app = fresh_app_with_empty_history();
        app.history.refuse_overwrite = true; // 受保护历史 → 拒绝导出
        app.now = Duration::from_secs(5);
        assert!(!app.run_export_csv(None), "失败导出应返回 false");
        assert!(
            app.export_notice
                .as_deref()
                .is_some_and(|n| n.starts_with("导出失败")),
            "失败应设置含原因的提示：{:?}",
            app.export_notice
        );
    }

    // === 设置面板布局回归 (2026-08-01) ===

    /// 测试用主题 (任意调色板，只验证布局几何)。
    fn test_theme() -> SceneTheme {
        SceneTheme::new(ScenePalette {
            base: Color::BLACK,
            accent: Color::WHITE,
            text_primary: Color::WHITE,
            text_secondary: Color::rgb(0.7, 0.7, 0.7),
            surface: Color::rgba(1.0, 1.0, 1.0, 0.1),
            surface_input: Color::rgba(1.0, 1.0, 1.0, 0.2),
            backdrop_light: Color::WHITE,
            backdrop_dark: Color::BLACK,
        })
    }

    // === 面板关闭后焦点回归 (2026-08-01) ===

    #[test]
    fn closing_stats_panel_requests_focus_restore_to_anchor() {
        let mut app = fresh_app_with_empty_history();
        app.update(Msg::ToggleStats); // 打开
        assert!(app.stats_open);
        assert!(app.focus_request().is_none(), "打开面板时不应请求恢复焦点");
        app.update(Msg::ToggleStats); // 关闭
        assert!(!app.stats_open);
        assert_eq!(
            app.focus_request(),
            Some("stats-button"),
            "关闭统计后面板焦点应请求回到统计按钮"
        );
        app.focus_restored();
        assert!(app.focus_request().is_none(), "恢复应用后应清除一次性请求");
    }

    #[test]
    fn closing_settings_panel_requests_focus_restore_to_anchor() {
        let mut app = fresh_app_with_empty_history();
        app.update(Msg::ToggleSettings);
        app.update(Msg::ToggleSettings); // 关闭
        assert_eq!(app.focus_request(), Some("settings-button"));
    }

    #[test]
    fn escape_close_requests_focus_restore() {
        // Escape 关闭路径同样应请求焦点回归 (焦点为空时由应用层关闭面板)。
        let mut app = fresh_app_with_empty_history();
        app.update(Msg::ToggleStats);
        app.event(&Event::Key {
            key: Key::Named(NamedKey::Escape),
            pressed: true,
            shift: false,
            ctrl: false,
            alt: false,
        });
        assert!(!app.stats_open, "Escape 应关闭统计面板");
        assert_eq!(app.focus_request(), Some("stats-button"));
    }

    #[test]
    fn stepper_row_height_tracks_content_not_window() {
        // 回归：减号前的 20px 占位 UiBox 若无显式高度，Box::layout 对未指定
        // 维度取父约束上限 → 步进行被顶到窗体高，设置卡片被撑到窗高只显示首行。
        let t = test_theme();
        let mut row = danqing::widget::node(stepper_row(
            t,
            "专注时长",
            |_: &PomodoroApp| 25,
            Msg::DecFocus,
            Msg::IncFocus,
        ));
        let mut texts = danqing::TextBatch::new();
        let size = row.layout(
            danqing::Constraints::loose(Size::new(960.0, 640.0)),
            &mut texts,
        );
        assert!(
            size.height < 100.0,
            "步进行高度应随内容 (控件高), 而非窗体高：{}",
            size.height
        );
    }

    #[test]
    fn scene_distribution_rows_cover_all_scenes() {
        // 回归：报告面板场景分布曾硬编码 5 行，星夜 (index 5) 被漏。
        // 行数必须与 SCENES 对齐，新增场景时自动跟随。
        let t = test_theme();
        let node = danqing::widget::node(scene_distribution_rows(t));
        assert_eq!(
            node.children().len(),
            SCENES.len(),
            "报告面板场景分布行数应与 SCENES 对齐 (当前 {} 场景)",
            SCENES.len()
        );
    }
}
