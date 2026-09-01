# Memory Index

> 按任务类型分组;同一 memory 可能在多组中出现(交叉引用)。
> memory 文件放 `memory/` 目录,与下方索引一一对应。

## 发布/打包

- [MSIX 侧载测试工作流与坑](memory/msix-sideload-workflow.md) — ps1 中文必须 BOM / manifest 必须无 BOM / Msixvc 只认 LocalMachine 信任 / Cert: PSDrive 本机不可用; 2026-09-01 实测

## 音频 (rodio 环境音)

- [rodio 0.22 repeat_infinite bug](memory/rodio-022-repeat-infinite-bug.md) — symphonia 解码器循环秒空无声,须自实现 LoopingDecoder (src/ambient.rs);2026-08-27 自 danqing/memory 迁入
- [Poll 空转致环境音呲啦](memory/poll-control-flow-audio-crackling.md) — 隐藏态用 ControlFlow::Poll 致 tick 数千 fps,hammer rodio player → buffer underrun;统一 WaitUntil(16ms);2026-08-27 自 danqing/memory 迁入
