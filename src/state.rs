//! @author 十四叔
//! @date 2026/07/25

//! 番茄钟状态持久化: 运行态 + 场景 + 计时快照 + 时间轴基准。
//!
//! JSON 写到 OS 配置目录 (`%APPDATA%/danqing/pomodoro.json` on Windows),
//! 启动时优先加载, 失败回退默认 25:00 Idle。Running 状态按 wall-clock
//! 偏移恢复 deadline, 允许跨重启不丢时间。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::timer::{DEFAULT_BREAK_SECS, DEFAULT_FOCUS_SECS, DEFAULT_LONG_BREAK_SECS, Phase, Run};

/// 持有运行态的枚举镜像 (跨进程序列化)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    /// 静止 (未开始或被重置)。
    Idle,
    /// 计时中。
    Running,
    /// 暂停 (剩余时间已快照)。
    Paused,
}

impl From<Run> for RunState {
    fn from(r: Run) -> Self {
        match r {
            Run::Idle => Self::Idle,
            Run::Running => Self::Running,
            Run::Paused => Self::Paused,
        }
    }
}

impl From<RunState> for Run {
    fn from(s: RunState) -> Self {
        match s {
            RunState::Idle => Self::Idle,
            RunState::Running => Self::Running,
            RunState::Paused => Self::Paused,
        }
    }
}

/// 番茄钟持久化快照 (重启恢复的最小集)。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PomodoroState {
    /// 当前阶段。
    pub phase: Phase,
    /// 当前运行态。
    pub run: RunState,
    /// 剩余秒数 (向下取整, 1s 误差可接受)。
    pub remaining_secs: u64,
    /// 当前场景索引。
    pub current_scene: usize,
    /// 保存时刻的 elapsed 时间 (注入时间轴基准)。
    pub saved_elapsed_secs: u64,
    /// 保存时刻的 wall-clock Unix 秒。
    pub saved_wall_secs: u64,
    /// 用户是否已看过首次启动的快捷键提示 (一过性, 看过即不再显示)。
    /// `serde(default)` 保证旧版 JSON 缺此字段时反序列化为 `false`, 触发一次性提示。
    #[serde(default)]
    pub has_seen_shortcut_hint: bool,
    /// 当前大循环内已自然完成的专注数 (0..4)。
    /// `serde(default)` 保证旧版 JSON 缺此字段时反序列化为 `0`。
    #[serde(default)]
    pub completed_focus: u8,
    /// 今日计数所属日期 (YYYY-MM-DD); 空串表示未记录 (旧版 JSON / 首次启动)。
    #[serde(default)]
    pub today_date: String,
    /// 今日已自然完成的专注数 (跨日归零由 `today::resolve_today_count` 判定)。
    #[serde(default)]
    pub today_count: u32,
    /// 专注时长（秒）。缺省 25 分钟，向后兼容旧版 JSON。
    #[serde(default = "default_focus_secs")]
    pub focus_duration_secs: u64,
    /// 短休息时长（秒）。缺省 5 分钟，向后兼容旧版 JSON。
    #[serde(default = "default_break_secs")]
    pub break_duration_secs: u64,
    /// 长休息时长（秒）。缺省 15 分钟，向后兼容旧版 JSON。
    #[serde(default = "default_long_break_secs")]
    pub long_break_duration_secs: u64,
    /// 全局环境音开关 (false = 静音所有场景音景)。
    /// `serde(default = "default_sound_on")` 保证旧版 JSON 缺此字段时默认开 (true)。
    #[serde(default = "default_sound_on")]
    pub sound_on: bool,
}

fn default_sound_on() -> bool {
    true
}

fn default_focus_secs() -> u64 {
    DEFAULT_FOCUS_SECS
}
fn default_break_secs() -> u64 {
    DEFAULT_BREAK_SECS
}
fn default_long_break_secs() -> u64 {
    DEFAULT_LONG_BREAK_SECS
}

impl PomodoroState {
    /// 启动时计算 effective_now: 当前 wall-clock - saved_wall + saved_elapsed。
    /// 跨重启的 elapsed 偏移; 即 `AnimationCtx::elapsed` 应达到的值。
    pub fn effective_now_offset(&self) -> Duration {
        let now_wall = current_wall_secs();
        let delta = now_wall.saturating_sub(self.saved_wall_secs);
        Duration::from_secs(self.saved_elapsed_secs.saturating_add(delta))
    }
}

/// 当前 wall-clock Unix 秒 (失败时返回 0, 不影响持久化逻辑)。
pub fn current_wall_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 持久化文件路径 (OS 配置目录 + danqing/pomodoro.json)。
pub fn state_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("danqing").join("pomodoro.json"))
}

/// 写盘: 原子写 (临时文件 + rename)。失败不 panic, 记录错误。
pub fn save_state(state: &PomodoroState) -> io::Result<()> {
    let Some(path) = state_path() else {
        log::warn!("持久化路径不可用, 跳过保存");
        return Ok(());
    };
    save_to_path(&path, state)
}

/// 加载: 文件不存在 / 解析失败返回 None。
pub fn load_state() -> Option<PomodoroState> {
    let path = state_path()?;
    load_from_path(&path)
}

/// 写入指定路径 (测试与显式路径场景)。
pub fn save_to_path(path: &Path, state: &PomodoroState) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(state).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// 读取指定路径 (测试与显式路径场景)。
pub fn load_from_path(path: &Path) -> Option<PomodoroState> {
    let data = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&data) {
        Ok(state) => Some(state),
        Err(err) => {
            log::warn!("解析持久化文件失败: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_state_conversion_roundtrip() {
        for r in [Run::Idle, Run::Running, Run::Paused] {
            let s: RunState = r.into();
            let r2: Run = s.into();
            assert_eq!(r, r2);
        }
    }

    #[test]
    fn state_serialization_roundtrip() {
        let original = PomodoroState {
            phase: Phase::Focus,
            run: RunState::Running,
            remaining_secs: 1234,
            current_scene: 2,
            saved_elapsed_secs: 567,
            saved_wall_secs: 1_000_000,
            has_seen_shortcut_hint: true,
            completed_focus: 2,
            today_date: "2026-07-27".into(),
            today_count: 2,
            focus_duration_secs: 1500,
            break_duration_secs: 300,
            long_break_duration_secs: 900,
            sound_on: true,
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: PomodoroState = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn save_and_load_to_temp_path() {
        let dir = std::env::temp_dir().join("danqing-test-state-1");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("pomodoro.json");

        let original = PomodoroState {
            phase: Phase::Break,
            run: RunState::Paused,
            remaining_secs: 60,
            current_scene: 3,
            saved_elapsed_secs: 42,
            saved_wall_secs: 999_999,
            has_seen_shortcut_hint: false,
            completed_focus: 3,
            today_date: "2026-07-26".into(),
            today_count: 5,
            focus_duration_secs: 1500,
            break_duration_secs: 300,
            long_break_duration_secs: 900,
            sound_on: true,
        };
        save_to_path(&path, &original).unwrap();
        let loaded = load_from_path(&path).unwrap();
        assert_eq!(original, loaded);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_old_state_without_hint_field_defaults_to_false() {
        // 旧版 JSON 缺 has_seen_shortcut_hint 字段: 反序列化应默认 false (触发一次性提示)。
        let dir = std::env::temp_dir().join("danqing-test-old-hint");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("pomodoro.json");
        fs::create_dir_all(&dir).unwrap();
        let old_json = r#"{
            "phase": "Focus",
            "run": "Idle",
            "remaining_secs": 1500,
            "current_scene": 0,
            "saved_elapsed_secs": 0,
            "saved_wall_secs": 0
        }"#;
        fs::write(&path, old_json).unwrap();
        let loaded = load_from_path(&path).expect("旧版 JSON 应能加载");
        assert!(
            !loaded.has_seen_shortcut_hint,
            "缺字段时应默认为 false (触发提示)"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_old_state_without_cycle_field_defaults_to_zero() {
        // 旧版 JSON 缺 completed_focus 字段 (打磨 WS2 新增): 反序列化应默认 0。
        let dir = std::env::temp_dir().join("danqing-test-old-cycle");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("pomodoro.json");
        fs::create_dir_all(&dir).unwrap();
        let old_json = r#"{
            "phase": "Focus",
            "run": "Running",
            "remaining_secs": 700,
            "current_scene": 1,
            "saved_elapsed_secs": 10,
            "saved_wall_secs": 100,
            "has_seen_shortcut_hint": true
        }"#;
        fs::write(&path, old_json).unwrap();
        let loaded = load_from_path(&path).expect("旧版 JSON 应能加载");
        assert_eq!(loaded.completed_focus, 0, "缺字段时应默认为 0");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_old_state_without_today_fields_defaults() {
        // 旧版 JSON 缺 today_date / today_count 字段 (打磨 WS3 新增):
        // 反序列化应默认为空串与 0 (空串经 resolve_today_count 判定即归零)。
        let dir = std::env::temp_dir().join("danqing-test-old-today");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("pomodoro.json");
        fs::create_dir_all(&dir).unwrap();
        let old_json = r#"{
            "phase": "Break",
            "run": "Paused",
            "remaining_secs": 300,
            "current_scene": 0,
            "saved_elapsed_secs": 0,
            "saved_wall_secs": 0,
            "has_seen_shortcut_hint": true,
            "completed_focus": 1
        }"#;
        fs::write(&path, old_json).unwrap();
        let loaded = load_from_path(&path).expect("旧版 JSON 应能加载");
        assert!(loaded.today_date.is_empty(), "缺字段时应默认为空串");
        assert_eq!(loaded.today_count, 0, "缺字段时应默认为 0");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_nonexistent_path_returns_none() {
        let path = std::env::temp_dir().join("danqing-test-nonexistent.json");
        let _ = fs::remove_file(&path);
        assert!(load_from_path(&path).is_none());
    }

    #[test]
    fn load_from_corrupted_json_returns_none() {
        let dir = std::env::temp_dir().join("danqing-test-corrupt");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("pomodoro.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "{not json").unwrap();
        assert!(load_from_path(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn state_path_returns_pomodoro_json() {
        let path = state_path().unwrap();
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("pomodoro.json")
        );
    }

    #[test]
    fn load_old_state_without_duration_fields_defaults_to_defaults() {
        // 旧版 JSON 缺三个时长字段: 应反序列化为默认 25/5/15。
        let dir = std::env::temp_dir().join("danqing-test-old-duration");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("pomodoro.json");
        fs::create_dir_all(&dir).unwrap();
        let old_json = r#"{
            "phase": "Focus",
            "run": "Idle",
            "remaining_secs": 1500,
            "current_scene": 0,
            "saved_elapsed_secs": 0,
            "saved_wall_secs": 0,
            "has_seen_shortcut_hint": true,
            "completed_focus": 1,
            "today_date": "2026-07-31",
            "today_count": 3
        }"#;
        fs::write(&path, old_json).unwrap();
        let loaded = load_from_path(&path).expect("旧版 JSON 应能加载");
        assert_eq!(
            loaded.focus_duration_secs, 1500,
            "缺 focus_duration_secs 时应默认为 1500"
        );
        assert_eq!(
            loaded.break_duration_secs, 300,
            "缺 break_duration_secs 时应默认为 300"
        );
        assert_eq!(
            loaded.long_break_duration_secs, 900,
            "缺 long_break_duration_secs 时应默认为 900"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_old_state_without_sound_field_defaults_to_on() {
        // 旧版 JSON 缺 sound_on 字段 (Task F 新增): 应默认 true (环境音默认开)。
        let dir = std::env::temp_dir().join("danqing-test-old-sound");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("pomodoro.json");
        fs::create_dir_all(&dir).unwrap();
        let old_json = r#"{
            "phase": "Focus",
            "run": "Idle",
            "remaining_secs": 1500,
            "current_scene": 0,
            "saved_elapsed_secs": 0,
            "saved_wall_secs": 0,
            "has_seen_shortcut_hint": true,
            "completed_focus": 1,
            "today_date": "2026-08-01",
            "today_count": 2,
            "focus_duration_secs": 1500,
            "break_duration_secs": 300,
            "long_break_duration_secs": 900
        }"#;
        fs::write(&path, old_json).unwrap();
        let loaded = load_from_path(&path).expect("旧版 JSON 应能加载");
        assert!(loaded.sound_on, "缺 sound_on 字段时应默认 true");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn effective_now_offset_includes_wall_clock_delta() {
        let now_secs = current_wall_secs();
        let s = PomodoroState {
            phase: Phase::Focus,
            run: RunState::Idle,
            remaining_secs: 1500,
            current_scene: 0,
            // 假装保存于 100s 之前
            saved_elapsed_secs: 100,
            saved_wall_secs: now_secs.saturating_sub(100),
            has_seen_shortcut_hint: false,
            completed_focus: 0,
            today_date: String::new(),
            today_count: 0,
            focus_duration_secs: 1500,
            break_duration_secs: 300,
            long_break_duration_secs: 900,
            sound_on: true,
        };
        let offset = s.effective_now_offset().as_secs();
        // 期望 ≈ saved_elapsed + (now - saved_wall) = 100 + 100 = 200
        let tolerance = 2;
        assert!(
            (offset as i64 - 200).unsigned_abs() <= tolerance,
            "offset={offset}, expected ~200"
        );
    }
}
