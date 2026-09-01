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
    [string]$StageDir = "target\package\stage-store",
    [string]$OutDir = "target\msix",
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

# Copy Store assets
$StoreAssetsDir = Join-Path (Join-Path $RepoRoot $OutDir) "assets"
if (Test-Path $StoreAssetsDir) {
    $DestAssets = Join-Path $MsixDir "Assets"
    New-Item -ItemType Directory -Path $DestAssets -Force | Out-Null
    Copy-Item "$StoreAssetsDir\*" $DestAssets
    Write-Host "Copied Store assets"
} else {
    Write-Host "WARNING: Store assets not found. Run: python tools/gen_store_assets.py"
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
    '                          BackgroundColor="transparent"'
    '                          Square150x150Logo="Assets\Square150x150Logo.png"'
    '                          Square44x44Logo="Assets\Square44x44Logo.png">'
    '        <uap:DefaultTile Wide310x150Logo="Assets\Wide310x150Logo.png"'
    '                         ShortName="' + $DisplayName + '" />'
    '        <uap:SplashScreen Image="Assets\SplashScreen.png" />'
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
