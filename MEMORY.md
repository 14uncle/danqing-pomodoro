# Memory Index

> 按任务类型分组;同一 memory 可能在多组中出现(交叉引用)。
> memory 文件放 `memory/` 目录,与下方索引一一对应。

## 音频 (rodio 环境音)

- [rodio 0.22 repeat_infinite bug](memory/rodio-022-repeat-infinite-bug.md) — symphonia 解码器循环秒空无声,须自实现 LoopingDecoder (src/ambient.rs);2026-08-27 自 danqing/memory 迁入
- [Poll 空转致环境音呲啦](memory/poll-control-flow-audio-crackling.md) — 隐藏态用 ControlFlow::Poll 致 tick 数千 fps,hammer rodio player → buffer underrun;统一 WaitUntil(16ms);2026-08-27 自 danqing/memory 迁入
