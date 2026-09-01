//! @author 十四叔
//! @date 2026/08/30

//! 版本授权: 免费版 vs 完整版。
//!
//! 运行时检查逻辑:
//! - 编译时 `full` feature → 始终为完整版
//! - 否则启动时检查微软商店内购, 已购则解锁
//! - 未购/检查失败 → 免费版 (2 场景, 无统计/报告)

#[cfg(not(feature = "full"))]
use std::sync::atomic::{AtomicBool, Ordering};

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
// 微软商店 IAP 检查
// ---------------------------------------------------------------------------

#[cfg(all(not(feature = "full"), feature = "store"))]
mod store {
    use windows::Services::Store::StoreContext;

    /// 完整版内购项的 Offer ID。
    /// 待办: 在 Partner Center 创建内购 add-on 时, Offer ID 必须与此值一致。
    const FULL_VERSION_OFFER_ID: &str = "danqing-pomodoro-full";

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
}
