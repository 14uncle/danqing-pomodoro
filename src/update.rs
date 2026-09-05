//! @author 十四叔
//! @date 2026/09/05

//! 双轨应用内更新感知: 纯逻辑 + 两轨 cfg 后端。
//!
//! 轨道编译期隔离见 spec (docs/specs/update-check.md): 非 `store` 编译查
//! GitHub Releases (ureq), `store` 编译查/拉 StoreContext (store 模块);
//! `fetch_latest_version` 两份 cfg 定义即轨道接缝。
//! 纯逻辑: 版本号解析/比较、检查结果缓存 (24h TTL)、「版本」行更新提示模型。
//! 约定: 任何一步解析/读写失败都按「无新版」处理, 静默不打扰。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 检查结果缓存新鲜度: 24 小时内不重复发起网络/商店查询。
pub const CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// 版本号三元组 (major, minor, patch)。
pub type VersionTriple = (u64, u64, u64);

/// 当前版本号: GitHub 轨取编译期包版本, 商店轨取 MSIX 包身份版本
/// (build_msix.ps1 的 -Version 独立于 Cargo.toml, 二进制自报不可信)。
///
/// 进程级缓存: 版本号在两轨上都是进程生命周期常量 (商店轨更新必须重启进程
/// 才生效), 而本函数每帧被 UI 绑定多次调用 —— 不缓存会让商店轨开发形态
/// (无包身份) 每帧重复 WinRT 失败并刷 warn。
pub fn current_version() -> &'static str {
    static CURRENT_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CURRENT_VERSION.get_or_init(current_version_uncached)
}

/// 实际读取 (每进程一次, 见 [`current_version`])。
fn current_version_uncached() -> String {
    #[cfg(not(feature = "store"))]
    {
        env!("CARGO_PKG_VERSION").to_string()
    }
    #[cfg(feature = "store")]
    {
        store::package_version()
    }
}

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

/// 检查结论 (随缓存落盘)。两轨信息不对称:
/// GitHub 轨知道新版本号, 商店轨只知「有/没有」。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UpdateStatus {
    /// 已是最新。
    UpToDate,
    /// 有新版且已知版本号 (GitHub 轨: releases/latest 的 tag)。
    KnownVersion(String),
    /// 有新版但版本号未知 (商店轨: StorePackageUpdate.Package 是「被更新的
    /// 当前包」, 不暴露新版本号 —— 2026-09-05 侧载实测, 详见 spec 落地记录)。
    UnknownVersion,
}

/// 检查结果缓存: 落 `%APPDATA%/danqing/update-check.json`
/// (与 pomodoro.json 同目录的独立小文件, 不往现有数据文件加字段)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckCache {
    /// 上次检查成功的 wall-clock 秒 (state::current_wall_secs 同口径)。
    pub checked_at_secs: u64,
    /// 写入本缓存的二进制版本: 换版 (更新/降级) 后旧缓存作废, 见 [`usable_cache`]。
    pub checked_version: String,
    /// 检查结论 (见 [`UpdateStatus`] 各轨语义)。
    pub status: UpdateStatus,
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

/// 全局检查结果: 后台线程 [`publish`] 写, UI 线程经 [`current_hint`] 每帧读。
static CHECK_CACHE: std::sync::Mutex<Option<CheckCache>> = std::sync::Mutex::new(None);

/// 发布检查结果 (覆盖式; None = 清空)。锁中毒时放弃本次发布 (不 panic)。
pub(crate) fn publish(cache: Option<CheckCache>) {
    if let Ok(mut guard) = CHECK_CACHE.lock() {
        *guard = cache;
    }
}

/// 当前更新提示: 全局缓存 + 当前版本合成, UI 每帧调用。
pub fn current_hint() -> Option<UpdateHint> {
    // 克隆出锁再合成: 收窄临界区, 持锁期间不做版本合成。
    let cache = CHECK_CACHE.lock().ok()?.clone();
    update_hint(current_version(), cache.as_ref())
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

/// 由缓存计算更新提示: 无缓存 / 已是最新 / 版本号解析失败 → None (界面零变化);
/// 返回值即角标可见性依据 (Some = 设置按钮亮角标)。
pub fn update_hint(current: &str, cache: Option<&CheckCache>) -> Option<UpdateHint> {
    match &cache?.status {
        UpdateStatus::UpToDate => None,
        // 商店轨: 版本号不可得, 提示不显示版本 (spec 成功标准 3)。
        UpdateStatus::UnknownVersion => Some(UpdateHint {
            status: "有新版本".to_string(),
            action: update_action_text(),
        }),
        UpdateStatus::KnownVersion(latest) => {
            let (major, minor, patch) = parse_version(latest)?;
            if !is_newer(current, latest) {
                return None;
            }
            Some(UpdateHint {
                status: format!("有新版本 v{major}.{minor}.{patch}"),
                action: update_action_text(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// 微软商店: 包身份版本读取
// ---------------------------------------------------------------------------

#[cfg(feature = "store")]
mod store {
    use super::UpdateStatus;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::Services::Store::{StoreContext, StorePackageUpdate};
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};

    /// 更新拉起在途标志: 系统对话框存活期间忽略重复点击 (防重入)。
    static UPDATE_REQUESTING: AtomicBool = AtomicBool::new(false);

    /// 读 MSIX 包身份版本 (Major.Minor.Build, 丢弃 build_msix.ps1 恒写 0 的 Revision);
    /// 无包身份 (`cargo run --features store` 直跑) 回退编译期版本 + warn。
    pub fn package_version() -> String {
        let read = (|| -> windows::core::Result<String> {
            let pkg = windows::ApplicationModel::Package::Current()?;
            let version = pkg.Id()?.Version()?;
            // 丢弃 Revision: build_msix.ps1 恒写 0
            Ok(format!(
                "{}.{}.{}",
                version.Major, version.Minor, version.Build
            ))
        })();
        match read {
            Ok(version) => version,
            Err(err) => {
                log::warn!("无 MSIX 包身份, 版本号回退编译期值: {err}");
                env!("CARGO_PKG_VERSION").to_string()
            }
        }
    }

    /// 查商店应用更新: 有待装更新 → UnknownVersion; 无更新 → UpToDate;
    /// 非 MSIX 环境/任何 API 失败 → None 静默 (spec 约束 4)。
    ///
    /// 注意 StorePackageUpdate.Package 是「被更新的当前包」, 新版本号不可得,
    /// 所以商店轨只有有无、没有版本 (2026-09-05 侧载实测)。
    pub fn check_update() -> Option<UpdateStatus> {
        if !crate::license::is_running_as_msix() {
            return None;
        }
        match check_update_inner() {
            Ok(status) => Some(status),
            Err(err) => {
                log::warn!("商店更新检查失败: {err}");
                None
            }
        }
    }

    /// 同步执行商店更新查询 (调用方负责线程)。
    fn check_update_inner() -> windows::core::Result<UpdateStatus> {
        // WinRT 异步调用需 COM 单元 (同 IAP 纪律: 成败都继续, 线程退出不配对 RoUninitialize)。
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let context = StoreContext::GetDefault()?;
        let updates = context.GetAppAndOptionalStorePackageUpdatesAsync()?.get()?;
        let count = updates.Size()?;
        log::info!("商店更新查询: 待装更新 {count} 个");
        Ok(if count == 0 {
            UpdateStatus::UpToDate
        } else {
            UpdateStatus::UnknownVersion
        })
    }

    /// 拉起商店系统更新 UI (后台线程: 同步等待会阻塞 UI)。
    /// 系统对话框接管进度/安装/重启提示; 任何失败仅一行 warn。
    pub fn request_update() {
        if !crate::license::is_running_as_msix() {
            return; // 双保险: 非 MSIX 不产生提示, 正常不可达
        }
        // 防重入: 系统对话框在途期间忽略重复点击 (IAP 购买链路同款纪律)。
        if UPDATE_REQUESTING.swap(true, Ordering::AcqRel) {
            return;
        }
        std::thread::spawn(|| {
            if let Err(err) = request_update_inner() {
                log::warn!("拉起商店更新失败: {err}");
            }
            UPDATE_REQUESTING.store(false, Ordering::Release);
        });
    }

    /// 同步执行商店更新拉起 (调用方负责线程)。
    fn request_update_inner() -> windows::core::Result<()> {
        use windows::Win32::UI::Shell::IInitializeWithWindow;
        use windows::core::Interface;

        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let context = StoreContext::GetDefault()?;
        // 同购买对话框: 显示 UI 的 Store 调用必须先挂主窗口属主 (IInitializeWithWindow 约定)。
        let Some(hwnd) = crate::license::find_main_window() else {
            log::warn!("主窗口未找到, 无法挂更新对话框属主");
            return Ok(());
        };
        unsafe { context.cast::<IInitializeWithWindow>()?.Initialize(hwnd)? };
        let updates = context.GetAppAndOptionalStorePackageUpdatesAsync()?.get()?;
        // WinRT 引用类型的 IIterable<T> 由 Vec<Option<T>> 转换
        // (T::Default = Option<T>, 与 HSTRING 等值类型的直转不同)。
        let updates: Vec<Option<StorePackageUpdate>> = updates.into_iter().map(Some).collect();
        if updates.is_empty() {
            // 用户点击与检查之间更新已被安装/撤下: 无事发生
            return Ok(());
        }
        let updates: windows_collections::IIterable<StorePackageUpdate> = updates.into();
        // Param<IIterable> 由 &U 实现, 按值不行 (IAP 同款, 见 memory)。
        context
            .RequestDownloadAndInstallStorePackageUpdatesAsync(&updates)?
            .get()?;
        log::info!("商店更新流程已结束 (系统对话框接管后续)");
        Ok(())
    }
}

/// 缓存仅对写入它的二进制版本有效: 版本不一致 (更新/降级) 即作废重查 ——
/// 商店轨 UnknownVersion 无版本号, 无法像 KnownVersion 那样经 is_newer 自纠,
/// 换版 (含商店自动更新) 后继续发布会把「有新版本」误驻留至多 24h
/// (2026-09-05 复跑验证揪出)。
fn usable_cache(cache: Option<CheckCache>, current: &str) -> Option<CheckCache> {
    cache.filter(|c| c.checked_version == current)
}

/// 启动更新检查: 先读缓存立即发布 (GitHub 轨过期缓存经 is_newer 自纠, 商店轨
/// 靠 [`usable_cache`] 版本闸门), 缓存过期/缺失/版本不符才后台线程重查;
/// 成功写缓存并发布, 失败静默 (一行 warn, 本次会话不重试)。
pub fn spawn_check() {
    let cached = usable_cache(
        cache_path().and_then(|p| load_cache_from(&p)),
        current_version(),
    );
    let fresh = cached
        .as_ref()
        .is_some_and(|c| c.is_fresh(crate::state::current_wall_secs()));
    publish(cached);
    if fresh {
        return;
    }
    // 后台线程不 join: 进程退出即终止, 无泄漏 (spec R4)。
    std::thread::spawn(|| match fetch_update_status() {
        Some(status) => {
            let cache = CheckCache {
                checked_at_secs: crate::state::current_wall_secs(),
                checked_version: current_version().to_string(),
                status,
            };
            match cache_path() {
                Some(path) => {
                    if let Err(err) = save_cache_to(&path, &cache) {
                        log::warn!("更新检查缓存写入失败: {err}");
                    }
                }
                // 刚侧载的包首启可能拿不到配置目录 (包状态未初始化完):
                // 跳过落盘, 下次启动重查 — 2026-09-05 复跑实测。
                None => log::warn!("配置目录不可得, 更新检查结果不落盘"),
            }
            publish(Some(cache));
        }
        None => log::warn!("更新检查失败, 本次会话不再重试"),
    });
}

/// 执行更新动作 (设置面板「版本」行按钮; 行为按轨道分派)。
pub fn perform_action() {
    #[cfg(not(feature = "store"))]
    {
        // GitHub 轨: 跳发布页手动下载 (自动替换 exe 不属本期范围, 见 spec)
        if let Err(err) = open::that(RELEASES_PAGE) {
            log::warn!("打开发布页失败: {err}");
        }
    }
    #[cfg(feature = "store")]
    {
        // 商店轨: 拉起系统更新 UI (下载/安装/重启提示由系统对话框接管)
        store::request_update();
    }
}

// ---------------------------------------------------------------------------
// 检查后端: 轨道编译期隔离 (spec: 商店轨绝口不提 GitHub, 反之亦然)
// ---------------------------------------------------------------------------

/// GitHub Releases API 端点 (latest 天然排除 draft/prerelease)。
#[cfg(not(feature = "store"))]
const RELEASES_API: &str = "https://api.github.com/repos/14uncle/danqing-pomodoro/releases/latest";
/// 发布页: 「前往下载」跳转目标。
#[cfg(not(feature = "store"))]
pub const RELEASES_PAGE: &str = "https://github.com/14uncle/danqing-pomodoro/releases/latest";
/// 检查请求全局超时 (启动后一次性后台调用, 不阻塞 UI)。
#[cfg(not(feature = "store"))]
const FETCH_TIMEOUT_SECS: u64 = 10;

/// GitHub 轨: 查 releases/latest 的 tag_name; 网络/解析任何失败返回 None。
#[cfg(not(feature = "store"))]
fn fetch_update_status() -> Option<UpdateStatus> {
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
    }
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(FETCH_TIMEOUT_SECS)))
        .build();
    // GitHub API 无 User-Agent 直接 403。
    let release: Release = ureq::Agent::new_with_config(config)
        .get(RELEASES_API)
        .header("User-Agent", "danqing-pomodoro")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    Some(UpdateStatus::KnownVersion(release.tag_name))
}

/// 商店轨: 查 StoreContext 应用更新 (有/无更新; 失败/无包身份 → None 静默)。
#[cfg(feature = "store")]
fn fetch_update_status() -> Option<UpdateStatus> {
    store::check_update()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_version ---

    #[cfg(not(feature = "store"))]
    #[test]
    fn current_version_uses_cargo_pkg_version_on_github_track() {
        assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
    }

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
            checked_version: "0.2.0".to_string(),
            status: UpdateStatus::UpToDate,
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
            checked_version: "0.2.0".to_string(),
            status: UpdateStatus::KnownVersion("v0.2.1".to_string()),
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

    // --- 缓存版本闸门 (换版作废) ---

    #[test]
    fn usable_cache_drops_cache_from_other_binary_version() {
        let cache = CheckCache {
            checked_at_secs: 1,
            checked_version: "0.2.0".to_string(),
            status: UpdateStatus::UnknownVersion,
        };
        assert_eq!(
            usable_cache(Some(cache.clone()), "0.2.0"),
            Some(cache.clone())
        ); // 同版保留
        assert_eq!(usable_cache(Some(cache), "0.2.7"), None); // 换版作废 (商店更新后场景)
        assert_eq!(usable_cache(None, "0.2.7"), None);
    }

    #[test]
    fn load_cache_tolerates_legacy_format_without_version() {
        // checked_version 引入前的旧格式 (只存在于未发布的侧载测试机):
        // 反序列化失败按无缓存处理, 触发重查, 不留尸。
        let legacy = temp_cache_path("legacy");
        std::fs::write(&legacy, r#"{"checked_at_secs":1,"status":"UpToDate"}"#).expect("写旧格式");
        assert_eq!(load_cache_from(&legacy), None);
        let _ = std::fs::remove_file(&legacy);
    }

    // --- 更新提示模型 ---

    #[test]
    fn update_hint_none_when_uptodate_or_not_newer() {
        assert!(update_hint("0.2.0", None).is_none()); // 无缓存
        let up_to_date = CheckCache {
            checked_at_secs: 1,
            checked_version: "0.2.0".to_string(),
            status: UpdateStatus::UpToDate,
        };
        assert!(update_hint("0.2.0", Some(&up_to_date)).is_none());
        let older = CheckCache {
            checked_at_secs: 1,
            checked_version: "0.2.0".to_string(),
            status: UpdateStatus::KnownVersion("v0.2.0".to_string()),
        };
        assert!(update_hint("0.2.0", Some(&older)).is_none()); // 版本追平
    }

    #[test]
    fn update_hint_normalizes_known_version_display() {
        let newer = CheckCache {
            checked_at_secs: 1,
            checked_version: "0.2.0".to_string(),
            status: UpdateStatus::KnownVersion("9.9.9".to_string()), // 无 v 前缀也归一化展示
        };
        let hint = update_hint("0.2.0", Some(&newer)).expect("应有提示");
        assert_eq!(hint.status, "有新版本 v9.9.9");
        assert_eq!(hint.action, update_action_text());
    }

    #[test]
    fn update_hint_unknown_version_omits_version_number() {
        // 商店轨: 商店不暴露新版本号, 提示只显示「有新版本」(spec 成功标准 3)。
        let cache = CheckCache {
            checked_at_secs: 1,
            checked_version: "0.2.0".to_string(),
            status: UpdateStatus::UnknownVersion,
        };
        let hint = update_hint("0.2.0", Some(&cache)).expect("应有提示");
        assert_eq!(hint.status, "有新版本");
        assert_eq!(hint.action, update_action_text());
    }

    #[test]
    fn action_text_matches_track() {
        #[cfg(not(feature = "store"))]
        assert_eq!(update_action_text(), "前往下载");
        #[cfg(feature = "store")]
        assert_eq!(update_action_text(), "更新");
    }

    // --- 全局状态 → 提示合成 ---

    #[test]
    fn current_hint_reflects_published_cache() {
        publish(Some(CheckCache {
            checked_at_secs: 1,
            checked_version: "0.2.0".to_string(),
            status: UpdateStatus::KnownVersion("v99.0.0".to_string()),
        }));
        assert!(current_hint().is_some());
        publish(None);
        assert!(current_hint().is_none());
    }
}
