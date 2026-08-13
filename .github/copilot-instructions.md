# 丹青番茄钟 (danqing-pomodoro) — GitHub Copilot Instructions

## Tech Stack
- Rust 1.85+, edition 2024 (工具链 stable-x86_64-pc-windows-gnu)
- UI 框架: danqing (winit 0.30 + wgpu) — git 依赖 (Cargo.lock 钉 rev), 本机经用户级 paths override 透明使用本地 `../danqing`
- 音频: rodio 0.22 (symphonia-ogg/vorbis)
- 平台: 当前只测试 Windows

## Commands
- 构建: `cargo build`
- 运行: `cargo run --release`
- 测试: `cargo test`
- 静态检查: `cargo clippy -- -D warnings`

## Code Conventions
- 注释和文档一律使用中文，与现有代码保持一致
- 文件头带 `//! @author` / `//! @date` 注释
- 魔法数字提取为 `const`，并附中文 doc 注释说明用途
- UI 组件从 `danqing::widget` 导入 (`Box as UiBox` 避免与 `std::boxed::Box` 冲突)
- 纯逻辑模块 (timer.rs, motion.rs, stats.rs) 时间由外部注入，不读 wall-clock，确保可单元测试
- 数据文件格式 (`focus-history.json`) 变更会影响用户已有数据，修改前需说明迁移方案

## Patterns
- 结构字段使用 `///` doc 注释说明用途 (见 `src/main.rs` 的 `PomodoroApp`)
- 常量使用 `const` + 中文 doc 注释 (见 `FLASH_DURATION`, `FADE_DURATION`)
- 场景定义在 `src/scenes.rs`，顺序即切换顺序
- 动效强度合成: 每个场景有独立的 `xxx_intensity(from, to, fade, envelope)` 函数
- 雨是例外 — 暂停时定格可见，强度不含包络

## Boundaries
- `Cargo.toml` 中 danqing 依赖提交状态固定为 git 依赖; 本地联动开发临时切 path 不提交 (CI 单 checkout 会拦, 详见 DEVELOPMENT.md「依赖机制」)
- 不要提交 `target/`、`.idea/` 或任何生成的打包产物
- 运行时资产通过相对路径加载 — 可执行文件旁边必须有 `assets/` 目录
- 改动涉及场景 shader 动效时，确认 `assets/background/` 噪声纹理的引用不受影响

## Source Layout
```
src/
├── main.rs       ← 入口 + PomodoroApp (UI 组装、事件循环)
├── timer.rs      ← 番茄钟状态机 (纯逻辑)
├── state.rs      ← 应用状态持久化 (focus-history.json)
├── stats.rs      ← 专注统计/年度报告
├── scenes.rs     ← 9 个场景定义 (调色板/动效参数)
├── motion.rs     ← shader 动效策略
├── ambient.rs    ← 环境音程序化生成
├── audio.rs      ← rodio 播放/交叉淡化
├── fader.rs      ← 场景切换交叉淡化
├── flash.rs      ← 完成反馈脉冲
├── hint.rs       ← 快捷键提示浮层
├── tray.rs       ← 系统托盘菜单
├── today.rs      ← 今日统计面板
```
