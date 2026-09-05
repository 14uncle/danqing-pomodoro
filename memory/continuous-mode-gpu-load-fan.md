---
name: continuous-mode-gpu-load-fan
description: pomodoro 启动风扇狂转是启动峰值非异常 — Continuous 60fps 稳态 GPU ~26% 为设计明码标价, 用户决定不降帧
metadata:
  type: project
---

启动后风扇狂转 → 数秒内回落安静。排查结论 (2026-09-04 实测, 开始菜单 release 构建):

- 启动峰值 = wgpu 管线编译 + 9 张场景图上传显存, 暂时现象, 非泄漏
- 稳态: CPU ~5% / GPU ~26%, 来自 `WindowMode::Continuous` (main.rs) 60fps 全屏场景 shader —— 设计使然
- 隐藏窗口后负载 ≈0 (隐藏态只 tick 不渲染), 管线无泄漏
- 曾讨论引擎加 30fps 档降载, **用户决定不动** (风扇已静, 负载可接受)

**Why:** 风扇噪音触发过「是否有 bug」的疑问; 实测数据证明一切符合设计, 避免日后重复排查。

**How to apply:** 再遇「风扇狂转/占用高」报告, 先区分启动峰值 vs 稳态, 再区分可见 vs 隐藏; 稳态可见 GPU 高占用是 Continuous 模式的既定代价, 除非商店用户集中投诉, 否则不动。若未来真要降载, 方向是引擎加 30fps 档 (FrameRate::Half), 不动 shader 不用 Adaptive 5fps (杀场景灵魂)。

**Related:** [[poll-control-flow-audio-crackling]] — 同属事件循环节奏引发的体感问题。
