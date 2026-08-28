---
name: poll-control-flow-audio-crackling
description: ControlFlow::Poll causes audio crackling when window hidden - high-frequency tick hammers rodio player
metadata: 
  node_type: memory
  type: feedback
  originSessionId: 8865e39c-f058-48eb-9afc-8ad01c7c0df7
  modified: 2026-08-04T05:51:25.528Z
---

When window is hidden, using `ControlFlow::Poll` causes event loop to spin at thousands of fps. Each iteration calls `app.tick()` → `ambient_player.apply()` → `player.play()/set_volume()` at thousands of Hz, causing audio buffer underruns and crackling.

**Why:** Poll = no wait, event loop runs as fast as possible. Hidden apps don't get RedrawRequested, so Poll was used to drive tick. But tick rate becomes unbounded.

**How to apply:** Always use `WaitUntil(16ms)` regardless of visibility. Hidden apps still need tick (timer, persistence, audio), but at ~60fps is sufficient. The previous `Poll` approach was documented as "驱动 app.tick" but the side effect of hammering audio was not anticipated.

**Related:** [[danqing-visual-debug-tooling]](在 danqing 仓库 memory/) — window visibility issues often have non-obvious side effects on background threads.
