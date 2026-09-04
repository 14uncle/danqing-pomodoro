# build_msix.ps1 - Build MSIX package for Microsoft Store.
#
# Creates danqing-pomodoro-store.msix for Store submission.
# The Store auto-signs after upload, so no certificate needed.
#
# Usage (from repo root):
#   powershell -NoProfile -File tools/build_msix.ps1
#
# Prerequisites:
#   - Run build_freemium.ps1 first (builds store edition)
#   - Run python tools/gen_store_assets.py (generates Store icons)

param(
    [string]$Version = "0.2.0",
    [string]$StageDir = "..\release-archives\pomodoro\stage-store",
    [string]$OutDir = "..\release-archives\pomodoro\msix",
    # Partner Center 产品标识 (2026-09-01 回填): 应用和游戏 -> 产品标识 页
    [string]$PublisherCN = "CN=5F2A7EA5-3366-4B8A-8C0D-3BE22575711A",
    [string]$AppName = "14uncle.-",
    [string]$DisplayName = "丹青-番茄钟",
    [string]$PublisherDisplayName = "14uncle",
    [string]$Description = "专注陪伴的沉浸世界 —— 9 场景 shader 动效 x 环境音"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path "$PSScriptRoot\.."

# Locate makeappx.exe
$MakeAppx = Join-Path $RepoRoot "tools\sdk-tools\bin\10.0.22621.0\x64\makeappx.exe"
if (-not (Test-Path $MakeAppx)) {
    Write-Host "ERROR: makeappx.exe not found at $MakeAppx"
    Write-Host "Download: nuget install Microsoft.Windows.SDK.BuildTools"
    exit 1
}

# Validate stage dir
$StagePath = Join-Path $RepoRoot $StageDir
if (-not (Test-Path $StagePath)) {
    Write-Host "ERROR: Stage dir not found: $StagePath"
    Write-Host "Run build_freemium.ps1 first."
    exit 1
}

# Create staging dir
$MsixDir = Join-Path $OutDir "msix-staging"
if (Test-Path $MsixDir) { Remove-Item -Recurse -Force $MsixDir }
New-Item -ItemType Directory -Path $MsixDir -Force | Out-Null

Write-Host "=== Copying app files ==="

Copy-Item (Join-Path $StagePath "danqing-pomodoro.exe") $MsixDir
Copy-Item -Recurse (Join-Path $StagePath "assets") (Join-Path $MsixDir "assets")
$LicPath = Join-Path $StagePath "LICENSE-MIT"
if (Test-Path $LicPath) { Copy-Item $LicPath $MsixDir }

# Copy Store assets (缺失时自动生成, 不再仅警告)
$StoreAssetsDir = Join-Path (Join-Path $RepoRoot $OutDir) "assets"
if (-not (Test-Path $StoreAssetsDir)) {
    Write-Host "Store assets not found, running gen_store_assets.py ..."
    $Py = $null
    foreach ($c in @('python', 'python3', 'py')) {
        $w = Get-Command $c -ErrorAction SilentlyContinue
        if ($w) { $Py = $c; break }
    }
    if (-not $Py) { Write-Host "ERROR: python not found"; exit 1 }
    Push-Location $RepoRoot
    try { & $Py tools\gen_store_assets.py } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: gen_store_assets.py failed"; exit 1 }
}
$DestAssets = Join-Path $MsixDir "Assets"
New-Item -ItemType Directory -Path $DestAssets -Force | Out-Null
Copy-Item "$StoreAssetsDir\*" $DestAssets
Write-Host "Copied Store assets"

# 规范化资产目录名为 "Assets" (与 manifest 引用同大小写): 上面运行时资产先落成
# 小写 "assets", 商店资产合并入同一目录; shell 安装时按包内路径索引图标,
# 大小写不匹配会导致任务栏/磁贴图标落回蓝色占位块。运行时读取在 Windows 上
# 不区分大小写, 不受影响。
$MergedAssets = Join-Path $MsixDir "assets"
if (Test-Path $MergedAssets) {
    # 纯大小写改名在 Windows 上要两步 (直接改报 IOException)
    Rename-Item $MergedAssets "assets-case-tmp"
    Rename-Item (Join-Path $MsixDir "assets-case-tmp") "Assets"
}

Write-Host "=== Generating AppxManifest.xml ==="

$VerParts = $Version.Split('.')
while ($VerParts.Count -lt 3) { $VerParts += "0" }
$MsixVersion = "$($VerParts[0]).$($VerParts[1]).$($VerParts[2]).0"

$ManifestLines = @(
    '<?xml version="1.0" encoding="utf-8"?>'
    '<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"'
    '         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"'
    '         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"'
    '         IgnorableNamespaces="uap rescap">'
    ''
    '  <Identity Name="' + $AppName + '"'
    '            Publisher="' + $PublisherCN + '"'
    '            Version="' + $MsixVersion + '"'
    '            ProcessorArchitecture="x64" />'
    ''
    '  <Properties>'
    '    <DisplayName>' + $DisplayName + '</DisplayName>'
    '    <PublisherDisplayName>' + $PublisherDisplayName + '</PublisherDisplayName>'
    '    <Logo>Assets\StoreLogo.png</Logo>'
    '    <Description>' + $Description + '</Description>'
    '  </Properties>'
    ''
    '  <Dependencies>'
    '    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0" />'
    '  </Dependencies>'
    ''
    '  <Resources>'
    '    <Resource Language="en-US" />'
    '  </Resources>'
    ''
    '  <Applications>'
    '    <Application Id="App"'
    '                 Executable="danqing-pomodoro.exe"'
    '                 EntryPoint="Windows.FullTrustApplication">'
    '      <uap:VisualElements DisplayName="' + $DisplayName + '"'
    '                          Description="' + $Description + '"'
    # BackgroundColor 用 transparent (VS Code MSIX 同构 = 任务栏裸图标)。
    # ⚠️ 不要加回 <uap:DefaultTile> / <uap:SplashScreen> 子元素: 2026-09-04 实测,
    # 两者存在时 Win11 任务栏把 transparent 渲染成默认蓝底板; 全部去掉后与
    # VS Code 结构一致, 任务栏裸图标 (实验版本 v0.2.2 -> v0.2.3)。
    # 任务栏图标机制: 合并模式任务栏只认 manifest 的 Square44x44Logo 族
    # (microsoft/WindowsAppSDK#2730, 任务栏团队确认 by design); WM_SETICON 仅作用于
    # 「不合并」任务栏 —— 所以窗口图标代码对商店版任务栏无效, 全靠这里。
    '                          BackgroundColor="transparent"'
    '                          Square150x150Logo="Assets\Square150x150Logo.png"'
    '                          Square44x44Logo="Assets\Square44x44Logo.png">'
    '      </uap:VisualElements>'
    '    </Application>'
    '  </Applications>'
    ''
    '  <Capabilities>'
    '    <rescap:Capability Name="runFullTrust" />'
    '  </Capabilities>'
    ''
    '</Package>'
)

$ManifestPath = Join-Path $MsixDir "AppxManifest.xml"
$ManifestContent = $ManifestLines -join "`n"
# 无 BOM 写盘: BOM 会让包安装失败 (fix_msix.py 当年修的就是它)
[System.IO.File]::WriteAllText($ManifestPath, $ManifestContent, (New-Object System.Text.UTF8Encoding($false)))

Write-Host "=== Generating resources.pri (MakePri) ==="

# 生成资源索引 resources.pri: shell 靠它做「限定资源解析」, 才能识别
# Square44x44Logo.scale-*/targetsize-*_altform-unplated 等图标变体并用于任务栏
# 裸图标渲染。缺 resources.pri 时, shell 只能拿基础 Square44x44Logo.png 垫
# BackgroundColor 底板 → 任务栏蓝底 (2026-09-04 商店版根因; 对照 ScreenToGif
# 与 rufus 的 res/appstore/packme.cmd 均含此文件, 均裸图标)。
$MakePri = Join-Path $RepoRoot "tools\sdk-tools\bin\10.0.22621.0\x64\makepri.exe"
if (-not (Test-Path $MakePri)) {
    Write-Host "ERROR: makepri.exe not found at $MakePri"
    exit 1
}
Push-Location $MsixDir
try {
    & $MakePri createconfig /cf priconfig.xml /dq lang-en-US /pv 10.0.0 /o
    if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: makepri createconfig failed"; exit 1 }
    & $MakePri new /pr . /cf priconfig.xml /of resources.pri /o
    if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: makepri new failed"; exit 1 }
    # priconfig.xml 只是生成 resources.pri 的中间配置, 不进包
    Remove-Item priconfig.xml -ErrorAction SilentlyContinue
} finally {
    Pop-Location
}

Write-Host "=== Packaging with makeappx.exe ==="

$MsixPath = Join-Path $OutDir "danqing-pomodoro-store-v${Version}-x64.msix"
if (Test-Path $MsixPath) { Remove-Item $MsixPath -Force }

& $MakeAppx pack /d $MsixDir /p $MsixPath /o
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: makeappx.exe failed."
    exit 1
}

$Size = (Get-Item $MsixPath).Length
$SizeMB = [math]::Round($Size / 1MB, 1)

Write-Host ""
Write-Host "=== MSIX package created ==="
Write-Host "File: $MsixPath"
Write-Host "Size: $Size bytes ($SizeMB MB)"
Write-Host ""
Write-Host "The Store will auto-sign this package."
Write-Host "Upload to Partner Center -> your app -> Packages."
