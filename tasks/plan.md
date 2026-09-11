# Plan: 双轨应用内更新感知 (update-check)

- **日期**: 2026-09-05
- **Spec**: [docs/specs/update-check.md](../docs/specs/update-check.md) (已评审)
- **状态**: 待评审

## 组件与依赖顺序

```
T1 纯逻辑核心 (update.rs 骨架: 版本比较 / 缓存 / 行模型 + 单测)
   │
   ├─→ T2 「版本」行显示当前版本号 (store 轨读包版本, 含回退)
   │
   └─→ T3 GitHub 轨: ureq 查 releases/latest + 缓存接线 + 行变「有新版本」+ 点击跳发布页
       │
       └─→ T4 设置按钮角标 (Stack 叠加主题色圆点)

T5 商店轨: StoreContext 查更新 + 应用内拉更新 (依赖 T2 的包版本读取; 与 T3/T4 同文件故串行)

T6 双 feature 门禁 + 文档收尾
```

T1 → T2/T3 是硬依赖（行模型先行）；T4 依赖 T3 的「有新版本」状态源；
T5 只依赖 T2，但与 T3/T4 同改 `update.rs`/`main.rs`，串行避免冲突；T6 收尾。

## 关键实现决策

1. **ureq = "3.4"**（`default-features = false, features = ["rustls", "json"]`，已查 docs.rs 核实
   feature 名）。普通依赖非 optional：store 构建也会编它但 cfg 不调用——比「反向 feature」简单，
   编译成本可忽略。
2. **GitHub API 必须带 User-Agent 头**（无 UA 直接 403——本次查 crates.io API 复现了同款）。
   URL: `https://api.github.com/repos/14uncle/danqing-pomodoro/releases/latest`，取 `tag_name`。
3. **后台→UI 通信复用购买链路模式**：检查线程写全局原子/锁状态，tick 轮询入 app 字段；
   线程不 join，进程退出即终止（无害）。
4. **商店轨版本号**：`Package.Current.Id.Version`；`cargo run --features store` 无包身份时
   回退 `CARGO_PKG_VERSION` + 一行 warn（正好覆盖「非 MSIX 环境」全体，检查同样静默跳过）。
5. **商店轨拉更新**：`GetAppAndOptionalStorePackageUpdatesAsync` 查 →
   `RequestDownloadAndInstallStorePackageUpdatesAsync` 拉（系统对话框接管进度/重启）。
   `IInitializeWithWindow` 挂属主 + 主窗口定位：从 `license::store` 提取 `pub(crate)` helper 复用，
   不复制第三份。
6. **角标**：`danqing::widget::Stack` 已存在（已核实 re-export），圆点叠加不占布局。

## 风险与缓解

| # | 风险 | 缓解 |
|---|------|------|
| R1 | StoreContext 更新 API 行为只能 MSIX 侧载实测 | 复用 docs/ms-store-copy.md 侧载检查单；IAP 同款坑已有 memory |
| R2 | ureq 3.x API 生疏写错 | build 前查 docs.rs 官方文档 (source-driven-development)，不凭记忆 |
| R3 | 无包身份环境跑 store 构建崩溃 | 包版本读取失败回退 + warn；StoreContext 获取失败静默跳过 (约束 4) |
| R4 | release panic="abort"，后台线程 panic 杀全进程 | 检查/更新线程全程 Result，无 unwrap/expect (IAP 链路同纪律) |
| R5 | 角标挤压设置按钮布局 | Stack 叠加不参与主轴布局；沿用既有布局回归测试模式 |

## 验证检查点

- **T1 后**: `cargo test` 全绿（纯逻辑护网先成型）
- **T3 后**: GitHub 轨端到端手动验证（真实 API 查一次 + 缓存注入伪造新版看提示 + 点击跳页）
  —— 这是 GitHub 用户可见价值的最薄切片
- **T5 后**: MSIX 侧载低于商店在售版本，验证提示 + 拉起系统更新 UI
- **T6**: 默认与 `--features store` 两组合 `cargo clippy -- -D warnings` + `cargo test` 全绿
