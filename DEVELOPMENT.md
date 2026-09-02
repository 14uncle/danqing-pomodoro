# 开发指南

## 项目结构

```
danqing-pomodoro/
├── Cargo.toml          ← 依赖声明(danqing 框架 + 第三方库)
├── build.rs            ← 构建脚本(嵌入 Windows 图标)
├── src/                ← 源码
│   ├── main.rs         ← 入口
│   ├── scenes.rs       ← 场景定义
│   ├── ambient.rs      ← 环境音
│   ├── motion.rs       ← 动效策略
│   ├── stats.rs        ← 数据统计
│   └── ...
├── assets/             ← 运行时资产(场景图/环境音/字体/图标)
│   ├── scenes/         ← 9 个场景 PNG
│   ├── audio/          ← 9 个环境音 OGG
│   ├── fonts/          ← 回退字体
│   ├── logo/           ← 应用图标
│   └── background/     ← 噪声纹理(shader 用)
└── tools/              ← 打包脚本
    ├── package_portable.ps1  ← Windows 便携包打包
    └── patch_icon.py         ← 图标注入
```

## 依赖机制

本项目通过 `Cargo.toml` 声明对 danqing 框架的依赖。Cargo 是 Rust 的包管理器，自动处理下载、编译和链接。

### 当前：Git 依赖 (提交状态,danqing 已公开)

```toml
# Cargo.toml
[dependencies]
danqing = { git = "https://github.com/14uncle/danqing" }
```

- 仓库自洽：任何人 `git clone` 后 `cargo build` 即可构建，Cargo 自动从 GitHub 克隆 danqing
- `Cargo.lock` 钉住精确 rev:发布可复现，checkout 旧 tag + `cargo build --locked` 可重建当时的二进制
- 升级框架是有意识的决定：`cargo update -p danqing` 后提交 Cargo.lock
- CI 为单 checkout:path 依赖被误提交会因找不到 `../danqing` 立即失败 (护栏)

### 本地联动开发：临时切 path (不提交)

框架与产品同步开发时，临时把 `Cargo.toml` 改为本地路径：

```toml
danqing = { path = "../danqing" }
```

- 修改框架代码后重新编译即可生效，无需 push
- **完事改回 git 依赖再提交**;误提交会被 CI 护栏拦住
- 也可在用户级 `~/.cargo/config.toml` 配置 `paths = ["F:/github/farm01/danqing"]` path override(对本机所有项目生效,Cargo.lock 不受影响);danqing 新增依赖时 override 会报错，此时用临时切换

### 未来:crates.io 版本依赖 (框架稳定后)

```toml
[dependencies]
danqing = "0.1.0"
```

- 发布到 crates.io 后，用户只需 `cargo build`,Cargo 自动从 crates.io 下载
- 语义化版本：`0.1.0` 表示兼容的补丁更新
- 最简洁的依赖方式，但需要框架先发布到 crates.io

### 依赖解析流程

```
cargo build
    ↓
读取 Cargo.toml
    ↓
检查 danqing 依赖类型:
  - path → 直接使用本地代码
  - git  → 克隆/更新 Git 仓库到 ~/.cargo/git/
  - 版本 → 从 crates.io 下载到 ~/.cargo/registry/
    ↓
编译 danqing 框架(首次慢,后续增量)
    ↓
编译 danqing-pomodoro
    ↓
链接生成可执行文件
```

### 添加新依赖

```bash
# 添加第三方库
cargo add serde --features derive
cargo add chrono

# 添加 danqing 子模块(如果框架拆分了)
cargo add danqing-render --path ../danqing/render
```

## 构建

### 开发构建

```bash
cargo build          # 快速编译,含调试信息
cargo run            # 编译并运行
cargo test           # 运行测试
cargo clippy -- -D warnings  # 静态检查
```

### 发布构建

```bash
cargo build --release  # 优化编译(慢但二进制小)
```

Release 配置特点：
- `lto = "fat"` — 全量链接时优化，二进制更小
- `codegen-units = 1` — 单编译单元，优化更充分
- `opt-level = "z"` — 体积优先
- `strip = "debuginfo"` — 剥离调试信息

### 打包

```powershell
# Windows 便携包(含图标注入)
powershell -NoProfile -File tools/package_portable.ps1 `
  -BinaryName danqing-pomodoro `
  -IcoPath "assets\logo\pomodoro.ico"
```

输出：`../release-archives/pomodoro/danqing-pomodoro-v0.1.0-win-x64.zip`（农场共享归档；编译产物在农场根 `.cargo-target/`，见根 CLAUDE.md）

## 运行时资产

应用通过相对路径加载资产，所以**可执行文件必须在项目根目录运行**,或者 assets/ 目录在可执行文件旁边：

```
danqing-pomodoro/
├── danqing-pomodoro.exe
└── assets/
    ├── scenes/
    ├── audio/
    └── ...
```

打包脚本会自动处理这个结构。

## 工具链

- **Rust**: 1.85+ (stable-x86_64-pc-windows-gnu)
- **框架**: danqing (winit 0.30 + wgpu 30)
- **打包**: PowerShell + Python (patch_icon.py)

## 常见问题

### Q: `cargo run` 报错找不到 danqing

- 默认 git 依赖：检查网络能否访问 GitHub,或 `cargo update -p danqing` 重新拉取
- 临时 path 切换时：确认 danqing 仓库在同级目录 (`../danqing`)
- 配置了用户级 `paths` override 时报依赖图不一致：本地 danqing 有未提交/未推送的依赖变更，先提交 push 再 `cargo update -p danqing`

### Q: 图标没有嵌入 exe

`build.rs` 使用 `winresource` 嵌入图标，但 GNU 工具链可能不支持。使用打包脚本的 `patch_icon.py` 作为后备方案。

### Q: 环境音不播放

检查 `assets/audio/` 目录是否存在且包含 OGG 文件。rodio 在找不到文件时会静默降级。

### Q: 跨平台构建

当前只测试了 Windows。macOS/Linux 理论上支持 (winit + wgpu 自动选择),但需要：
- 安装对应平台的 GPU 驱动
- 可能需要调整 `build.rs`(仅 Windows 嵌入图标)
- 环境音的 `MessageBeep` 调用在非 Windows 平台会被跳过
