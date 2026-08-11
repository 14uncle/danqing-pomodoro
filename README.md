# 丹青番茄钟

专注陪伴的沉浸世界 —— 9 个手绘场景 × 动效 × 环境音，让专注不再是忍耐。

![场景预览](assets/scenes/bonfire.png)

## 功能

- **9 个沉浸场景**: 篝火、海、雨、山、森林、铁匠铺、洞穴、夜市、火车
- **场景动效**: 每个场景独立 shader 动画 (火焰呼吸、海浪、雨丝、金属反光...)
- **环境音**: 程序化生成的场景音效，交叉淡化切换
- **专注计时**: 25/5/15 经典番茄钟，支持自定义时长
- **数据统计**: 今日/本周/累计专注时长,年度报告深度洞察
- **数据导出**: CSV/JSON 明文导出，本地存储，你的数据你做主

## 下载

从 [Releases](https://github.com/GANWEIHUN/danqing-pomodoro/releases) 下载最新版本。

解压后双击 `danqing-pomodoro.exe` 即可运行。

## 构建

需要 Rust 1.85+ 和 [danqing](https://github.com/GANWEIHUN/danqing) 框架。

```bash
# 克隆
git clone --recursive https://github.com/GANWEIHUN/danqing-pomodoro.git
cd danqing-pomodoro

# 构建
cargo build --release

# 运行
cargo run --release
```

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Space` | 开始/暂停 |
| `◀` / `▶` | 切换场景 |
| `Esc` | 显示/隐藏窗口 |
| `Ctrl+Q` | 退出 |

## 数据存储

专注记录保存在：
- **Windows**: `%APPDATA%/danqing/focus-history.json`
- **macOS**: `~/Library/Application Support/danqing/focus-history.json`
- **Linux**: `~/.config/danqing/focus-history.json`

## 反馈

遇到问题或有建议？请到 [Issues](https://github.com/GANWEIHUN/danqing-pomodoro/issues) 提交。

## 开发

详见 [DEVELOPMENT.md](DEVELOPMENT.md) —— 依赖机制、构建流程、打包指南。

## 技术栈

- **UI 框架**: [danqing](https://github.com/GANWEIHUN/danqing) — Rust 跨平台自绘 UI
- **渲染**: wgpu (D3D12/Vulkan/Metal)
- **窗口**: winit 0.30
- **字体**: fontdue + font-kit

## 许可证

MIT OR Apache-2.0
