//! @author 十四叔
//! @date 2026/08/30

//! 版本授权: 免费版 vs 完整版。
//!
//! 运行时检查逻辑:
//! - 编译时 `full` feature → 始终为完整版
//! - 否则启动时检查微软商店内购, 已购则解锁
//! - 未购/检查失败 → 免费版 (2 场景, 无统计/报告)

#[cfg(not(feature = "full"))]
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// 运行时完整版状态 (仅在非 `full` 编译时使用)。
#[cfg(not(feature = "full"))]
static FULL_VERSION: AtomicBool = AtomicBool::new(false);

/// 初始化授权状态。
///
/// 应在 `main()` 最早处调用。
/// - `full` feature 启用时: 无操作 (编译时常量已为 true)
/// - `store` feature 启用时: 检查微软商店内购许可证
/// - 其他: 保持免费版
pub fn init() {
    #[cfg(feature = "full")]
    {
        // 编译时已确定为完整版, 无需运行时检查
    }

    #[cfg(all(not(feature = "full"), feature = "store"))]
    {
        if store::check_license() {
            FULL_VERSION.store(true, Ordering::Relaxed);
            log::info!("Store license active: full version unlocked");
        } else {
            log::info!("No store license: free version");
        }
    }

    #[cfg(not(any(feature = "full", feature = "store")))]
    {
        // 免费版, 无需操作
    }
}

/// 是否为完整版。
pub fn is_full() -> bool {
    #[cfg(feature = "full")]
    {
        true
    }
    #[cfg(not(feature = "full"))]
    {
        FULL_VERSION.load(Ordering::Relaxed)
    }
}

/// 免费版可用的场景数量。
pub const FREE_SCENE_COUNT: usize = 2;

/// 微软商店内购链接 (用于免费版升级引导; 应用上架后生效)。
pub const STORE_URL: &str = "https://www.microsoft.com/store/apps/9P3W6W1SR6DS";

/// 检查指定场景索引是否可用。
#[expect(dead_code)]
pub fn is_scene_available(index: usize) -> bool {
    is_full() || index < FREE_SCENE_COUNT
}

/// 检查统计功能是否可用。
pub fn stats_available() -> bool {
    is_full()
}

/// 检查报告功能是否可用。
pub fn report_available() -> bool {
    is_full()
}

// ---------------------------------------------------------------------------
// 购买流程状态 (设置面板「版本」行 + 应用内购买)
// ---------------------------------------------------------------------------

/// 购买状态码: 空闲 (未发起 / 取消后复位)。
#[cfg(not(feature = "full"))]
const PURCHASE_IDLE: u8 = 0;
/// 购买状态码: 商店购买对话框进行中 (防重入: 期间隐藏升级按钮)。
#[cfg(not(feature = "full"))]
const PURCHASE_PURCHASING: u8 = 1;
/// 购买状态码: 上次购买失败 (网络/服务错误), 可重试。
#[cfg(not(feature = "full"))]
const PURCHASE_FAILED: u8 = 2;

/// 购买流程状态。仅后台购买线程与测试写入; UI 每帧经 [`purchase_state`] 读。
#[cfg(not(feature = "full"))]
static PURCHASE_STATE: AtomicU8 = AtomicU8::new(PURCHASE_IDLE);

/// 购买流程状态 (设置面板「版本」行据此展示)。仅非 `full` 构建存在 (完整版无购买流程)。
#[cfg(not(feature = "full"))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PurchaseState {
    /// 空闲 (未发起或已复位)。
    Idle,
    /// 商店购买对话框进行中。
    Purchasing,
    /// 上次购买失败, 可重试。
    Failed,
}

/// 当前购买状态 (仅非 `full` 构建存在)。
#[cfg(not(feature = "full"))]
pub fn purchase_state() -> PurchaseState {
    match PURCHASE_STATE.load(Ordering::Relaxed) {
        PURCHASE_PURCHASING => PurchaseState::Purchasing,
        PURCHASE_FAILED => PurchaseState::Failed,
        _ => PurchaseState::Idle,
    }
}

/// 设置面板「版本」行的展示模型 (纯函数, 与授权/购买状态一一对应)。
pub struct VersionRow {
    /// 状态文案。
    pub status: &'static str,
    /// 操作按钮文案; None = 不显示按钮 (完整版 / 购买中)。
    pub action: Option<&'static str>,
}

/// 计算「版本」行展示模型。
///
/// 购买成功后 [`is_full`] 立即变 true, 行随之变「完整版 ✓」— 这本身就是购买反馈。
pub fn version_row() -> VersionRow {
    #[cfg(feature = "full")]
    {
        VersionRow {
            status: "完整版 ✓",
            action: None,
        }
    }
    #[cfg(not(feature = "full"))]
    {
        // 商店版运行时购买成功后 is_full() 翻 true; 走到这里的是免费态
        if is_full() {
            return VersionRow {
                status: "完整版 ✓",
                action: None,
            };
        }
        match purchase_state() {
            PurchaseState::Idle => VersionRow {
                status: "免费版",
                action: Some("解锁完整版"),
            },
            PurchaseState::Purchasing => VersionRow {
                status: "购买中…",
                action: None,
            },
            PurchaseState::Failed => VersionRow {
                status: "购买未完成",
                action: Some("重试"),
            },
        }
    }
}

/// 发起完整版购买。
///
/// - `full` 构建: 编译期已是完整版, 无操作
/// - 商店版 (`store`): 已购无操作; 未购时 MSIX 环境由后台线程拉起商店购买对话框,
///   成功即写 `FULL_VERSION` 解锁 (无需重启), 取消复位, 失败置「购买未完成」
/// - 其他 (便携免费版): 打开商店网页引导 (与统计/报告的升级引导一致)
pub fn purchase_full_version() {
    #[cfg(feature = "full")]
    {
        // 编译期已是完整版
    }
    #[cfg(all(not(feature = "full"), feature = "store"))]
    {
        if !is_full() {
            store::purchase_full_version();
        }
    }
    #[cfg(not(any(feature = "full", feature = "store")))]
    {
        let _ = open::that(STORE_URL);
    }
}

// ---------------------------------------------------------------------------
// 微软商店 IAP 检查
// ---------------------------------------------------------------------------

#[cfg(all(not(feature = "full"), feature = "store"))]
mod store {
    use std::sync::atomic::Ordering;
    use windows::Services::Store::StoreContext;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::Shell::IInitializeWithWindow;
    use windows::core::BOOL;

    /// 完整版内购项的 Offer ID。
    /// 待办: 在 Partner Center 创建内购 add-on 时, Offer ID 必须与此值一致。
    const FULL_VERSION_OFFER_ID: &str = "danqing-pomodoro-full";

    /// 购买结果 (仅模块内部使用)。
    enum PurchaseOutcome {
        /// 购买成功或此前已购。
        Purchased,
        /// 用户取消 (StorePurchaseStatus::NotPurchased)。
        Cancelled,
        /// 网络/服务/API 错误, 可重试。
        Failed,
    }

    /// 拉起商店购买对话框 (立即返回, 后台线程跑购买, 结果写回授权/购买状态)。
    pub fn purchase_full_version() {
        if !is_running_as_msix() {
            // 非 MSIX (开发运行): 商店 API 不可用, 退化到网页引导
            log::info!("Not MSIX, opening store page for upgrade");
            let _ = open::that(super::STORE_URL);
            return;
        }
        // 防重入: 仅空闲 → 购买中的跃迁成功者有权发起
        if super::PURCHASE_STATE
            .compare_exchange(
                super::PURCHASE_IDLE,
                super::PURCHASE_PURCHASING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return;
        }
        std::thread::spawn(|| {
            // catch_unwind 兜底: 任何意外 panic 不得把状态卡死在「购买中」
            let outcome =
                std::panic::catch_unwind(purchase_blocking).unwrap_or(PurchaseOutcome::Failed);
            match outcome {
                PurchaseOutcome::Purchased => {
                    super::FULL_VERSION.store(true, Ordering::Release);
                    super::PURCHASE_STATE.store(super::PURCHASE_IDLE, Ordering::Release);
                    log::info!("Store purchase succeeded: full version unlocked");
                }
                PurchaseOutcome::Cancelled => {
                    super::PURCHASE_STATE.store(super::PURCHASE_IDLE, Ordering::Release);
                    log::info!("Store purchase cancelled by user");
                }
                PurchaseOutcome::Failed => {
                    super::PURCHASE_STATE.store(super::PURCHASE_FAILED, Ordering::Release);
                    log::warn!("Store purchase failed (see above), retry available");
                }
            }
        });
    }

    /// 同步执行商店购买 (调用方负责线程与状态管理)。
    fn purchase_blocking() -> PurchaseOutcome {
        use windows::Services::Store::StorePurchaseStatus;
        use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
        use windows::core::{HSTRING, Interface};

        // WinRT 异步调用需 COM 单元; 已初始化会返回错误, 忽略即可
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };

        let context = match StoreContext::GetDefault() {
            Ok(ctx) => ctx,
            Err(e) => {
                log::warn!("StoreContext::GetDefault failed: {e}");
                return PurchaseOutcome::Failed;
            }
        };

        // 桌面进程必须把购买对话框的属主设为我们的主窗口, 否则显示 UI 的
        // 调用直接失败 (IInitializeWithWindow 约定)。
        let Some(hwnd) = find_main_window() else {
            log::warn!("main window not found, cannot own purchase dialog");
            return PurchaseOutcome::Failed;
        };
        match context.cast::<IInitializeWithWindow>() {
            Ok(init) => {
                if let Err(e) = unsafe { init.Initialize(hwnd) } {
                    log::warn!("IInitializeWithWindow::Initialize failed: {e}");
                    return PurchaseOutcome::Failed;
                }
            }
            Err(e) => {
                log::warn!("cast to IInitializeWithWindow failed: {e}");
                return PurchaseOutcome::Failed;
            }
        }

        let op = match context.RequestPurchaseAsync(&HSTRING::from(FULL_VERSION_OFFER_ID)) {
            Ok(op) => op,
            Err(e) => {
                log::warn!("RequestPurchaseAsync failed: {e}");
                return PurchaseOutcome::Failed;
            }
        };
        match op.get() {
            Ok(result) => match result.Status() {
                Ok(StorePurchaseStatus::Succeeded | StorePurchaseStatus::AlreadyPurchased) => {
                    PurchaseOutcome::Purchased
                }
                Ok(StorePurchaseStatus::NotPurchased) => PurchaseOutcome::Cancelled,
                Ok(status) => {
                    let ext = result.ExtendedError().map(|h| h.0).unwrap_or(0);
                    log::warn!("purchase status={} ext=0x{ext:08X}", status.0);
                    PurchaseOutcome::Failed
                }
                Err(e) => {
                    log::warn!("purchase Status() failed: {e}");
                    PurchaseOutcome::Failed
                }
            },
            Err(e) => {
                log::warn!("purchase async wait failed: {e}");
                PurchaseOutcome::Failed
            }
        }
    }

    /// 找本进程主窗口句柄 (购买对话框属主)。
    ///
    /// 过滤条件: 属于本进程 + 可见 + 无属主 (顶层) + 有标题,
    /// 以排除托盘/全局热键创建的隐藏消息窗口。
    fn find_main_window() -> Option<HWND> {
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GW_OWNER, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId,
            IsWindowVisible,
        };

        struct Ctx {
            pid: u32,
            found: HWND,
        }
        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
            let top_level = unsafe { GetWindow(hwnd, GW_OWNER) }
                .map(|h| h.0.is_null())
                .unwrap_or(false);
            if pid == ctx.pid && visible && top_level && unsafe { GetWindowTextLengthW(hwnd) } > 0 {
                ctx.found = hwnd;
                return BOOL(0); // 找到即停止枚举
            }
            BOOL(1)
        }

        let mut ctx = Ctx {
            pid: std::process::id(),
            found: HWND::default(),
        };
        let _ = unsafe { EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut Ctx as isize)) };
        if ctx.found.0.is_null() {
            None
        } else {
            Some(ctx.found)
        }
    }

    /// 检查微软商店内购许可证是否有效。
    ///
    /// 返回 `true` 表示已购买完整版内购项。
    /// 任何错误 (非 MSIX 环境/网络问题/未购买) 均返回 `false`。
    pub fn check_license() -> bool {
        // 1. 检查是否在 MSIX 环境中运行
        if !is_running_as_msix() {
            log::debug!("Not running as MSIX, skipping store check");
            return false;
        }

        // 2. 获取 StoreContext
        let context = match StoreContext::GetDefault() {
            Ok(ctx) => ctx,
            Err(e) => {
                log::warn!("StoreContext::GetDefault failed: {e}");
                return false;
            }
        };

        // 3. 查 add-on 级内购许可证。
        //    注意: StoreAppLicense::IsActive 是应用级许可证, 免费上架时所有
        //    安装者都为 true, 不能用它判断内购 — 必须遍历 AddOnLicenses,
        //    匹配我们内购项的 Offer ID 且 IsActive。
        match context.GetAppLicenseAsync() {
            Ok(op) => match op.get() {
                Ok(license) => match license.AddOnLicenses() {
                    Ok(add_ons) => {
                        let owned = (&add_ons).into_iter().any(|pair| match pair.Value() {
                            Ok(lic) => {
                                let active = lic.IsActive().unwrap_or(false);
                                let token = lic
                                    .InAppOfferToken()
                                    .map(|t| t.to_string_lossy())
                                    .unwrap_or_default();
                                active && token == FULL_VERSION_OFFER_ID
                            }
                            Err(_) => false,
                        });
                        log::info!("Store add-on license check: owned={owned}");
                        owned
                    }
                    Err(e) => {
                        log::warn!("AddOnLicenses failed: {e}");
                        false
                    }
                },
                Err(e) => {
                    log::warn!("GetAppLicense async failed: {e}");
                    false
                }
            },
            Err(e) => {
                log::warn!("GetAppLicenseAsync failed: {e}");
                false
            }
        }
    }

    /// 检查是否以 MSIX 包形式运行。
    fn is_running_as_msix() -> bool {
        // MSIX 环境下 Windows 会设置此环境变量
        std::env::var("PackageName").is_ok()
            // 备选: 检查包根目录是否存在 AppxBlockMap.xml
            || std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("AppxBlockMap.xml").exists()))
                .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_version_limits_scenes() {
        if !is_full() {
            assert!(is_scene_available(0), "场景 0 应可用");
            assert!(is_scene_available(1), "场景 1 应可用");
            assert!(!is_scene_available(2), "场景 2 应锁定");
            assert!(!is_scene_available(8), "场景 8 应锁定");
        }
    }

    #[test]
    fn full_version_all_scenes_available() {
        if is_full() {
            for i in 0..9 {
                assert!(is_scene_available(i), "完整版场景 {i} 应可用");
            }
        }
    }

    #[test]
    fn feature_gate_consistency() {
        if !is_full() {
            assert!(!stats_available());
            assert!(!report_available());
        }
        if is_full() {
            assert!(stats_available());
            assert!(report_available());
        }
    }

    #[test]
    fn init_does_not_panic() {
        init();
    }

    // === 版本行展示模型 (设置面板付费入口, 2026-09-01) ===

    /// 免费版: 状态文案 + 升级按钮; 购买中隐藏按钮防重入; 失败可重试。
    /// (唯一读写 PURCHASE_STATE 的测试, 串行执行完并复位, 避免并行污染。)
    #[cfg(not(feature = "full"))]
    #[test]
    fn version_row_tracks_purchase_state() {
        // 空闲: 免费版 + 解锁入口
        PURCHASE_STATE.store(PURCHASE_IDLE, Ordering::Relaxed);
        let row = version_row();
        assert_eq!(row.status, "免费版");
        assert_eq!(row.action, Some("解锁完整版"));

        // 购买中: 按钮消失 (防重入), 只留状态文案
        PURCHASE_STATE.store(PURCHASE_PURCHASING, Ordering::Relaxed);
        let row = version_row();
        assert_eq!(row.status, "购买中…");
        assert_eq!(row.action, None);

        // 失败: 提示 + 重试入口
        PURCHASE_STATE.store(PURCHASE_FAILED, Ordering::Relaxed);
        let row = version_row();
        assert_eq!(row.status, "购买未完成");
        assert_eq!(row.action, Some("重试"));

        // 复位: 回到初始形态
        PURCHASE_STATE.store(PURCHASE_IDLE, Ordering::Relaxed);
        assert_eq!(version_row().action, Some("解锁完整版"));
    }

    /// 完整版: 恒为「完整版 ✓」且无操作按钮。
    #[cfg(feature = "full")]
    #[test]
    fn version_row_full_version() {
        let row = version_row();
        assert_eq!(row.status, "完整版 ✓");
        assert_eq!(row.action, None);
    }
}
