//! @author 十四叔
//! @date 2026/09/05

//! 双轨应用内更新感知: 纯逻辑核心。
//!
//! 轨道隔离见 spec (docs/specs/update-check.md): 非 `store` 编译查 GitHub Releases,
//! `store` 编译查 StoreContext —— 两轨后端在后续任务接入, 本模块先落地可单测的纯逻辑:
//! 版本号解析/比较、检查结果缓存 (24h TTL)、「版本」行更新提示模型。
//! 约定: 任何一步解析/读写失败都按「无新版」处理, 静默不打扰。

// 纯逻辑切片先行, UI 接线在 Task 2/3 落地 —— 接线完成后移除此 allow。
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 检查结果缓存新鲜度: 24 小时内不重复发起网络/商店查询。
pub const CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// 版本号三元组 (major, minor, patch)。
pub type VersionTriple = (u64, u64, u64);

/// 解析版本串: 容忍 `v0.2.1` / `0.2.1` 两种写法, 拒绝一切非法格式
/// (段数不对、非数字、预发布后缀如 `0.2.1-beta` —— `/releases/latest` 本就不含预发布)。
pub fn parse_version(s: &str) -> Option<VersionTriple> {
    let s = s.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None; // 四段式 (如商店包版本 0.2.1.0) 不在比较域内
    }
    Some((major, minor, patch))
}

/// remote 是否严格比 current 新; 任一端解析失败按「无新版」处理。
pub fn is_newer(current: &str, remote: &str) -> bool {
    match (parse_version(current), parse_version(remote)) {
        (Some(cur), Some(rem)) => rem > cur,
        _ => false,
    }
}

/// 检查结果缓存: 落 `%APPDATA%/danqing/update-check.json`
/// (与 pomodoro.json 同目录的独立小文件, 不往现有数据文件加字段)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckCache {
    /// 上次检查成功的 wall-clock 秒 (state::current_wall_secs 同口径)。
    pub checked_at_secs: u64,
    /// 查到的远端最新版本串 (原始 tag, 如 "v0.2.1")。
    pub latest_version: String,
}

impl CheckCache {
    /// 缓存是否在 TTL 内; 时钟回拨 (checked_at 在未来) 按新鲜处理。
    pub fn is_fresh(&self, now_secs: u64) -> bool {
        now_secs.saturating_sub(self.checked_at_secs) < CACHE_TTL_SECS
    }
}

/// 缓存文件路径 (OS 配置目录 + danqing/update-check.json)。
pub fn cache_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("danqing").join("update-check.json"))
}

/// 从指定路径读缓存: 文件缺失/损坏/解析失败一律 None (交给下次重新检查)。
pub fn load_cache_from(path: &Path) -> Option<CheckCache> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// 写缓存到指定路径 (先建父目录); 失败由调用方降级为 warn 日志。
pub fn save_cache_to(path: &Path, cache: &CheckCache) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(cache).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

/// 「版本」行的更新提示模型: 有新版时给出文案与按钮。
pub struct UpdateHint {
    /// 状态文案 (版本号已归一化为 vX.Y.Z 展示)。
    pub status: String,
    /// 操作按钮文案 (按轨道分派, 见 [`update_action_text`])。
    pub action: &'static str,
}

/// 更新按钮文案: GitHub 轨跳发布页下载, 商店轨应用内拉更新。
pub fn update_action_text() -> &'static str {
    #[cfg(not(feature = "store"))]
    {
        "前往下载"
    }
    #[cfg(feature = "store")]
    {
        "更新"
    }
}

/// 由缓存计算更新提示: 无缓存 / 版本未更新 / 解析失败 → None (界面零变化);
/// 返回值即角标可见性依据 (Some = 设置按钮亮角标)。
pub fn update_hint(current: &str, cache: Option<&CheckCache>) -> Option<UpdateHint> {
    let cache = cache?;
    let (major, minor, patch) = parse_version(&cache.latest_version)?;
    if !is_newer(current, &cache.latest_version) {
        return None;
    }
    Some(UpdateHint {
        status: format!("有新版本 v{major}.{minor}.{patch}"),
        action: update_action_text(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_version ---

    #[test]
    fn parse_version_accepts_plain_and_v_prefixed() {
        assert_eq!(parse_version("0.2.1"), Some((0, 2, 1)));
        assert_eq!(parse_version("v0.2.1"), Some((0, 2, 1)));
        assert_eq!(parse_version("v10.20.30"), Some((10, 20, 30)));
    }

    #[test]
    fn parse_version_rejects_malformed() {
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("v"), None);
        assert_eq!(parse_version("0.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("0.2.1-beta"), None);
        assert_eq!(parse_version("garbage"), None);
    }

    // --- is_newer ---

    #[test]
    fn is_newer_compares_each_component() {
        assert!(is_newer("0.2.0", "0.2.1")); // patch 新
        assert!(is_newer("0.2.0", "0.3.0")); // minor 新
        assert!(is_newer("0.2.0", "1.0.0")); // major 新
        assert!(is_newer("0.2.0", "v0.2.1")); // 带 v 前缀的 tag
    }

    #[test]
    fn is_newer_rejects_equal_older_and_garbage() {
        assert!(!is_newer("0.2.0", "0.2.0")); // 相等
        assert!(!is_newer("1.0.0", "0.9.9")); // 远端更旧
        assert!(!is_newer("0.2.0", "garbage")); // 远端非法
        assert!(!is_newer("garbage", "0.2.1")); // 本地非法
    }

    // --- 缓存新鲜度 ---

    #[test]
    fn cache_freshness_respects_ttl_boundary() {
        let cache = CheckCache {
            checked_at_secs: 1000,
            latest_version: "v0.2.1".to_string(),
        };
        assert!(cache.is_fresh(1000 + CACHE_TTL_SECS - 1)); // TTL 内
        assert!(!cache.is_fresh(1000 + CACHE_TTL_SECS)); // 恰好到期算过期
        assert!(cache.is_fresh(500)); // 时钟回拨按新鲜处理
    }

    // --- 缓存读写 ---

    fn temp_cache_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pomodoro-update-check-test-{}-{tag}.json",
            std::process::id()
        ))
    }

    #[test]
    fn cache_roundtrip_preserves_content() {
        let path = temp_cache_path("roundtrip");
        let cache = CheckCache {
            checked_at_secs: 1_757_000_000,
            latest_version: "v0.2.1".to_string(),
        };
        save_cache_to(&path, &cache).expect("写缓存");
        assert_eq!(load_cache_from(&path), Some(cache));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_cache_tolerates_missing_and_corrupt() {
        let missing = temp_cache_path("missing");
        assert_eq!(load_cache_from(&missing), None);

        let corrupt = temp_cache_path("corrupt");
        std::fs::write(&corrupt, "{not json").expect("写坏文件");
        assert_eq!(load_cache_from(&corrupt), None);
        let _ = std::fs::remove_file(&corrupt);
    }

    // --- 更新提示模型 ---

    #[test]
    fn update_hint_none_without_cache_or_newer_version() {
        assert!(update_hint("0.2.0", None).is_none()); // 无缓存
        let older = CheckCache {
            checked_at_secs: 1,
            latest_version: "v0.2.0".to_string(),
        };
        assert!(update_hint("0.2.0", Some(&older)).is_none()); // 版本追平
    }

    #[test]
    fn update_hint_normalizes_display_version() {
        let newer = CheckCache {
            checked_at_secs: 1,
            latest_version: "9.9.9".to_string(), // 无 v 前缀也归一化展示
        };
        let hint = update_hint("0.2.0", Some(&newer)).expect("应有提示");
        assert_eq!(hint.status, "有新版本 v9.9.9");
        assert_eq!(hint.action, update_action_text());
    }

    #[test]
    fn action_text_matches_track() {
        #[cfg(not(feature = "store"))]
        assert_eq!(update_action_text(), "前往下载");
        #[cfg(feature = "store")]
        assert_eq!(update_action_text(), "更新");
    }
}
