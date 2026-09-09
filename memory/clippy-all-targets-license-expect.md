# clippy --all-targets 炸 license.rs:65 (预存, 非回归)

`cargo clippy --all-targets -- -D warnings` 在本仓报错:
`license.rs:65 #[expect(dead_code)]` unfulfilled —— `is_scene_available` 仅被同文件
`#[cfg(test)]` 测试引用, test target 下 dead_code 不触发, expectation 落空。

**仓库门禁是 `cargo clippy -- -D warnings` (无 --all-targets), 该项为绿。**
2026-09-09 干净 HEAD 上实测同样炸 (预存问题, 与当日 anim 迁移无关)。

**Why:** 防止下次有人在 pomodoro 加 `--all-targets` 严格化时误判为「我改坏了」而误修。

**How to apply:** 日常验证用仓库惯例命令。若要根治: `#[cfg_attr(not(test), expect(dead_code))]`
或把测试挪到 tests/ —— 2026-09-09 已挂账待用户裁决, 勿顺手改 (scope 纪律)。
