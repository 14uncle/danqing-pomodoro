# Implementation Plan: 渠道政策落地 —— GitHub 切到 free 二进制

## Overview

把 2026-09-03 确认的渠道政策（开源 + 免费 + freemium，开箱二进制 = 2 场景）落到 **GitHub 发布** 与 **README 口径**：v0.2.0 release 挂 **free** 二进制（不再有任何免费完整版 ZIP 后门）；完整版 = 商店买断 或 `cargo build --release --features full`。这是纯发布/文档改动，无代码逻辑改动。

## 现状 (2026-09-03 只读核查)

- **GitHub 唯一 release = v0.1.1**（2026-08-12，名称「增加反馈入口」），资产 = `danqing-pomodoro-v0.1.0-win-x64.zip`（2 下载，**旧全功能构建**——那时还没有 free/full/store 特性）。**没有任何 v0.2.0 release**。
- 本地 `release-archives/pomodoro/` 已有 free / store / full 三个 v0.2.0 ZIP + sha256。
- git HEAD = `489923e`；09-02 三处购买修复已提交（`64d27b1`）。工作区仅 docs 改动（`MEMORY.md`、`docs/ms-store-copy.md` 已改，两个新 doc）。**代码干净，无需改码**。
- `tools/build_freemium.ps1` 一条命令产出 free（`--features` 空）/ store（`--features store`）/ full（`--features full`）三包。

## 架构决策

| 决策 | 选择 | 理由 |
|------|------|------|
| GitHub v0.2.0 挂哪个包 | **free** zip (`danqing-pomodoro-free-v0.2.0-win-x64.zip`) + sha256 | 政策规定开箱 = 2 场景 |
| 是否发 store/full 到 GitHub | **都不发** | store 的 IAP 需商店身份，独立 exe 无身份校验不过；full 正是政策要关掉的 |
| 旧 v0.1.1 release | **保留不动**（Q1 待用户拍板） | 历史产物、2 下载、明确是旧版；发 v0.2.0 为 Latest 后它即非默认 |
| 发布时机 | 现在发 v0.2.0（Q2 待用户拍板） | 政策已锁、一两分钟 gh 命令、与商店认证解耦 |
| 完整版获取 | 商店买断 或 自行编译 | 与政策措辞一致 |

## 任务清单

按依赖顺序，详见 `tasks/todo.md`：

1. 重编并校验 free v0.2.0 包（保证与 HEAD `489923e` 一致）
2. 发布 v0.2.0 GitHub release（挂 free zip + sha256，设为 Latest，body 写明口径）
3. README 增「免费 vs 完整版」口径 + 下载段
4. 核对 `ms-store-copy.md` 等口径与 policy 一致（应无需改）

**Checkpoint（Task 2 后）：** `gh release view` 确认 v0.2.0 挂 free、为 Latest；v0.1.1 按 Q1 处理结果保留或已清。

## 开放问题 (拍板后我才能决定任务细节)

- **Q1** 旧 v0.1.1 的 v0.1.0 全功能资产怎么处置？—— (a) 保留原样（推荐：历史归档，2 下载）；还是 (b) 删除该资产，彻底关掉后门。
- **Q2** v0.2.0 现在就发到 GitHub，还是等商店认证通过、两边同步发？—— 推荐现在发。
- **Q3** free zip 是否重编保证与 HEAD 一致？—— 推荐重编（`build_freemium.ps1`，便宜且可验证 2 场景）。

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 旧 v0.1.1 仍挂全功能 zip | 低（2 下载、历史版） | Q1 保留；发 v0.2.0 为 Latest 即非默认 |
| free zip 与 HEAD 不一致 | 中（发错产物） | Task 1 重编 + sha256 + 手动验 2 场景 |
| Release body 口径不清 | 低 | 发布前 pre-write body，声明免费版/完整版途径 |
| 误发 store/full 包 | 高（政策回退） | 只传 free；gh release view 复核资产清单 |

## 非目标

不改商店 freemium 定价 / 不动桌景 / 不启新 POC / 不改 `focus-history.json` / 不删改旧文章（产品仍开源免费）。
