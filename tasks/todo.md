# 渠道政策落地任务 (2026-09-03)

> 依据 `docs/ms-store-channel-policy.md`。纯发布/文档改动，无代码逻辑改动。
> 完整背景见 `tasks/plan.md`。

## 拍板前置 (open questions)

- [ ] Q1 旧 v0.1.1 的 v0.1.0 全功能资产：**保留** / 删除
- [ ] Q2 v0.2.0 GitHub release：**现在就发** / 等商店认证完再发
- [ ] Q3 free zip：**重编**保证与 HEAD 一致 / 直接用现有包

## Task 1: 重编并校验 free v0.2.0 包

**描述:** 用 `tools/build_freemium.ps1` 重编 free 包，保证产物与 git HEAD (`489923e`) 一致；确认解压后 exe 只含篝火/海 2 场景（free 版正确性）。

**验收:**
- [ ] `release-archives/pomodoro/danqing-pomodoro-free-v0.2.0-win-x64.zip` + `.sha256` 生成且更新
- [ ] 手动运行 free exe，场景仅 篝火、海，统计/报告不在免费版内

**验证:**
- [ ] `powershell -NoProfile -File tools/build_freemium.ps1 -Version 0.2.0`
- [ ] 解压 free zip → 双击 exe 确认 2 场景

**依赖:** 无
**文件:** `tools/build_freemium.ps1`（不需改）
**范围:** XS

## Task 2: 发布 v0.2.0 GitHub release（挂 free）

**描述:** `gh release create v0.2.0`，上传 free zip + sha256，设 Latest；body 写明「GitHub 发布免费版（2 场景）；完整版 = 微软商店买断 或 `cargo build --release --features full`」。

**验收:**
- [ ] `gh release view v0.2.0` 资产 = free zip + sha256，**不含** full/store 包
- [ ] tag v0.2.0 标记为 Latest
- [ ] release body 写清免费/完整版边界

**验证:**
- [ ] `gh release view v0.2.0 --repo 14uncle/danqing-pomodoro`
- [ ] `gh release list` 确认 v0.2.0 为 Latest

**依赖:** Task 1
**文件:** 无（仅发布产物）
**范围:** S

## Checkpoint: 发布后

- [ ] `gh release view` 复核：free 在、full/store 不在、Latest 指向 v0.2.0
- [ ] 按 Q1 结果，v0.1.1 保留原样 或 已清资产

## Task 3: README 增「免费 vs 完整版」口径 + 下载段

**描述:** 「功能」列表把 9 场景/统计/年度报告/导出标注为完整版；「下载」段补「GitHub 发布免费版；完整版 = 商店买断 或编译」。加一句能说清免费/收费边界的话。

**验收:**
- [ ] 读者读完 README 一句话能说出「免费到哪、收费到哪」
- [ ] 功能列表与 free 二进制实际（2 场景）不矛盾

**验证:**
- [ ] 通读 README 下载 + 功能两段

**依赖:** 无
**文件:** `README.md`
**范围:** S

## Task 4: 核对 ms-store-copy.md 等口径与 policy 一致

**描述:** 核对 `docs/ms-store-copy.md`「免费版 vs 完整版」表首 = 2 vs 9（应该已一致），及 workflow §5 与 policy 无矛盾。

**验收:**
- [ ] 三份 doc（channel-policy / ms-store-copy / ms-store-workflow）口径一致

**验证:**
- [ ] 逐段对照

**依赖:** 无
**文件:** `docs/ms-store-copy.md`（预计不需改）
**范围:** XS

## 非目标

不改商店定价 / 不动桌景 / 不启 POC / 不改 `focus-history.json` / 不删改旧文章。
