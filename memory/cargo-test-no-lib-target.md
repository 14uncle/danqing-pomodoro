# 测试口令: 裸 cargo test (本仓无 lib target)

pomodoro 是纯二进制 crate (只 src/main.rs 一个 bin target)，`cargo test --lib --tests`
直接报 `error: no library targets found in package danqing-pomodoro`。
三件套测试口令 = 裸 `cargo test`（2026-09-09 实测 184 绿）。

**Why:** danqing 仓门禁是 `cargo test --lib --tests`，跨仓惯性带过来会踩空；
该错误还不含 "test result" 字样，被 grep 管道整个吞掉过一次，排查绕弯。

**How to apply:** 本仓三件套 = `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test`。
另：Bash 工具 cwd 跨调用复位不规律（有时粘住有时弹回 danqing），跑本仓命令
每条链显式 `cd /f/github/farm01/danqing-pomodoro &&` 并用 pwd 核对，
靠测试数量签名（本仓 184 / danqing 458+）能识别跑错仓。
