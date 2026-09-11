//! @author 十四叔
//! @date 2026/09/05

//! 双轨应用内更新感知: 轨道接缝在本模块, 核心已下沉框架 (2026-09-08)。
//!
//! 纯逻辑 (版本对比/24h TTL 缓存+换版作废/角标提示模型) 与 GitHub 轨运输整体委托
//! `danqing::update` (框架 `update` feature); 产品只保留:
//! - 产品身份 (repo/UA/发布页/当前版本) 经 [`UpdateSpec`] 注入;
//! - 商店轨 (MSIX): 包身份版本读取 + StoreContext 查/拉更新 —— 框架只收 GitHub 轨,
//!   store/授权/IAP 是产品政策 (见 danqing::update 模块头与 docs/specs/update-check.md);
//! - 轨道分派: [`spawn_check`] / [`perform_action`] / [`current_hint`] 的双轨出口。
//!
//! 约定不变: 任何一步解析/读写/网络/商店失败都按「无新版」静默处理, 不打扰用户。
//!
//! 缓存文件随迁移换名: `update-check.json` → `update-check-14uncle-danqing-pomodoro.json`
//! (框架按 repo 分词防多产品互覆; 旧文件无害残留, 首启重查一次即自愈)。

#[cfg(feature = "store")]
use danqing::update::{CheckCache, cache_path, load_cache_from, publish, save_cache_to};
use danqing::update::{UpdateHint, UpdateSpec};

/// 产品身份: 仓库 `owner/name` (框架据此拼 GitHub API 端点与缓存文件名)。
const REPO: &str = "14uncle/danqing-pomodoro";
/// 请求 User-Agent (GitHub API 无 UA 直接 403)。
const USER_AGENT: &str = "danqing-pomodoro";
/// 「前往下载」跳转的发布页 (GitHub 轨)。
const RELEASES_PAGE: &str = "https://github.com/14uncle/danqing-pomodoro/releases/latest";

/// 注入框架的产品身份 (每次调用现合成: 四个 &str 的 Copy 结构, 零成本)。
fn spec() -> UpdateSpec {
    UpdateSpec {
        repo: REPO,
        user_agent: USER_AGENT,
        releases_page: RELEASES_PAGE,
        current_version: current_version(),
    }
}

/// 当前版本号: GitHub 轨取编译期包版本, 商店轨取 MSIX 包身份版本
/// (build_msix.ps1 的 -Version 独立于 Cargo.toml, 二进制自报不可信;
/// 进程级 OnceLock 缓存在 store 模块内, 见 [`store::package_version`])。
pub fn current_version() -> &'static str {
    #[cfg(not(feature = "store"))]
    {
        env!("CARGO_PKG_VERSION")
    }
    #[cfg(feature = "store")]
    {
        store::package_version()
    }
}

/// 启动更新检查: 先读缓存立即发布 (换版/过期作废), 过期/缺失才后台线程重查;
/// 成功写缓存并发布, 失败静默 (一行 warn, 本次会话不重试)。
/// GitHub 轨整体委托框架; 商店轨同构流程在 [`spawn_check_store`], 仅运输不同。
pub fn spawn_check() {
    #[cfg(not(feature = "store"))]
    danqing::update::spawn_check(spec());
    #[cfg(feature = "store")]
    spawn_check_store();
}

/// 当前更新提示: 框架全局缓存 + 当前版本合成, UI 每帧调用 (Some = 设置按钮亮角标)。
/// 商店轨覆写按钮文案: 框架只产 GitHub 轨的「前往下载」, 商店轨是应用内「更新」。
pub fn current_hint() -> Option<UpdateHint> {
    let hint = danqing::update::current_hint(&spec())?;
    #[cfg(not(feature = "store"))]
    {
        Some(hint)
    }
    #[cfg(feature = "store")]
    {
        Some(UpdateHint {
            action: "更新",
            ..hint
        })
    }
}

/// 执行更新动作 (设置面板「版本」行按钮; 行为按轨道分派)。
pub fn perform_action() {
    #[cfg(not(feature = "store"))]
    {
        // GitHub 轨: 跳发布页手动下载 (自动替换 exe 不属本期范围, 见 spec)
        danqing::update::perform_action(&spec());
    }
    #[cfg(feature = "store")]
    {
        // 商店轨: 拉起系统更新 UI (下载/安装/重启提示由系统对话框接管)
        store::request_update();
    }
}

/// 商店轨启动检查: 与框架 spawn_check 同构 (缓存闸门 → 后台线程 → 发布),
/// 唯一差异是运输换成 StoreContext::GetAppAndOptionalStorePackageUpdatesAsync。
#[cfg(feature = "store")]
fn spawn_check_store() {
    // 缓存仅对写入它的二进制版本有效: 商店轨 UnknownVersion 无版本号, 无法经 is_newer
    // 自纠, 换版 (含商店自动更新) 后旧缓存必须作废重查 (框架 usable_cache 同款闸门,
    // 2026-09-05 复跑验证揪出)。
    let cached = cache_path(&spec())
        .and_then(|p| load_cache_from(&p))
        .filter(|c| c.checked_version == current_version());
    let fresh = cached
        .as_ref()
        .is_some_and(|c| c.is_fresh(crate::state::current_wall_secs()));
    publish(cached);
    if fresh {
        return;
    }
    // 后台线程不 join: 进程退出即终止, 无泄漏 (spec R4)。
    std::thread::spawn(|| match store::check_update() {
        Some(status) => {
            let cache = CheckCache {
                checked_at_secs: crate::state::current_wall_secs(),
                checked_version: current_version().to_string(),
                status,
            };
            match cache_path(&spec()) {
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

// ---------------------------------------------------------------------------
// 微软商店: 包身份版本读取 + 更新查/拉 (产品政策, 框架明确不收)
// ---------------------------------------------------------------------------

#[cfg(feature = "store")]
mod store {
    use danqing::update::UpdateStatus;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::Services::Store::{StoreContext, StorePackageUpdate};
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};

    /// 更新拉起在途标志: 系统对话框存活期间忽略重复点击 (防重入)。
    static UPDATE_REQUESTING: AtomicBool = AtomicBool::new(false);

    /// 读 MSIX 包身份版本 (Major.Minor.Build, 丢弃 build_msix.ps1 恒写 0 的 Revision);
    /// 无包身份 (`cargo run --features store` 直跑) 回退编译期版本 + warn。
    ///
    /// 进程级 OnceLock 缓存: 版本号是进程生命周期常量 (商店轨更新必须重启进程才生效),
    /// 而本函数经 current_version() 每帧被 UI 绑定多次调用 —— 不缓存会让开发形态
    /// (无包身份) 每帧重复 WinRT 失败并刷 warn。
    pub fn package_version() -> &'static str {
        static PACKAGE_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        PACKAGE_VERSION.get_or_init(package_version_uncached)
    }

    /// 实际读取 (每进程一次, 见 [`package_version`])。
    fn package_version_uncached() -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use danqing::update::{CheckCache, UpdateStatus, publish};

    /// 产品身份防手滑: repo 同时决定 GitHub API 端点与缓存文件名。
    #[test]
    fn spec_carries_product_identity() {
        let s = spec();
        assert_eq!(s.repo, "14uncle/danqing-pomodoro");
        assert_eq!(s.user_agent, "danqing-pomodoro");
        assert_eq!(
            s.releases_page,
            "https://github.com/14uncle/danqing-pomodoro/releases/latest"
        );
    }

    #[cfg(not(feature = "store"))]
    #[test]
    fn current_version_uses_cargo_pkg_version_on_github_track() {
        assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
    }

    /// GitHub 轨全委托框架: 发布到框架全局缓存的结果经本模块出口可见 (纯逻辑测试在框架侧)。
    #[cfg(not(feature = "store"))]
    #[test]
    fn github_track_hint_flows_from_framework_cache() {
        publish(Some(CheckCache {
            checked_at_secs: 1,
            checked_version: current_version().to_string(),
            status: UpdateStatus::KnownVersion("v99.0.0".to_string()),
        }));
        let hint = current_hint().expect("应有提示");
        assert_eq!(hint.status, "有新版本 v99.0.0");
        assert_eq!(hint.action, "前往下载");
        publish(None);
        assert!(current_hint().is_none());
    }

    /// 商店轨: 按钮文案覆写为「更新」(框架只产「前往下载」), 版本号不可得保持不显。
    #[cfg(feature = "store")]
    #[test]
    fn store_track_hint_action_overridden() {
        publish(Some(CheckCache {
            checked_at_secs: 1,
            checked_version: current_version().to_string(),
            status: UpdateStatus::UnknownVersion,
        }));
        let hint = current_hint().expect("应有提示");
        assert_eq!(hint.status, "有新版本");
        assert_eq!(hint.action, "更新");
        publish(None);
    }
}
