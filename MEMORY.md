# Memory Index

> 按任务类型分组;同一 memory 可能在多组中出现(交叉引用)。
> memory 文件放 `memory/` 目录,与下方索引一一对应。

## 发布/打包

- [渠道政策: 开源+免费+freemium](docs/ms-store-channel-policy.md) — 一句规则/两渠道免税表/落地动作; 取代 08-10「全功能免费」+ 09-01「GitHub 发 full 版」; 2026-09-03 interview-me 确认
- [微软商店上架工作流](docs/ms-store-workflow.md) — 父应用 + add-on 提报逐 tab 步骤 (定价0=免费 / runFullTrust 强制隐私走文本 / IARC 数字商品=是 / 硬顺序: 父应用先发布); 踩坑含 7 关键词幽灵第8、自由版统计走浏览器; 2026-09-03 首走成稿 备案在 docs/ms-store-copy.md
- [MSIX 侧载测试工作流与坑](memory/msix-sideload-workflow.md) — ps1 中文必须 BOM / manifest 必须无 BOM / Msixvc 只认 LocalMachine 信任 / Cert: PSDrive 本机不可用; 2026-09-01 实测
- [商店 IAP 购买链路的 windows crate 坑](memory/store-iap-windows-crate-pitfalls.md) — NULL 句柄被包装成 Err (is_err 判空) / RequestPurchaseAsync 走 StoreProduct / IInitializeWithWindow 挂属主; 2026-09-01 评审揪出
- [商店更新检查 API 的坑](memory/store-update-api-pitfalls.md) — StorePackageUpdate.Package 是当前包不暴露新版本号 / 刚侧载的包商店有注册延迟 / Size() 计数日志做诊断桩; 2026-09-05 侧载实测揪出
- [双轨应用内更新感知](docs/intent/update-check.md) — 商店轨 StoreContext 应用内拉更新 / GitHub 轨静默查 Releases 告知+跳发布页; store feature 编译时隔离轨道; 设置页版本行+设置按钮角标; 2026-09-05 落地并双轨实测通过 (spec: docs/specs/update-check.md)

## 性能/负载

- [Continuous 模式 GPU 负载与风扇](memory/continuous-mode-gpu-load-fan.md) — 启动风扇狂转=峰值非异常; 稳态 CPU 5%/GPU 26% 是 60fps 明码标价, 隐藏≈0; 2026-09-04 决定不降帧

## 音频 (rodio 环境音)

- [rodio 0.22 repeat_infinite bug](memory/rodio-022-repeat-infinite-bug.md) — symphonia 解码器循环秒空无声,须自实现 LoopingDecoder (src/ambient.rs);2026-08-27 自 danqing/memory 迁入
- [Poll 空转致环境音呲啦](memory/poll-control-flow-audio-crackling.md) — 隐藏态用 ControlFlow::Poll 致 tick 数千 fps,hammer rodio player → buffer underrun;统一 WaitUntil(16ms);2026-08-27 自 danqing/memory 迁入
