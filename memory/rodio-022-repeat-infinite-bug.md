---
name: rodio-022-repeat-infinite-bug
description: "rodio 0.22 repeat_infinite 对 symphonia 解码器秒空无声,须自实现循环源(丹青番茄钟环境音 2026-07-28 踩坑)"
metadata: 
  node_type: memory
  type: reference
  originSessionId: 53d044d9-b638-4b95-b73f-d4da3d741a88
  modified: 2026-07-28T03:28:39.672Z
---

rodio 0.22.2 的 `Source::repeat_infinite()` 对 symphonia 解码器**无声秒空**:内部走 `buffered()`,`extract()` 建缓冲时读到 symphonia 解码器初始空包 `current_span_len() == Some(0)`,误判为流结束 → 追加后队列立即空、`get_pos()` 恒 0ns。不带 repeat 直接 append 同一 Decoder 则播放位置正常推进——A/B 对照是定位关键。

**Why:** rodio `Source` 文档明确 `Some(0)` 仅应在无更多数据时返回,symphonia 解码器出生即 `Some(0)` 违反契约(上游 bug);症状是"完全无声 + 无任何报错",日志一切正常,极易误判为设备/音量问题。

**How to apply:** 在 rodio 0.22 上需要循环播放环境音时,不要用 `repeat_infinite`,自实现 `Iterator + Source` 循环源:耗尽时**重开文件从头解码**回卷(不要用 `try_seek`——symphonia 粗粒度 seek 回 0 会跳过首个 Vorbis 音频包,mountain 实测少 1156 采样 ≈24ms,每次循环接缝爆音),回卷后仍空则永久关闭防音频线程空转;`current_span_len()`/`total_duration()` 报 `None`(无限流)。参考实现本仓库 `src/ambient.rs` 的 `LoopingDecoder`(源自 danqing 仓库 examples/pomodoro/ambient.rs,commit 690497e 在 danqing 历史),回归测试 `looping_decoder_restart_is_sample_accurate`(第二遍与第一遍逐位一致)。**已证伪并回滚的方案:整段解码入内存 + 游标回绕**(661b16e,08189da revert)——回卷 I/O 欠载理论不成立(内存化后小 beep 依旧,残余 artifact 更可能是资产接缝在安静风声下的可闻染色),~53MB 内存代价换不到收益,用户接受重开方案的小 pop。诊断手法:无头 spike 打 `player.get_pos()/empty()` 每秒快照定位秒空;"对齐搜索"找第 2 遍相对第 1 遍的样本偏移 δ 定位 seek 偏差;ffmpeg 解 f32 raw 后查接缝跳变/瞬断点排除资产本身。另注意 rodio 0.22 相对 0.21 全面改名(`DeviceSinkBuilder::open_default_sink()`、`Player::connect_new(&Mixer)`、`MixerDeviceSink`)。
