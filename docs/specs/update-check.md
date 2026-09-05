# Spec: 双轨应用内更新感知 (update-check)

- **日期**: 2026-09-05
- **意图来源**: [docs/intent/update-check.md](../intent/update-check.md) (interview-me 已确认, 含角标修订)
- **状态**: spec 已评审 (2026-09-05), 进 plan 阶段

## Objective

两条分发轨各自获得应用内更新感知, 用户不离开应用就知道「我在哪个版本、有没有新版」。

- **商店轨** (`store` feature 编译的 MSIX): 启动静默查 `StoreContext`, 有新版时设置面板提示,
  点击在应用内拉起商店系统更新 UI (下载/安装/重启流程由系统接管)。
- **GitHub 轨** (默认编译的便携包): 启动静默查 GitHub Releases API, 有新版时设置面板提示,
  点击 `open::that` 跳发布页手动下载。

用户故事:

1. 作为商店付费用户, 我在设置面板看到当前版本号; 有新版时设置按钮出现角标,
   点「更新」直接走商店更新, 永远不会被导向 GitHub 免费版。
2. 作为 GitHub 便携包用户, 我在设置面板看到当前版本号; 有新版时设置按钮出现角标,
   点「前往下载」打开发布页。
3. 作为任何用户, 无新版/断网/检查失败时, 界面与现在完全一致, 我不受任何打扰。

## Tech Stack

- Rust 1.85+ / edition 2024, 工具链 stable-x86_64-pc-windows-gnu (仓库 override)
- danqing UI 框架 (git 依赖, 本地 [patch] 联动), Theme token 取色, 不自造颜色
- **新增依赖 (唯一)**: `ureq` (default-features = false, rustls) — 仅非 store 编译启用,
  全应用第一个网络依赖, 意图文档第 5 条约束已批准
- **windows crate 新增 feature**: `ApplicationModel` (读包版本; 0.61 起 feature 名不带 Windows 前缀) — 仅 store 编译
- 已有可复用依赖: `serde`/`serde_json` (解析 API 响应 + 缓存文件), `open` (跳发布页),
  `dirs` (缓存目录), `chrono` (24h 缓存判定)

## Commands

```powershell
# 默认轨 (GitHub 便携包) 开发闭环
cargo build
cargo test
cargo clippy -- -D warnings
cargo clippy --features store -- -D warnings   # 商店轨代码必须过编译+lint

# 商店轨手动验证 (MSIX 侧载, 与 IAP 同边界)
powershell -NoProfile -File tools/build_msix.ps1 -Version 0.2.2
```

## Project Structure (增量)

```
src/
├── update.rs        ← 新增: 更新检查 (纯逻辑 + 两轨 cfg 后端)
├── license.rs       ← 复用其 mod store 模式 (StoreContext + IInitializeWithWindow 挂属主)
├── main.rs          ← 设置面板「版本」行扩展 + 设置按钮角标 + 启动时触发检查
└── state.rs         ← 不动; 缓存路径复用 dirs::config_dir()/danqing/

%APPDATA%/danqing/
├── pomodoro.json        ← 不动 (现有应用状态)
└── update-check.json    ← 新增: { 上次检查时间, 最新版本/有更新标志 }
```

`update.rs` 内部结构:

| 部分 | 说明 | 可测性 |
|------|------|--------|
| 版本号解析/比较 | `v0.2.1` → `(0,2,1)` 三元组, 纯函数 | 单测 |
| 缓存读写 | update-check.json 读写 + 24h 新鲜度判定, 纯逻辑 | 单测 |
| 行展示模型 | 「版本」行文案/按钮/角标可见性, 纯函数 | 单测 |
| GitHub 后端 `#[cfg(not(feature="store"))]` | ureq GET releases/latest → tag_name | 手动 |
| 商店后端 `#[cfg(feature="store")]` | StoreContext 查/拉更新 | 手动 (MSIX 侧载) |

## Code Style

与仓库现状一致: 中文注释、文件头 `//! @author 十四叔` + `//! @date`、魔法数字提 const、
纯逻辑与 UI 分离、字段带 doc 注释。轨道隔离用编译期 cfg, 与 license.rs 同款:

```rust
/// 当前版本号: GitHub 轨取编译期包版本, 商店轨取 MSIX 包身份版本
/// (build_msix.ps1 的 -Version 独立于 Cargo.toml, 二进制自报不可信)。
pub fn current_version() -> String {
    #[cfg(not(feature = "store"))]
    {
        env!("CARGO_PKG_VERSION").to_string()
    }
    #[cfg(feature = "store")]
    {
        store::package_version()
    }
}
```

## Testing Strategy

- 单测 (`cargo test`): 版本比较边界 (相等/maj/min/patch 各维、带不带 v 前缀、非法串)、
  24h 缓存新鲜度、缓存文件缺失/损坏回退、行展示模型全分支
- 编译门禁: 默认/`store` 两个 feature 组合都过 build + clippy (cfg 分支防腐烂)
- 手动验证:
  - GitHub 轨: 本地跑默认构建, 临时把比较基准调低一档确认提示出现 + 点击跳发布页;
  - 商店轨: 侧载一个低于商店在售版本的 MSIX, 确认提示出现 + 点击拉起系统更新 UI
    (检查单沿用 docs/ms-store-copy.md 的侧载流程)

## Boundaries

- **Always**: 提交前 `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test` 全绿
  (两个 feature 组合); 检查失败/断网一律静默; 后台线程全程 Result 无 panic
  (release panic="abort", 见 memory/store-iap-windows-crate-pitfalls)
- **Ask first**: 新增任何依赖 (ureq 已批, 其余都要问); 改 windows crate feature 列表
  (`Windows_ApplicationModel` 已批, 其余要问); 改持久化文件内容结构
- **Never**: 改 `pomodoro.json` / `focus-history.json` 现有字段; 商店轨代码路径出现
  GitHub URL (反之亦然); 任何形式的弹窗/横幅打断专注; 自造颜色绕过 Theme token

## Success Criteria

1. 设置面板「版本」行显示版本号: GitHub 轨 `v0.2.0 · 免费版` 样式; 商店轨显示包版本
2. GitHub 轨: 远端 release 更新时, 版本行变「有新版本 vX.Y.Z」+「前往下载」按钮,
   设置按钮出现主题色角标; 点击打开 releases/latest 页面
3. 商店轨: StoreContext 报告有更新时, 版本行变「有新版本」+「更新」按钮, 角标同上;
   点击拉起系统更新 UI
4. 无新版/断网/检查超时/响应解析失败 → 界面零变化, 日志一行 warn
5. 24h 内重复启动不发网络请求/商店查询 (读缓存); 检查失败不写缓存
6. 默认与 `store` 两个 feature 组合: `cargo test` 全绿 + `cargo clippy -- -D warnings` 零警告

## Open Questions

1. 角标不做「忽略此版本/已读」状态 —— 每次启动按最新检查结果亮灭。接受?
2. 商店轨系统更新 UI 装完会提示重启应用, 应用内不做额外处理 (系统对话框已覆盖)。认可?
3. ureq 版本选型 (2.x vs 3.x API 差异) 留到 plan 阶段查官方文档定, 不阻塞 spec。
