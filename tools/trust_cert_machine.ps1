# trust_cert_machine.ps1 - 把侧载测试证书(仅公钥)加入 LocalMachine\TrustedPeople
#
# 背景: AppX 部署服务 (Msixvc) 以 SYSTEM 身份校验签名链, 只认本机存储;
#       sign_msix_local.ps1 写入的 CurrentUser\TrustedPeople 对它不可见。
# 前提: 已跑过 tools/sign_msix_local.ps1 (生成了 target\msix\sideload-signing.pfx)
# 用法 (触发 UAC 提权):
#   powershell -NoProfile -Command "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-File','F:/github/farm01/danqing-pomodoro/tools/trust_cert_machine.ps1'"
# 清理: certlm.msc -> 受信任的发布者 -> 删主题 CN=5F2A7EA5-... 的证书。

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path "$PSScriptRoot\.."
$PfxPath = Join-Path $RepoRoot "target\msix\sideload-signing.pfx"

if (-not (Test-Path $PfxPath)) {
    Write-Host "ERROR: $PfxPath 不存在, 先跑 tools/sign_msix_local.ps1"
    exit 1
}

# 直接入库 PFX 加载的证书对象 (私钥仍留在用户密钥容器, 机器存储只持久化证书本身)
$pfx = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($PfxPath, "sideload")

$store = New-Object System.Security.Cryptography.X509Certificates.X509Store("TrustedPeople", "LocalMachine")
$store.Open("ReadWrite")
$store.Add($pfx)
$store.Close()
Write-Host "OK: 已加入 LocalMachine\TrustedPeople -> $($pfx.Thumbprint) $($pfx.Subject)"
