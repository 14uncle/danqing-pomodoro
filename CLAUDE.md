# Project: 丹青番茄钟 (danqing-pomodoro)

专注陪伴的沉浸世界 —— Rust 桌面番茄钟应用，9 个场景 × shader 动效 × 环境音。

## Tech Stack

- **语言**: Rust 1.85+, edition 2024 (工具链 stable-x86_64-pc-windows-gnu)
- **UI 框架**: [danqing](../danqing) — 自研 Rust 跨平台自绘 UI (winit 0.30 + wgpu)
- **音频**: rodio 0.22 (symphonia-ogg/vorbis)
- **平台**: 当前只测试 Windows;`windows-sys` 用于全局快捷键/调试

## Commands

- 构建：`cargo build`
- 运行：`cargo run --release` (debug 下 GPU 渲染较慢)
- 测试：`cargo test`
- 静态检查：`cargo clippy -- -D warnings`
- 打包：`powershell -NoProfile -File tools/package_portable.ps1 -BinaryName danqing-pomodoro -IcoPath "assets\logo\pomodoro.ico"`

## Code Conventions

- **注释和文档一律使用中文**,与现有代码保持一致
- 文件头带 `//! @author` / `//! @date` 注释 (见 `src/main.rs`)
- 魔法数字提取为 `const`,并附中文 doc 注释说明用途 (见 `main.rs` 顶部的 `FLASH_DURATION` 等)
- UI 组件从 `danqing::widget` 导入 (`Box as UiBox` — 避免与 `std::boxed::Box` 冲突)
- 计时逻辑 (`timer.rs`)、状态 (`state.rs`)、统计 (`stats.rs`) 是纯逻辑模块，与 UI 分离
- 运行时资产通过**相对路径**加载 — 可执行文件旁边必须有 `assets/` 目录

## Boundaries

- `Cargo.toml` 中 `danqing = { path = "../danqing" }` 是本地路径依赖 — 修改前先确认用户意图 (开发期保持 path 依赖，发布才换 git 依赖)
- 不要提交 `target/`、`.idea/` 或任何生成的打包产物
- 数据文件格式 (`focus-history.json`) 变更会影响用户已有数据 — 修改前需要说明迁移方案
- `build.rs` 仅 Windows 嵌入图标;跨平台改动需谨慎 (见 DEVELOPMENT.md 常见问题)
- 改动涉及场景 shader 动效时，确认 `assets/background/` 噪声纹理的引用不受影响

## Patterns

结构字段注释示例 (来自 `src/main.rs`):

```rust
/// 番茄钟应用状态。
struct PomodoroApp {
    /// 计时状态机 (纯逻辑)。
    timer: Pomodoro,
    /// 注入时间轴：自应用启动的累计时间 (由 tick 心跳推进)。
    now: Duration,
}
```

时长/常量定义示例:

```rust
/// 场景交叉淡化时长 (spec: 600~1000ms)。
const FADE_DURATION: Duration = Duration::from_millis(800);
```

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
├── close_button.rs / log_helper.rs / starfield.rs
```

## References

- 产品说明：[README.md](README.md)
- 依赖机制/构建/打包: [DEVELOPMENT.md](DEVELOPMENT.md)
- danqing 框架仓库：`../danqing` (本地路径依赖)
