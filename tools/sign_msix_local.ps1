# sign_msix_local.ps1 - MSIX 侧载测试: 自签名证书 + 信任 + 签名 + 安装
#
# 用法 (仓库根目录):
#   powershell -NoProfile -File tools/sign_msix_local.ps1
#
# 动作说明 (纯 .NET 实现, 绕开本机 Cert: PSDrive 不可用的问题):
#   1. CertificateRequest 内存生成自签名证书 (主题 = 商店 Publisher CN), 导出 PFX
#   2. 证书公钥加入 CurrentUser\TrustedPeople (侧载安装的前置信任, 仅当前用户)
#   3. signtool 用 PFX 给 MSIX 签名
#   4. Add-AppxPackage 安装
# 清理: certmgr.msc -> 当前用户 -> 受信任的发布者, 删主题 CN=5F2A7EA5-... 的自签名证书;
#       卸载: 设置 -> 应用 -> 丹青-番茄钟 -> 卸载。
# 注意: 仅用于本地测试; 提交商店的包无需签名 (商店收录时自动重签)。

param(
    [string]$MsixPath = "target\msix\danqing-pomodoro-store-v0.2.0-x64.msix",
    [string]$PublisherCN = "CN=5F2A7EA5-3366-4B8A-8C0D-3BE22575711A",
    [string]$PfxPassword = "sideload"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path "$PSScriptRoot\.."
$Signtool = Join-Path $RepoRoot "tools\sdk-tools\bin\10.0.22621.0\x64\signtool.exe"
$Msix = Join-Path $RepoRoot $MsixPath
$PfxPath = Join-Path $RepoRoot "target\msix\sideload-signing.pfx"

if (-not (Test-Path $Msix)) {
    Write-Host "ERROR: MSIX not found: $Msix (先跑 tools/build_msix.ps1)"
    exit 1
}

Write-Host "=== 1/4 生成自签名证书 (.NET CertificateRequest) ==="
$rsa = [System.Security.Cryptography.RSA]::Create(2048)
$req = New-Object System.Security.Cryptography.X509Certificates.CertificateRequest(
    $PublisherCN, $rsa,
    [System.Security.Cryptography.HashAlgorithmName]::SHA256,
    [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
# EKU: 代码签名 (1.3.6.1.5.5.7.3.3)
$eku = New-Object System.Security.Cryptography.OidCollection
[void]$eku.Add("1.3.6.1.5.5.7.3.3")
$req.CertificateExtensions.Add(
    (New-Object System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension($eku, $false)))
# 基本约束: 非 CA
$req.CertificateExtensions.Add(
    (New-Object System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension($false, $false, 0, $true)))
$cert = $req.CreateSelfSigned(
    [System.DateTimeOffset]::UtcNow.AddDays(-1),
    [System.DateTimeOffset]::UtcNow.AddYears(2))
[System.IO.File]::WriteAllBytes($PfxPath,
    $cert.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $PfxPassword))
Write-Host "PFX: $PfxPath"

Write-Host "=== 2/4 加入 CurrentUser\TrustedPeople ==="
$store = New-Object System.Security.Cryptography.X509Certificates.X509Store("TrustedPeople", "CurrentUser")
$store.Open("ReadWrite")
$store.Add($cert)
$store.Close()

Write-Host "=== 3/4 signtool 签名 ==="
& $Signtool sign /fd SHA256 /f $PfxPath /p $PfxPassword $Msix
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: signtool failed"
    exit 1
}

Write-Host "=== 4/4 Add-AppxPackage 安装 ==="
Add-AppxPackage $Msix
Write-Host ""
Write-Host "=== 完成 ==="
Write-Host "启动: explorer.exe shell:AppsFolder\14uncle.-_3y3rwcp1ep416!App"
