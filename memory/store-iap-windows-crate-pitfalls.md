# Memory: 商店 IAP 购买链路的 windows crate 坑

> 2026-09-01 code-review 揪出 (commit b1abb77 修复), 购买代码仅 MSIX 环境可达, 单测/clippy 全盖不住

## 坑

- **windows crate 把「返回 NULL 句柄」的 Win32 调用包装成 Err**:
  `GetWindow(hwnd, GW_OWNER)` 对无属主顶层窗口返回 NULL → 包装器
  (`is_invalid()` = null || -1) 转成 `Err`。判「无属主」必须 `.is_err()`,
  写 `.map(|h| h.0.is_null()).unwrap_or(false)` 就是逻辑反转 — 真实代码里
  这个反转让 find_main_window 恒 None, 购买必败。**教训: 用 windows crate
  包装过的句柄返回值时, 先想「NULL 到哪去了」。**
- **RequestPurchaseAsync 的文档化入参不是开发者自取的 offer token**:
  文档路径 = `GetAssociatedStoreProductsAsync(["Durable"])` → 按
  `InAppOfferToken` 匹配 add-on → `StoreProduct.RequestPurchaseAsync()`
  (内部用 Partner Center 分配的 Store ID)。`check_license` 匹配
  `InAppOfferToken` 是对的, 购买直接传 token 不是文档承诺的路径。
- **IInitializeWithWindow 必须先挂属主** (桌面进程显示 UI 的 Store 调用约定):
  `StoreContext::cast::<IInitializeWithWindow>()?.Initialize(hwnd)`;
  hwnd 用 EnumWindows 按 PID + 可见 + 无属主 + 有标题过滤 (排除托盘/热键隐藏窗口)。
- **IIterable 传参**: `vec![HSTRING::from("Durable")].into()` 得 `IIterable<HSTRING>`,
  调用时传 `&kinds` (Param<InterfaceType> 由 &U 实现, 按值不行);
  windows crate 不 re-export windows_collections, 要命名类型就得加直接依赖。
- **BOOL 在 `windows::core`**, 不在 `windows::Win32::Foundation` (0.61)。
- **release profile panic="abort"**: catch_unwind 只在 dev 构建生效;
  后台购买线程的可靠性靠「全程 Result、无 panic 路径」纪律, 注释别吹过头。

## 验证边界

购买链路 (对话框拉起/取消/失败) 只能 MSIX 侧载实测 — 前提: Partner Center
建好 add-on (Offer ID `danqing-pomodoro-full`)。检查单在 docs/ms-store-copy.md。
