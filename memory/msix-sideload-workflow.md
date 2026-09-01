# Memory: MSIX 侧载测试工作流与坑

> 2026-09-01 实测踩出 (商店 freemium 上架链路), 工具已固化在 tools/

## 标准流程 (仓库根目录顺序执行)

1. `powershell -NoProfile -File tools/build_freemium.ps1` — 三版本 (free/store/full)
2. `powershell -NoProfile -File tools/build_msix.ps1` — MSIX (Publisher CN 已回填真实值)
3. `powershell -NoProfile -File tools/sign_msix_local.ps1` — 自签名证书 + 签名 + 安装
4. 若 0x800B0109: `tools/trust_cert_machine.ps1` 需 UAC 提权跑一次 (见下)
5. 启动: `explorer.exe shell:AppsFolder\14uncle.-_3y3rwcp1ep416!App`

## 坑 (每个都真实炸过)

- **.ps1 含中文必须存 UTF-8 BOM**: 本机 Windows PowerShell 5.1 按 GBK 读无 BOM 文件,
  中文字节会吃掉引号/括号 → 解析爆炸。Edit/Write 工具写出的是无 BOM UTF-8,
  改完 ps1 必须补 BOM (见 git 历史 005a9dc 的手法)
- **AppxManifest.xml 必须无 BOM**: 有 BOM 的包安装失败 (8/30 fix_msix.py 修的就是它);
  build_msix.ps1 已改 `UTF8Encoding($false)` 根治
- **Msixvc 部署服务以 SYSTEM 身份验签, 只认 LocalMachine\TrustedPeople**:
  CurrentUser\TrustedPeople 加了也 0x800B0109
- **本机 powershell 的 Cert: PSDrive 不可用** (Security 模块加载失败, 疑与 env 注入有关):
  New-SelfSignedCertificate 不可用, 用 .NET CertificateRequest.CreateSelfSigned 替代
- **MSIX 下 exe 旁的 logs/ 写不进** (WindowsApps 只读): 框架 log.rs 静默降级,
  商店版无日志 —— danqing 侧待办: 日志目录回退 %APPDATA%
- **PowerShell 的 X509Certificate2 构造器对 byte[] 绑定有怪癖**:
  `New-Object X509Certificate2($cert.Export(...))` 匹配不到重载; 直接 Add($cert) 即可
- **explorer.exe 启动 App 返回码恒 1**: 不要用 `&&` 串后续命令
- **侧载证书必须幂等复用** (sign_msix_local.ps1 已内建): 每次新建证书 = 换新指纹,
  而 LocalMachine\TrustedPeople 信任的是旧指纹 → 0x800B0109; 换证书必须重跑 trust_cert_machine.ps1
- **同版本号覆盖安装可能不换二进制**: 改代码验证必须 Remove-AppxPackage 后重装,
  别信 -ForceUpdateFromAnyVersion (同版本号会静默保持旧文件)
- **杀进程按路径过滤**: 用户日常跑的便携版与 MSIX 实例同名, 用 `$_.Path -like "*WindowsApps*"` 区分
- **cargo 指纹可能失灵**: 框架 (patch 本地路径) 改了但产品构建疑似没带上 (E0433 找不到新模块)
  → `cargo clean -p danqing` 强制重建, 别怀疑代码 (2026-09-01 真实发生)
- **任务栏图标被 AUMID 缓存污染**: 首装时若 exe 无内嵌图标/资产路径大小写不匹配,
  任务栏按钮落蓝色占位块, 之后重装/bump 版本都不刷新 (缓存键=PFN!App, 与版本无关);
  任务视图磁贴图标走另一条解析路径不受影响, 可用来区分「包资产坏了」还是「缓存坏了」;
  真实商店用户全新安装不受影响 → 不阻塞提交; 本机验证等 shell 自行刷新或清 iconcache
- **exe 内嵌图标必须验证**: build_freemium.ps1 的 IcoPath 默认值曾指向不存在的 logo.ico,
  patch_icon 静默跳过 (仅 warning) → 构建输出里必须看到 "Injecting logo" 且无 warning

## 清理方法

- 证书: certmgr.msc (当前用户) + certlm.msc (本机) -> 受信任的发布者 -> CN=5F2A7EA5-...
- 卸载: 设置 -> 应用 -> 丹青-番茄钟
