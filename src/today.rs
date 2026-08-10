//! @author 十四叔
//! @date 2026/07/27

//! 今日完成计数 (纯逻辑)。
//!
//! 「今日」按本地日期 (chrono `Local`, YYYY-MM-DD) 判定: 日期变更则计数归零。
//! 计数持久化在 `PomodoroState` (today_date / today_count), 启动加载与每次
//! 计数前都经 `resolve_today_count` 归零判定。本地日期读取集中在
//! `today_string()`, 归零判定为纯函数, 可完整单元测试。

/// 当前本地日期串 (YYYY-MM-DD)。
///
/// 复用已有 dev-dependency `chrono`; 读取点集中于此, 测试不依赖系统时钟。
pub fn today_string() -> String {
    chrono::Local::now().date_naive().to_string()
}

/// 今日计数归零判定: 已存日期与今日相同则保留计数, 否则归零。
/// 空串 (旧版 JSON / 首次启动) 视为不同日期, 一律归零。
pub fn resolve_today_count(saved_date: &str, saved_count: u32, today: &str) -> u32 {
    if !saved_date.is_empty() && saved_date == today {
        saved_count
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_date_keeps_count() {
        assert_eq!(resolve_today_count("2026-07-27", 5, "2026-07-27"), 5);
        assert_eq!(resolve_today_count("2026-07-27", 0, "2026-07-27"), 0);
    }

    #[test]
    fn different_date_resets_count() {
        assert_eq!(resolve_today_count("2026-07-26", 5, "2026-07-27"), 0);
        assert_eq!(resolve_today_count("2026-07-27", 5, "2026-07-26"), 0);
    }

    #[test]
    fn empty_saved_date_resets_count() {
        // 旧版 JSON / 首次启动: 无已存日期, 一律归零重新累计。
        assert_eq!(resolve_today_count("", 0, "2026-07-27"), 0);
        assert_eq!(resolve_today_count("", 9, "2026-07-27"), 0);
    }

    #[test]
    fn today_string_is_local_yyyy_mm_dd() {
        let s = today_string();
        assert_eq!(s.len(), 10, "应为 YYYY-MM-DD 十字符: {s}");
        let bytes = s.as_bytes();
        assert_eq!(bytes[4], b'-');
        assert_eq!(bytes[7], b'-');
        for (i, b) in bytes.iter().enumerate() {
            if i == 4 || i == 7 {
                continue;
            }
            assert!(b.is_ascii_digit(), "位置 {i} 应为数字: {s}");
        }
        // 与 chrono 本地日期一致 (同一时刻二次取值, 跨午夜边界允许不等, 测试环境可接受)。
        assert_eq!(s, chrono::Local::now().date_naive().to_string());
    }
}
