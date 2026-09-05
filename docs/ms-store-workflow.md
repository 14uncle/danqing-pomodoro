# 微软商店上架工作流 —— 丹青-番茄钟

> 2026-09-03 首次实测走完一次完整提交而成稿。范围: 父应用 (丹青-番茄钟 MSIX) 提报 + 内购 add-on (`danqing-pomodoro-full`) 的提交顺序。
> 配套: `docs/ms-store-copy.md` (listing 文案/检查单/付款税务指引) + `docs/privacy-policy.md`(隐私策略, **待落盘**)。

## 0. 流程总览

```
账户就绪(双绿) → 打 MSIX → 建内购 add-on
        ↓
父应用提报: [定价和可用性] [属性] [年龄分级] [程序包] [Store 一览] [提交选项] → 提交认证 → 发布
        ↓ (硬顺序: 必须先发布)
add-on 提报: [定价] [Store 一览] [提交选项] → 提交认证 → 发布
```

两条铁律:
1. **硬顺序**: add-on 只能在父应用**发布之后**才能提交。窗口期商店版「解锁完整版」显示「购买未完成·重试」属预期, add-on 上线即自愈。
2. **免费+内购双轨 (同一条边界, 见 `docs/ms-store-channel-policy.md`)**: GitHub Release 挂免费二进制 (2 场景); 商店渠道 2 场景免费 + IAP 解锁 (全部 9 场景/统计/年度报告/导出); 完整版 = 商店买断 或 `cargo build --release --features full`。

---

## 1. 前置条件 (提报前必须全绿)

| 项 | 状态 | 说明 |
|----|------|------|
| Partner Center 账户 | ✅ | 用微软账户注册 |
| 付款资料 | ✅ | 电子银行转账 (建行 CNAPS+卡号), 当天过; 详见 ms-store-copy.md 文末 |
| 税务 W-8BEN | ✅ | 个人 + 中美协定 10% (Tax Treaty Status=True) |
| MSIX 包 | ✅ | `danqing-pomodoro-store-v0.2.0-x64.msix`, 用真实产品标识重打 |
| add-on 已建 | ✅ | Offer ID `danqing-pomodoro-full`, Store ID 9P4B2MPB8HNN, ¥18 + 首发 ¥7.9 |

---

## 2. 父应用提报: 各标签页 (Partner Center -> 应用和游戏 -> 丹青-番茄钟)

这些标签是**并列的状态块**, 可任意顺序填, 提交选项填毕即进入认证。

### 2.1 定价和可用性

| 字段 | 填法 | 备注 |
|------|------|------|
| 基础价格 | **0** = 免费 | ⚠️ 下拉没有显式「免费」项, 选 0 即免费 |
| 免费试用 | **无免费试用** | 免费应用无试用概念 |
| 定价 | 免费 + 应用内购买 | 内购在 add-on 里配, 不在此处 |

> 应用内购买 (IAP) 不在父应用的定价页配 —— 它是独立的 add-on 产品, 见 §4。

### 2.2 属性

| 字段 | 填法 | 备注 |
|------|------|------|
| 类别 | 生产率 (Productivity) | |
| 是否访问/收集/传输个人信息 | **是** (被强制) | ⚠️ runFullTrust 全信任桌面应用会被强制成「是」, 保存后回来它会自己弹回「是」, 并强制要求隐私政策 |
| 隐私策略 | **「提供隐私策略文本」直接粘贴** | 比「提供 URL」干净: 免托管/commit/push。⚠️ 目前文本只在 Partner Center 字段里, 未落盘仓库 |
| 支持信息 | 联系邮箱 + https://github.com/14uncle/danqing-pomodoro/issues | 必填 |
| 发布日期 | 尽早 | |

> **隐私策略为什么弹回「是」**: 桌面全信任 (runFullTrust) 应用视为能访问系统资源, 商店强制问「是否收集个人信息」并要求隐私政策。别在「是/否」上硬扛, 直接走「提供隐私策略文本」。

### 2.3 年龄分级 (IARC 问卷)

| 问题 | 答案 | 备注 |
|------|------|------|
| 该应用是否允许用户购买数字商品? | **是** | 有应用内购买 |
| 是否包含现金奖励/礼品卡/可兑换加密资产/NFT 发行? | **否** | |

结果 (自动生成): IARC 3+ / Store 3+ / ESRB Everyone / PEGI 3+ / USK 0 / Russia 0+ —— 全部标「应用内购买」。

### 2.4 程序包 (MSIX)

| 字段 | 填法 | 备注 |
|------|------|------|
| MSIX 包 | 选 **v0.2.0** 新包 | ⚠️ 别选旧版 v0.2.1 (会是 stale)。产物在 `../release-archives/pomodoro/msix/` |
| 身份 | 14uncle / CN=5F2A7EA5-3366-4B8A-8C0D-3BE22575711A / x64 | 用 build_msix.ps1 重打时回填 |
| 最低系统 | Windows 10 17763+ | |

### 2.5 Store 一览 (listing)

来自 `docs/ms-store-copy.md`。

| 字段 | 内容 | 备注 |
|------|------|------|
| 产品名称 | 丹青-番茄钟 | |
| 简介 | 「9 个沉浸场景 × 动效 × 环境音……轻量秒启, 本地存储。」一句 | |
| 详细说明 | 长文案 (markdown) | 从 ms-store-copy.md |
| 关键词 | **恰好 7 个**: 番茄钟, 专注, 效率, 白噪音, 环境音, 沉浸, 放松 | ⚠️ 见踩坑 7 关键词报错 |
| 截图 | ≥1 张, 1366×768+, PNG/JPG, ≤5MB | 上传 5 张 |
| 系统要求 | Windows 10 17763+ / Windows 11 | |

> **截图必须用完整版**: 自由版 (free) 的统计/报告被 IAP 拦截, 点开会拉起浏览器/store。要截统计/报告面板, 先跑 `cargo run --release --features full` 进完整版。

### 2.6 提交选项

| 字段 | 填法 | 备注 |
|------|------|------|
| 完整信任说明 | 简述为什么需要 runFullTrust (桌面自绘 UI、全键盘/全局快捷键、本地文件读写) | |
| 发布时机 | **产品在通过认证后立即开始发布** | 满足「父应用先发布」顺序 |

---

## 3. 提交认证

状态栏: `提交 → 预处理 → 认证 → 发布` (一个橙色「提交认证」徽标)。

- 提交后进入「认证」阶段, 一般 3 个工作日内, 实际常更快。
- 认证通过后自动发布 (因为我们把发布时机设为「立即」)。
- **认证期间不可再改已提交内容**; 若要改, 需撤回来提新版本。

---

## 4. 内购 add-on 提报 (`danqing-pomodoro-full`)

> ⚠️ **硬顺序**: 这一步必须等父应用**完全发布**后才能做。

| 环节 | 内容 |
|------|------|
| Offer ID | `danqing-pomodoro-full` |
| 类型 | 持久型 (durable), 买断制 |
| 价格 | ¥18 + 首发促销 sale ¥7.9 (Partner Center sale pricing 可设时段) |
| Store ID | 9P4B2MPB8HNN |
| 提报入口 | Partner Center -> 你的 app -> 加载项 (add-ons) -> 该 add-on -> Store 一览 |

只需填 add-on 的定价 + Store 一览, 提报即认证; 无需再走父应用的年龄分级/程序包。

---

## 5. 上线后回填校验

| 项 | 动作 |
|----|------|
| `license.rs` STORE_URL | 已含 Store ID 9P3W6W1SR6DS; 上架后确认链接跳转到商店页 |
| 购买对话框 | 商店版点「解锁完整版」应拉起购买对话框 / 取消复位。发布前侧载已验证统计/报告统一走 IAP、fail-open 免崩溃; 对话框需 add-on 进目录后复验 |
| 双轨确认 | GitHub Release 挂免费二进制 (2 场景, 与商店免费层同构), **不再挂 full 免费 ZIP**; 完整版 = 商店买断 或 `cargo build --release --features full`。口径见 `docs/ms-store-channel-policy.md` |

---

## 6. 踩坑记录 (本次实测汇总)

1. **MSIX 上传成旧版 v0.2.1**: 提交列表里会显示 stale。务必核对包版本 = 你发布的产品版本。
2. **runFullTrust 强制隐私**: 全信任桌面应用被强制「是否收集个人信息=是」+ 必填隐私政策。别再在「是/否」上较劲, 直接「提供隐私策略文本」。
3. **基础价格下拉无「免费」项**: 选 0 即是免费。
4. **7 个关键词仍报「最多 7 个」**: 通常有**幽灵第 8 个** (未提交的空 chip 或残留输入)。全清空, 每词逐个 Enter 提交, 恰好 7 个。
5. **自由版统计/报告点开是浏览器**: 这是**正确的** IAP 拦截行为, 不是 bug。要截图得用完整版 build。
6. **隐私策略弹回「是」+ 又提示填 URL**: 选「否」保存后回来会弹回「是」。走「提供隐私策略文本」即可解。
7. **(付款阶段) 地址 Line 3 被判 P.O. Box**: 必须真实家庭住址纯拼音, 无信箱/「盒 Box」/单位 c/o 字样。
8. **(税务) Tax Treaty Status=False**: 漏勾「申请税收协定优惠」节。回 W-8BEN 勾 China→10% + 补 Foreign TIN。
9. **误建草稿**: 别用「新建应用」建重复预留; 在已有产品下填写。若误建需删草稿。
10. **add-on 图标卡 300×300**: add-on Store 一览图标下限 300×300, 仓库原有最大 256px 被拦。按 `pomodoro.svg` 几何 4× 超采样重渲出 `pomodoro_300.png` (Pillow, 无 SVG 渲染器依赖)。
11. **任务栏图标蓝底板 (2026-09-04, 最大坑)**: 商店版任务栏图标被垫 Windows 默认蓝底。排查走遍 BackgroundColor(transparent/#1A0F0A)、DefaultTile/SplashScreen 有无、targetsize/scale 资产家族、清图标缓存、PNG 脏透明像素——**全都无效**。真正根因: **MSIX 包缺 `resources.pri`**(MakePri 生成)。shell 靠它做「限定资源解析」才知道图标有 scale/targetsize/altform-unplated 变体可挑; 缺了它只能拿基础 `Square44x44Logo.png` 垫 BackgroundColor 底板。修复: `build_msix.ps1` 打包前跑 `makepri createconfig + makepri new` 生成 `resources.pri` + `resources.scale-*.pri` 并打进包 (对照 ScreenToGif / rufus `packme.cmd` 均含此文件且裸图标)。⚠️ 侧载测试时同版本号重装不刷新图标缓存, 每次验证必须 bump 版本号。
12. **商店页「支持」= mailto 死路 (2026-09-05)**: 「属性→支持信息」同时填了邮箱+URL 时, 商店页发行商信息的「支持」优先用 **mailto: 邮箱**。无默认邮件客户端的机器 (国内大多数) 点开是空白页/空白浏览器 (网页商店新页签 mailto: 空内容; 桌面商店拉起空浏览器)。对策: 只留 URL (GitHub issues), 删掉邮箱 —— 改属性是纯元数据提交不用新包, 但要等进行中的提交认证完才能改。

---

## 7. 提报输入清单 (提前备好)

- [x] MSIX 包 (v0.2.0) + 校验 (.sha256)
- [x] 付款 (CNAPS + 卡号) + 税务 (W-8BEN 身份证 + 协定 10%)
- [x] add-on Offer ID `danqing-pomodoro-full`
- [x] listing 文案 + 5 张截图 (完整版截)
- [x] 隐私策略文本 (⚠️ 备份一份进 `docs/privacy-policy.md`)
- [x] 支持信息邮箱 + issues 链接
- [x] IARC 问卷答案 (数字商品=是)

---

## 8. 更新记录

- 2026-09-03: 首次成稿。父应用 v0.2.0 进入「提交认证」, add-on 待父应用发布后提交。
- 2026-09-04: 父应用认证通过并发布 (商店页已上线, 约 1 天)。add-on `danqing-pomodoro-full` 同日提交进入「提交认证」 (定价 ¥18+首发 ¥7.9 / Store 一览标题+说明+300×300 图标)。发布后接 §5 回填校验。
- 2026-09-04: 修复任务栏图标蓝底板 (踩坑 §6-11: 缺 `resources.pri`)。`build_msix.ps1` 加 MakePri 步骤, `gen_store_assets.py` 加 scale 家族 + 干净透明清理 (BG_COLOR=(0,0,0,0) + alpha=0 像素 RGB 归零)。产物 `v0.2.7` 侧载实测任务栏裸图标 ✓。**父应用需重提 v0.2.7** (v0.2.0 已发布的是蓝底板版)。
