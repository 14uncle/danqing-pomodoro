# Tasks: update-check

> 依据 tasks/plan.md；按依赖序执行，完成一个勾一个。
> 每个任务提交前： `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test`。

- [x] Task 1: update.rs 纯逻辑核心
  - 内容: 新建 `src/update.rs`（文件头规范）；版本号解析/比较（`v0.2.1`→三元组，容忍有无 v 前缀、
    拒绝非法串）；`CheckCache` serde 结构（上次检查 wall_secs + 最新版本号）读写
    `%APPDATA%/danqing/update-check.json` + 24h 新鲜度判定 + 缺失/损坏回退；
    「版本」行展示模型纯函数（无新版/有新版 → 文案 + 按钮文案 + 角标可见性）
  - Acceptance: 上述全部纯函数有单测覆盖（含比较各维度、缓存损坏回退、新鲜度边界）
  - Verify: `cargo test` 全绿
  - Files: src/update.rs, src/main.rs (仅 `mod update;`)

- [x] Task 2: 「版本」行显示当前版本号
  - 内容: `update::current_version()`（非 store = `CARGO_PKG_VERSION`；store = 包版本，
    无包身份回退 + warn）；`license::VersionRow.status` 由 `&'static str` 改 `String`，
    文案变 `vX.Y.Z · 免费版` / `vX.Y.Z · 完整版 ✓`；windows crate 加 `Windows_ApplicationModel`
  - Acceptance: 版本行显示真实版本号；既有 version_row 单测同步更新且全绿
  - Verify: `cargo test` + `cargo run --release` 眼看版本行
  - Files: src/update.rs, src/license.rs, src/main.rs, Cargo.toml

- [x] Task 3: GitHub 轨更新检查端到端
  - 内容: 加 ureq 依赖；`fetch_latest()` 后端（UA 头、超时、全程 Result）；启动时后台线程
    检查（24h 缓存新鲜则跳过）；结果经全局状态进 app 字段；版本行变「有新版本 vX.Y.Z」+
    「前往下载」按钮；点击 `open::that(releases/latest)`；检查失败静默 + 一行 warn
  - Acceptance: 伪造缓存（latest_version=99.0.0）→ 行与按钮出现；真实运行打一次 API 成功；
    断网运行无界面变化
  - Verify: `cargo test` + 手动三条 Acceptance 各过一遍
  - Files: Cargo.toml, src/update.rs, src/main.rs

- [x] Task 4: 设置按钮角标
  - 内容: `Stack` 叠加主题色 accent 圆点于「设置」幽灵按钮右上角；仅「有新版本」时可见；
    版本追平后消失
  - Acceptance: 有新版时角标出现且不挤压按钮布局（沿用布局回归测试模式加一条断言）
  - Verify: `cargo test` + 手动眼看
  - Files: src/main.rs

- [x] Task 5: 商店轨 StoreContext 查/拉更新
  - 内容: license::store 提取 `pub(crate)` 窗口定位/属主绑定 helper；update.rs 商店后端：
    查更新（结果写全局状态，行模型复用 T1），点击「更新」拉起
    `RequestDownloadAndInstallStorePackageUpdatesAsync`；无包身份/调用失败全程静默
  - Acceptance: 侧载低于商店在售版本的 MSIX → 提示出现；点击拉起系统更新 UI；
    `cargo run --features store`（无包身份）不崩且静默
  - Verify: `cargo clippy --features store -- -D warnings` + MSIX 侧载手动实测
    (沿用 docs/ms-store-copy.md 检查单)
  - Files: src/update.rs, src/license.rs, src/main.rs

- [x] Task 6: 门禁 + 收尾
  - 内容: 双 feature 组合 fmt/clippy/test 全绿；spec 状态更新为已落地；MEMORY.md 索引行
    从「待进 spec」改为已落地
  - Acceptance: 两组合全绿；文档与代码一致
  - Verify: `cargo clippy -- -D warnings` && `cargo clippy --features store -- -D warnings`
    && `cargo test`
  - Files: docs/specs/update-check.md, MEMORY.md
