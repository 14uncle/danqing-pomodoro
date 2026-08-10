//! @author 十四叔
//! @date 2026/08/01

//! 专注会话历史(数据层): 每次自然完成的 Focus 记录一条会话。
//!
//! 独立存储(不污染 `pomodoro.json`, 避免无界增长): `%APPDATA%/danqing/focus-history.json`。
//! 数据格式从第一天为十年设计: 顶层 `format_version` + 字段级 `#[serde(default)]` —
//! 未来加字段不破坏旧文件; 未来大版本 (format_version 提高) 拒读不覆盖, 防止降级毁数据。
//! 明文 JSON 存储, 可导出 CSV 供人阅读。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, TimeZone};
use serde::{Deserialize, Serialize};

use crate::scenes::SCENES;

/// 当前历史文件格式版本。
pub const FORMAT_VERSION: u32 = 1;

/// 一条专注会话记录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    /// 会话开始墙钟 Unix 秒。
    #[serde(default)]
    pub started_ts: u64,
    /// 自然完成时刻墙钟 Unix 秒。
    #[serde(default)]
    pub completed_ts: u64,
    /// 计划专注时长 (完成时刻的配置, 秒)。
    #[serde(default)]
    pub planned_secs: u64,
    /// 实际专注时长 (累计 running Focus, 排除暂停, 秒)。
    #[serde(default)]
    pub focused_secs: u64,
    /// 完成时场景索引。
    #[serde(default)]
    pub scene_index: usize,
    /// 完成时在大循环内的轮次 (1..=CYCLE_LENGTH)。
    #[serde(default)]
    pub round_in_cycle: u8,
    /// 是否自然完成 (MVP 只记自然完成, true; 字段预留 skip 等其它终止语义)。
    #[serde(default = "default_completed_true")]
    pub completed: bool,
}

fn default_completed_true() -> bool {
    true
}

/// 专注会话历史: 版本化容器 (追加式, 按完成时间序)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusHistory {
    /// 历史文件格式版本; 未来大版本提高后旧程序拒读不覆盖。
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    /// 会话记录。
    #[serde(default)]
    pub sessions: Vec<SessionRecord>,
    /// 加载时发现未来版本文件后置位: 禁止任何覆盖写入 (防止降级把新版本数据
    /// 覆盖成空历史)。不参与序列化 (纯运行时保护)。
    #[serde(skip, default)]
    pub refuse_overwrite: bool,
}

fn default_format_version() -> u32 {
    FORMAT_VERSION
}

/// 某本地年的年度摘要 (深度洞察; 纯读聚合, 不触碰写路径)。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct YearSummary {
    /// 专注总秒数。
    pub total_secs: u64,
    /// 会话数。
    pub session_count: u32,
    /// 活跃天数 (本地日期去重)。
    pub active_days: u32,
    /// 按 `scene_index` 索引的专注秒数。
    pub scene_secs: Vec<u64>,
}

impl FocusHistory {
    /// 空历史。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一条会话 (调用方保证按完成时间序追加)。
    pub fn push(&mut self, record: SessionRecord) {
        self.sessions.push(record);
    }

    /// 最近 7 天 (滚动窗口, 含今天) 完成数 + 专注秒数。
    pub fn week_stats(&self, now_wall: u64) -> (u32, u64) {
        let cutoff = now_wall.saturating_sub(7 * 86_400);
        self.sessions
            .iter()
            .filter(|s| s.completed_ts >= cutoff && s.completed_ts <= now_wall)
            .fold((0u32, 0u64), |(c, f), s| (c + 1, f + s.focused_secs))
    }

    /// 累计完成数 + 专注秒数。
    pub fn total_stats(&self) -> (u32, u64) {
        self.sessions
            .iter()
            .fold((0u32, 0u64), |(c, f), s| (c + 1, f + s.focused_secs))
    }

    /// 某本地年的年度摘要: 过滤 `completed_ts` 落在该年的记录。
    /// 纯读聚合, 不新增写/导出路径 (`refuse_overwrite` 保护不受影响)。
    pub fn year_summary(&self, year: u32) -> YearSummary {
        let mut scene_secs: Vec<u64> = Vec::new();
        let mut active_days = std::collections::HashSet::new();
        let mut summary = YearSummary::default();
        for s in &self.sessions {
            if local_ym(s.completed_ts).0 != year {
                continue;
            }
            summary.total_secs += s.focused_secs;
            summary.session_count += 1;
            active_days.insert(local_ymd(s.completed_ts));
            if s.scene_index >= scene_secs.len() {
                scene_secs.resize(s.scene_index + 1, 0);
            }
            scene_secs[s.scene_index] += s.focused_secs;
        }
        summary.active_days = active_days.len() as u32;
        summary.scene_secs = scene_secs;
        summary
    }

    /// 近 N 个日历月逐月专注秒数: 按 (year, month) 升序, 缺月补零,
    /// 末项为 `now_wall` 所在月。`now_wall` 入参保证测试不依赖系统时钟。
    pub fn month_trend(&self, now_wall: u64, months: u32) -> Vec<(u32, u32, u64)> {
        let (now_y, now_m) = local_ym(now_wall);
        let end = now_y as i64 * 12 + i64::from(now_m) - 1;
        let start = end - i64::from(months) + 1;
        (start..=end)
            .map(|idx| {
                let (y, m) = ((idx / 12) as u32, (idx % 12 + 1) as u32);
                let secs = self
                    .sessions
                    .iter()
                    .filter(|s| local_ym(s.completed_ts) == (y, m))
                    .fold(0u64, |acc, s| acc + s.focused_secs);
                (y, m, secs)
            })
            .collect()
    }

    /// CSV 明文导出 (首行表头, 供人阅读): 时间戳转本地时间、场景索引转名字、
    /// 时长转 mm:ss、轮次转 "N/4"。机器可读的原始字段在 `focus-history.json`,
    /// CSV 是给人看的, 不保留裸 Unix 秒 / 数字索引。
    pub fn export_csv(&self) -> String {
        // UTF-8 BOM (\u{FEFF}): Excel 打开无 BOM 的 UTF-8 CSV 会按 ANSI 解码导致中文乱码,
        // BOM 让 Excel 识别 UTF-8 编码。
        let mut out = String::from("\u{FEFF}开始时间,完成时间,计划时长,实际专注,场景,轮次\n");
        for s in &self.sessions {
            out.push_str(&format!(
                "{},{},{},{},{},{}\n",
                format_ts(s.started_ts),
                format_ts(s.completed_ts),
                format_dur(s.planned_secs),
                format_dur(s.focused_secs),
                scene_name(s.scene_index),
                round_label(s.round_in_cycle),
            ));
        }
        out
    }
}

/// Unix 秒 → 本地时间 "YYYY-MM-DD HH:MM:SS" (无效时间戳兜底)。
fn format_ts(ts: u64) -> String {
    match Local.timestamp_opt(ts as i64, 0).single() {
        Some(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => "无效时间".into(),
    }
}

/// 秒数 → "MM:SS" (分钟:秒, 与倒计时同刻度)。
fn format_dur(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// 场景索引 → 名字 (越界兜底 "未知")。
fn scene_name(idx: usize) -> &'static str {
    SCENES.get(idx).map(|s| s.name).unwrap_or("未知")
}

/// 轮次 → "N/循环长度" (0 表示无轮次语义, 显示占位)。
fn round_label(round: u8) -> String {
    if round == 0 {
        "—".into()
    } else {
        format!("{round}/{}", super::timer::CYCLE_LENGTH)
    }
}

impl Default for FocusHistory {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            sessions: Vec::new(),
            refuse_overwrite: false,
        }
    }
}

/// 历史文件路径: OS 配置目录 + danqing/focus-history.json。
pub fn history_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("danqing").join("focus-history.json"))
}

/// 保存 (原子写: 临时文件 + rename)。失败不 panic, 记录错误。
pub fn save_history(history: &FocusHistory) -> io::Result<()> {
    let Some(path) = history_path() else {
        log::warn!("历史路径不可用, 跳过保存");
        return Ok(());
    };
    save_history_to_path(&path, history)
}

/// 导出历史为 CSV 到指定路径。失败返回用户可读的短原因 (静态文案,
/// 面板直接显示; 完整 OS 错误由调用方记录日志)。
pub fn export_csv_to(path: &Path, history: &FocusHistory) -> Result<(), &'static str> {
    if history.refuse_overwrite {
        return Err("检测到更高版本数据, 拒绝导出");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "创建导出目录失败")?;
    }
    std::fs::write(path, history.export_csv()).map_err(|_| "导出写入失败")?;
    Ok(())
}

/// 加载历史 (应用入口, 带降级数据保护)。
/// - 文件缺失 → 空历史 (可写)
/// - 文件存在且可解析 → 正常加载; 未来大版本拒读且拒写
/// - 文件存在但不可解析 (损坏 / 未来版本改了字段类型) → 空历史 + 拒写保护,
///   防止后续保存把新版本数据覆盖成空历史 (见 [`load_history_guarded`])。
pub fn load_history() -> FocusHistory {
    let Some(path) = history_path() else {
        return FocusHistory::new();
    };
    load_history_guarded(&path)
}

/// 带降级保护的历史加载 (可测): 文件存在但读不出/解析不出 → 拒写保护。
fn load_history_guarded(path: &Path) -> FocusHistory {
    if !path.exists() {
        return FocusHistory::new();
    }
    match load_history_from_path(path) {
        Some(history) => history,
        None => {
            log::warn!(
                "历史文件存在但无法解析 (损坏或来自更高版本), 拒写保护以防覆盖: {}",
                path.display()
            );
            let mut history = FocusHistory::new();
            history.refuse_overwrite = true;
            history
        }
    }
}

/// 写入指定路径 (测试与显式路径场景)。
/// 若历史标记了 `refuse_overwrite` (加载到未来版本文件), 拒绝写入以防降级覆盖。
pub fn save_history_to_path(path: &Path, history: &FocusHistory) -> io::Result<()> {
    if history.refuse_overwrite {
        log::warn!(
            "历史文件版本高于本程序, 拒绝覆盖写入以保护新版本数据: {}",
            path.display()
        );
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(history).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// 读取指定路径 (测试与显式路径场景)。
/// 未来大版本文件: 返回空历史 + `refuse_overwrite = true` (拒读且拒写, 防降级覆盖)。
pub fn load_history_from_path(path: &Path) -> Option<FocusHistory> {
    let data = fs::read_to_string(path).ok()?;
    let mut history: FocusHistory = serde_json::from_str(&data).ok()?;
    if history.format_version > FORMAT_VERSION {
        log::warn!(
            "历史文件版本 {} 高于本程序 {} (新版本软件写的数据), 拒读且后续拒写防降级覆盖",
            history.format_version,
            FORMAT_VERSION
        );
        history.sessions = Vec::new();
        history.refuse_overwrite = true;
    }
    Some(history)
}

/// epoch 秒 → 本地 (year, month)。出界/不可解析回退 (0, 0)。
fn local_ym(ts: u64) -> (u32, u32) {
    let (y, m, _) = local_ymd(ts);
    (y, m)
}

/// epoch 秒 → 本地 (year, month, day)。出界/不可解析回退 (0, 0, 0)。
fn local_ymd(ts: u64) -> (u32, u32, u32) {
    match Local.timestamp_opt(ts as i64, 0).single() {
        Some(dt) => (dt.year() as u32, dt.month(), dt.day()),
        None => (0, 0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(completed_ts: u64) -> SessionRecord {
        SessionRecord {
            started_ts: completed_ts.saturating_sub(1500),
            completed_ts,
            planned_secs: 1500,
            focused_secs: 1500,
            scene_index: 0,
            round_in_cycle: 1,
            completed: true,
        }
    }

    /// 构造本地时间戳 (正午, 避开 DST 边界歧义)。构造与断言同用 Local, 时区自洽。
    fn local_ts(y: i32, m: u32, d: u32) -> u64 {
        Local
            .with_ymd_and_hms(y, m, d, 12, 0, 0)
            .single()
            .expect("构造本地时间戳")
            .timestamp() as u64
    }

    #[test]
    fn session_record_serialization_roundtrip() {
        let original = record(1_000_000);
        let json = serde_json::to_string(&original).unwrap();
        let back: SessionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn history_save_load_roundtrip() {
        let dir = std::env::temp_dir().join("danqing-test-history-1");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.json");

        let mut history = FocusHistory::new();
        history.push(record(1_000_000));
        history.push(record(1_000_100));
        save_history_to_path(&path, &history).unwrap();
        let loaded = load_history_from_path(&path).expect("应能加载");
        assert_eq!(loaded.format_version, FORMAT_VERSION);
        assert_eq!(loaded.sessions.len(), 2);
        assert_eq!(loaded.sessions[0], record(1_000_000));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_history_without_format_version_defaults_to_current() {
        // 旧版 JSON 缺 format_version 字段: 应默认为当前版本, 正常加载。
        let dir = std::env::temp_dir().join("danqing-test-history-old");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, r#"{"sessions":[]}"#).unwrap();
        let loaded = load_history_from_path(&path).expect("缺 format_version 应能加载");
        assert_eq!(loaded.format_version, FORMAT_VERSION);
        assert!(loaded.sessions.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_history_missing_record_fields_defaults() {
        // 记录缺 scene_index / round_in_cycle 等字段 (未来新增): 应默认为 0。
        let dir = std::env::temp_dir().join("danqing-test-history-missing");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &path,
            r#"{"format_version":1,"sessions":[{"started_ts":1,"completed_ts":1501,"planned_secs":1500,"focused_secs":1500}]}"#,
        )
        .unwrap();
        let loaded = load_history_from_path(&path).expect("缺字段记录应能加载");
        assert_eq!(loaded.sessions[0].scene_index, 0);
        assert_eq!(loaded.sessions[0].round_in_cycle, 0);
        assert!(loaded.sessions[0].completed, "缺 completed 应默认为 true");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_history_future_version_refused() {
        // format_version 高于当前: 拒读 (空历史) 且置 refuse_overwrite, 防降级覆盖。
        let dir = std::env::temp_dir().join("danqing-test-history-future");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, r#"{"format_version":99,"sessions":[]}"#).unwrap();
        let loaded = load_history_from_path(&path).expect("未来版本应返回受保护的空历史");
        assert!(loaded.sessions.is_empty(), "未来版本数据不应被加载");
        assert!(loaded.refuse_overwrite, "应置 refuse_overwrite 以拒写");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_refuses_to_overwrite_future_version_file() {
        // 核心数据保护: 加载到未来版本文件后, 后续保存必须不触碰原文件。
        let dir = std::env::temp_dir().join("danqing-test-history-refuse");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.json");
        fs::create_dir_all(&dir).unwrap();
        let future_data = r#"{"format_version":99,"sessions":[{"started_ts":1}]}"#;
        fs::write(&path, future_data).unwrap();

        let mut history = load_history_from_path(&path).expect("未来版本应能识别");
        assert!(history.refuse_overwrite);
        history.push(record(42)); // 运行中新会话入内存
        save_history_to_path(&path, &history).expect("拒写应返回 Ok (静默跳过)");

        let on_disk = fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, future_data, "未来版本文件必须保持原样, 不得被覆盖");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_history_from_nonexistent_returns_none() {
        let path = std::env::temp_dir().join("danqing-test-history-nonexistent.json");
        let _ = fs::remove_file(&path);
        assert!(load_history_from_path(&path).is_none());
    }

    #[test]
    fn guarded_load_missing_file_is_writable() {
        let path = std::env::temp_dir().join("danqing-test-guarded-missing.json");
        let _ = fs::remove_file(&path);
        let history = load_history_guarded(&path);
        assert!(!history.refuse_overwrite, "文件缺失应可写 (全新会话)");
    }

    #[test]
    fn guarded_load_corrupt_file_is_protected() {
        // 文件存在但损坏 (非 JSON): 拒读且拒写, 防后续覆盖。
        let dir = std::env::temp_dir().join("danqing-test-guarded-corrupt");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "{not json").unwrap();
        let history = load_history_guarded(&path);
        assert!(history.refuse_overwrite, "损坏文件应拒写保护");
        assert!(history.sessions.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unparseable_future_file_survives_save() {
        // C1 回归: 未来版本改了字段类型 (started_ts u64 → 字符串), serde 解析失败。
        // 加载必须保护拒写, 后续保存不得覆盖原文件。
        let dir = std::env::temp_dir().join("danqing-test-history-typechange");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.json");
        fs::create_dir_all(&dir).unwrap();
        let future_data =
            r#"{"format_version":2,"sessions":[{"started_ts":"2026-08-01T10:00:00Z"}]}"#;
        fs::write(&path, future_data).unwrap();

        let mut history = load_history_guarded(&path);
        assert!(history.refuse_overwrite, "类型变更的未来文件应拒写保护");
        history.push(record(42));
        save_history_to_path(&path, &history).expect("拒写应返回 Ok");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            future_data,
            "未来版本文件必须保持原样"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn week_stats_rolling_window_excludes_old() {
        let now = 2_000_000u64;
        let mut history = FocusHistory::new();
        history.push(record(now - 10 * 86_400)); // 10 天前: 窗口外
        history.push(record(now - 6 * 86_400)); // 6 天前: 窗口内
        history.push(record(now)); // 今天: 窗口内
        let (count, focused) = history.week_stats(now);
        assert_eq!(count, 2);
        assert_eq!(focused, 2 * 1500);
    }

    #[test]
    fn total_stats_sums_all() {
        let mut history = FocusHistory::new();
        history.push(record(1));
        history.push(record(2));
        history.push(record(3));
        let (count, focused) = history.total_stats();
        assert_eq!(count, 3);
        assert_eq!(focused, 3 * 1500);
    }

    #[test]
    fn empty_history_stats_zero() {
        let history = FocusHistory::new();
        assert_eq!(history.week_stats(1_000_000), (0, 0));
        assert_eq!(history.total_stats(), (0, 0));
    }

    #[test]
    fn export_csv_is_human_readable() {
        let mut history = FocusHistory::new();
        history.push(record(100));
        let csv = history.export_csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2, "首行表头 + 一行数据");
        assert!(
            lines[0].starts_with("\u{FEFF}开始时间,"),
            "表头应带 UTF-8 BOM 且人读: {:?}",
            lines[0]
        );
        let row = lines[1];
        assert!(row.contains("1970-"), "时间戳应转本地日期: {row}");
        assert!(row.contains("25:00"), "时长应 mm:ss: {row}");
        assert!(row.contains(SCENES[0].name), "场景应为人读名字: {row}");
        assert!(row.contains("1/4"), "轮次应可见: {row}");
    }

    #[test]
    fn export_csv_writes_file_and_reports_ok() {
        let dir = std::env::temp_dir().join("danqing-test-export-ok");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.csv");
        let mut history = FocusHistory::new();
        history.push(record(100));
        let result = export_csv_to(&path, &history);
        assert_eq!(result, Ok(()));
        let csv = fs::read_to_string(&path).expect("导出文件应存在");
        assert!(
            csv.starts_with("\u{FEFF}开始时间,"),
            "应有表头 (含 UTF-8 BOM)"
        );
        assert!(csv.contains(SCENES[0].name), "应有一行数据 (场景名)");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_refuses_future_version_without_touching_file() {
        let dir = std::env::temp_dir().join("danqing-test-export-refuse");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("focus-history.csv");
        fs::write(&path, "sentinel").unwrap();
        let mut history = FocusHistory::new();
        history.refuse_overwrite = true;
        history.push(record(42));
        let result = export_csv_to(&path, &history);
        assert!(result.is_err(), "受保护历史应拒绝导出");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "sentinel",
            "拒绝导出时不得触碰原文件"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_fails_when_parent_path_is_a_file() {
        let dir = std::env::temp_dir().join("danqing-test-export-parent-file");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("blocker"), "file").unwrap();
        let path = dir.join("blocker").join("sub").join("focus-history.csv");
        let history = FocusHistory::new();
        assert_eq!(export_csv_to(&path, &history), Err("创建导出目录失败"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_fails_when_target_is_a_directory() {
        let dir = std::env::temp_dir().join("danqing-test-export-dir-target");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("focus-history.csv")).unwrap();
        let path = dir.join("focus-history.csv");
        let history = FocusHistory::new();
        assert_eq!(export_csv_to(&path, &history), Err("导出写入失败"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_history_writes_atomically_visible_file() {
        // 保存后文件为最终名 (非 .tmp 残留), 且可被加载。
        let dir = std::env::temp_dir().join("danqing-test-history-atomic");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("focus-history.json");
        let mut history = FocusHistory::new();
        history.push(record(42));
        save_history_to_path(&path, &history).unwrap();
        assert!(path.exists(), "最终文件应存在");
        assert!(
            !path.with_extension("json.tmp").exists(),
            "临时文件不应残留"
        );
        let loaded = load_history_from_path(&path).expect("应能加载");
        assert_eq!(loaded.sessions.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn year_summary_filters_by_year() {
        let mut history = FocusHistory::new();
        history.push(record(local_ts(2025, 6, 15)));
        history.push(record(local_ts(2026, 6, 15)));
        let s25 = history.year_summary(2025);
        assert_eq!(s25.session_count, 1);
        assert_eq!(s25.total_secs, 1500);
        assert_eq!(s25.active_days, 1);
        let s26 = history.year_summary(2026);
        assert_eq!(s26.session_count, 1);
        assert_eq!(s26.total_secs, 1500);
        let s24 = history.year_summary(2024);
        assert_eq!(s24.session_count, 0);
        assert_eq!(s24.total_secs, 0);
        assert_eq!(s24.active_days, 0);
        assert!(s24.scene_secs.is_empty(), "空年份无场景分布");
    }

    #[test]
    fn year_summary_cross_year_boundary() {
        // 2025-12-31 与 2026-01-01 记录互不串年。
        let mut history = FocusHistory::new();
        history.push(record(local_ts(2025, 12, 31)));
        history.push(record(local_ts(2026, 1, 1)));
        assert_eq!(history.year_summary(2025).session_count, 1);
        assert_eq!(history.year_summary(2026).session_count, 1);
    }

    #[test]
    fn year_summary_active_days_dedup() {
        let mut history = FocusHistory::new();
        history.push(record(local_ts(2026, 3, 1)));
        history.push(record(local_ts(2026, 3, 1)));
        history.push(record(local_ts(2026, 3, 2)));
        let s = history.year_summary(2026);
        assert_eq!(s.session_count, 3);
        assert_eq!(s.active_days, 2, "同日去重, 异日累加");
    }

    #[test]
    fn year_summary_scene_distribution() {
        let mut history = FocusHistory::new();
        let mut r0 = record(local_ts(2026, 3, 1));
        r0.scene_index = 0;
        let mut r2 = record(local_ts(2026, 3, 2));
        r2.scene_index = 2;
        history.push(r0);
        history.push(r2);
        let s = history.year_summary(2026);
        assert_eq!(s.scene_secs, vec![1500, 0, 1500], "按 scene_index 归位");
    }

    #[test]
    fn month_trend_zero_fills_and_ends_at_now_month() {
        let now = local_ts(2026, 3, 15);
        let mut history = FocusHistory::new();
        history.push(record(local_ts(2026, 1, 10)));
        history.push(record(local_ts(2026, 3, 5)));
        let trend = history.month_trend(now, 3);
        assert_eq!(trend, vec![(2026, 1, 1500), (2026, 2, 0), (2026, 3, 1500)]);
    }

    #[test]
    fn month_trend_cross_year() {
        let now = local_ts(2026, 2, 10);
        let mut history = FocusHistory::new();
        history.push(record(local_ts(2025, 12, 20)));
        history.push(record(local_ts(2026, 1, 5)));
        let trend = history.month_trend(now, 4);
        assert_eq!(
            trend,
            vec![
                (2025, 11, 0),
                (2025, 12, 1500),
                (2026, 1, 1500),
                (2026, 2, 0)
            ]
        );
    }

    #[test]
    fn empty_history_aggregations_zero() {
        let history = FocusHistory::new();
        let s = history.year_summary(2026);
        assert_eq!(s.session_count, 0);
        assert_eq!(s.total_secs, 0);
        assert_eq!(s.active_days, 0);
        assert!(s.scene_secs.is_empty());
        let trend = history.month_trend(local_ts(2026, 1, 15), 12);
        assert_eq!(trend.len(), 12);
        assert!(trend.iter().all(|&(_, _, secs)| secs == 0), "缺月补零");
    }
}
