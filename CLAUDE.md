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

- `Cargo.toml` 中 `danqing` 依赖提交状态固定为 **git 依赖**(danqing 已公开，仓库自洽 + Cargo.lock 钉 rev 保发布可复现);本地联动开发框架时临时切 `path = "../danqing"`,**不要提交**(CI 单 checkout 会拦);详见 DEVELOPMENT.md「依赖机制」
- 不要提交 `target/`、`.idea/` 或任何生成的打包产物
- 数据文件格式 (`focus-history.json`) 变更会影响用户已有数据 — 修改前需要说明迁移方案
- `build.rs` 仅 Windows 嵌入图标;跨平台改动需谨慎 (见 DEVELOPMENT.md 常见问题)
- 改动涉及场景 shader 动效时，确认 `assets/background/` 噪声纹理的引用不受影响

## Patterns

### 结构字段注释 (来自 `src/main.rs`)

```rust
/// 番茄钟应用状态。
struct PomodoroApp {
    /// 计时状态机 (纯逻辑)。
    timer: Pomodoro,
    /// 注入时间轴：自应用启动的累计时间 (由 tick 心跳推进)。
    now: Duration,
    /// 场景交叉淡化器 (含当前场景索引)。
    fader: SceneFader,
    /// 场景动效沉降包络 (纯逻辑：暂停 500ms 淡出 / 恢复淡入)。
    motion_envelope: motion::MotionEnvelope,
}
```

### 时长/常量定义 (来自 `src/main.rs`)

```rust
/// 场景交叉淡化时长 (spec: 600~1000ms)。
const FADE_DURATION: Duration = Duration::from_millis(800);
/// 持久化节流间隔：state_dirty 为 true 时，距上次保存超过此间隔才落盘。
const SAVE_THROTTLE: Duration = Duration::from_secs(1);
```

### 场景定义 (来自 `src/scenes.rs`)

场景由 `SceneSpec` 数组定义，顺序即 ◀/▶ 切换顺序。每个场景包含名称、图片路径、调色板:

```rust
pub const SCENES: [SceneSpec; 9] = [
    SceneSpec {
        name: "篝火",
        image: "assets/scenes/bonfire.png",
        palette: ScenePalette {
            base: Color::from_srgb8(26, 15, 10),
            accent: Color::from_srgb8(255, 159, 67),
            text_primary: Color::from_srgb8(240, 230, 215),
            // ...
        },
    },
    // ...
];
```

### 动效包络 (来自 `src/motion.rs`)

视觉动效使用 `MotionEnvelope` 做潮汐式沉降 (运行=全量, 暂停=500ms 归零)。
时间由外部注入 (`Duration` 累计值)，不读 wall-clock，可完整单元测试:

```rust
pub fn gain(&mut self, running: bool, now: Duration) -> f32 {
    let target = if running { 1.0 } else { 0.0 };
    // 目标变化触发 500ms 滑动，反向边沿从当前值续接 (无跳变)
    if target != self.last_target {
        self.anim = Some((self.value, target, now));
        self.last_target = target;
    }
    // ...
}
```

### 强度合成函数 (来自 `src/motion.rs`)

每个场景有独立的 `xxx_intensity(from, to, fade, envelope)` 函数。
雨是例外 — 暂停时定格可见，强度不含包络:

```rust
/// 火效强度合成：包络 × 篝火场景淡化权重。
pub fn fire_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(BONFIRE_SCENE, from, to, fade)
}
/// 雨例外：暂停时雨丝定格可见，强度不含包络。
pub fn rain_intensity(from: usize, to: usize, fade: f32) -> f32 {
    scene_weight(RAIN_SCENE, from, to, fade)
}
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
- 仓库记忆: [MEMORY.md](MEMORY.md) — 会话默认加载, 仓库级非显而易见知识落这里
- 商店上架 / 渠道政策（涉及发布、定价、内购、硬顺序先读这三份）: [docs/ms-store-copy.md](docs/ms-store-copy.md) listing 文案 + 提交前检查单 + 付款税务指引 · [docs/ms-store-workflow.md](docs/ms-store-workflow.md) 上架逐 tab 流程 + 踩坑 · [docs/ms-store-channel-policy.md](docs/ms-store-channel-policy.md) 开源+免费+freemium 渠道边界
