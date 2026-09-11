# 丹青番茄钟

专注陪伴的沉浸世界 —— 9 个场景 × 动效 × 环境音，让专注不再是忍耐。

![场景预览](assets/scenes/bonfire.png)

## 功能

**免费版已含：** 篝火、海 2 个沉浸场景（独立 shader 动效 + 真实环境音）、经典番茄钟计时、全局快捷键。

**完整版额外解锁**（微软商店买断 或 自行编译 `--features full`）：

- 其余 7 个沉浸场景：雨、山、森林、铁匠铺、洞穴、夜市、火车
- 专注数据统计（今日/本周/累计）、年度报告
- CSV / JSON 明文导出，本地存储，你的数据你做主

## 下载

从 [Releases](https://github.com/14uncle/danqing-pomodoro/releases) 下载**免费版**（开源、免费，2 场景）。解压后双击 `danqing-pomodoro.exe` 即可运行。

**完整版**（全部 9 场景 + 数据统计 + 年度报告 + 导出）二选一：

- **微软商店买断**：[丹青-番茄钟](https://www.microsoft.com/store/apps/9P3W6W1SR6DS)
- **自行编译**：`cargo build --release --features full`

## 构建

需要 Rust 1.85+。[danqing](https://github.com/14uncle/danqing) 框架经 git 依赖自动拉取，无需手动安装。

```bash
# 克隆
git clone https://github.com/14uncle/danqing-pomodoro.git
cd danqing-pomodoro

# 构建
cargo build --release

# 运行
cargo run --release
```

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+S` | 开始/暂停 |
| `Ctrl+Shift+P` | 显示/隐藏窗口 |
| `Ctrl+Shift+Q` | 退出 |

## 数据存储

专注记录保存在：
- **Windows**: `%APPDATA%/danqing/focus-history.json`
- **macOS**: `~/Library/Application Support/danqing/focus-history.json`
- **Linux**: `~/.config/danqing/focus-history.json`

## 反馈

遇到问题或有建议？请到 [Issues](https://github.com/14uncle/danqing-pomodoro/issues) 提交。

## 开发

详见 [DEVELOPMENT.md](DEVELOPMENT.md) —— 依赖机制、构建流程、打包指南。

## 技术栈

- **UI 框架**: [danqing](https://github.com/14uncle/danqing) — Rust 跨平台自绘 UI
- **渲染**: wgpu (D3D12/Vulkan/Metal)
- **窗口**: winit 0.30
- **字体**: fontdue + font-kit

## 许可证

MIT OR Apache-2.0
