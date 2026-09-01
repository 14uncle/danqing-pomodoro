# build_freemium.ps1 - Build both free and full versions of danqing-pomodoro.
#
# Produces:
#   target/package/danqing-pomodoro-free-v<version>-win-x64.zip
#   target/package/danqing-pomodoro-full-v<version>-win-x64.zip
#
# Usage (from repo root):
#   powershell -NoProfile -File tools/build_freemium.ps1
#   powershell -NoProfile -File tools/build_freemium.ps1 -Version 0.2.0

param(
    [string]$Version = "",
    [string]$OutDir = "target\package",
    [string]$IcoPath = "assets\logo\logo.ico"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path "$PSScriptRoot\.."

# Read version from Cargo.toml
if (-not $PSBoundParameters.ContainsKey('Version')) {
    $CargoToml = Join-Path $RepoRoot "Cargo.toml"
    if (Test-Path $CargoToml) {
        $Match = [regex]::Match(
            (Get-Content $CargoToml -Raw),
            '(?s)\[package\][^[]*?version\s*=\s*"([^"]+)"'
        )
        if ($Match.Success) {
            $Version = $Match.Groups[1].Value
            Write-Host ("Version from Cargo.toml: {0}" -f $Version)
        } else {
            Write-Host "ERROR: -Version not provided and Cargo.toml [package] section has no version"
            exit 1
        }
    }
}

$ReleaseDir = Join-Path $RepoRoot "target\release"

# Locate Python for icon injection
$Py = $null
foreach ($c in @('python', 'python3', 'py')) {
    $w = Get-Command $c -ErrorAction SilentlyContinue
    if ($w) { $Py = $c; break }
}

function Build-And-Package {
    param(
        [string]$Edition,    # "free" or "full"
        [string]$CargoArgs   # e.g. "" or "--features full"
    )

    $BinaryName = "danqing-pomodoro"
    $Stage = Join-Path $OutDir "stage-$Edition"
    $ArchiveBase = "${BinaryName}-${Edition}-v${Version}-win-x64"
    $ZipPath = Join-Path $OutDir "${ArchiveBase}.zip"

    Write-Host ""
    Write-Host "=== Building $Edition edition ==="

    Push-Location $RepoRoot
    try {
        $buildCmd = "cargo build --release $CargoArgs"
        Write-Host "Running: $buildCmd"
        Invoke-Expression $buildCmd
        if ($LASTEXITCODE -ne 0) { exit 1 }
    } finally {
        Pop-Location
    }

    $BinaryPath = Join-Path $ReleaseDir "${BinaryName}.exe"
    if (-not (Test-Path $BinaryPath)) {
        Write-Host "ERROR: binary not found: $BinaryPath"
        exit 1
    }

    # Clean + create stage dir
    if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
    New-Item -ItemType Directory -Path $Stage -Force | Out-Null

    Copy-Item $BinaryPath $Stage

    # Inject icon
    $PatchIconScript = Join-Path $RepoRoot "tools/patch_icon.py"
    if ($Py -and (Test-Path $PatchIconScript)) {
        $StageExe = Join-Path $Stage "${BinaryName}.exe"
        Write-Host "Injecting logo into $StageExe ..."
        Push-Location $RepoRoot
        try {
            & $Py tools/patch_icon.py --ico $IcoPath --exe $StageExe
            if ($LASTEXITCODE -ne 0) {
                Write-Warning "patch_icon.py failed; exe will keep default icon."
            }
        } finally {
            Pop-Location
        }
    }

    # Runtime DLLs
    $Dlls = @(Get-ChildItem -Path $ReleaseDir -Filter "*.dll" -File -ErrorAction SilentlyContinue)
    if ($Dlls.Count -gt 0) {
        foreach ($d in $Dlls) { Copy-Item $d.FullName $Stage }
    }

    # Assets
    $AssetsDir = Join-Path $RepoRoot "assets"
    if (Test-Path $AssetsDir) {
        Copy-Item -Recurse $AssetsDir (Join-Path $Stage "assets")
    }

    # LICENSE
    foreach ($name in @("LICENSE", "LICENSE.md", "LICENSE-MIT", "LICENSE-APACHE")) {
        $p = Join-Path $RepoRoot $name
        if (Test-Path $p) { Copy-Item $p $Stage; break }
    }

    # README
    $ReadmePath = Join-Path $RepoRoot "README.md"
    if (Test-Path $ReadmePath) { Copy-Item $ReadmePath $Stage }

    # Zip
    if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
    Compress-Archive -Path "$Stage\*" -DestinationPath $ZipPath

    # SHA256
    $Bytes = [System.IO.File]::ReadAllBytes($ZipPath)
    $Sha256 = [System.Security.Cryptography.SHA256]::Create()
    $Hash = ([System.BitConverter]::ToString($Sha256.ComputeHash($Bytes)) -replace '-', '').ToLower()
    $Sha256.Dispose()
    [System.IO.File]::WriteAllText("$ZipPath.sha256", $Hash, [System.Text.Encoding]::ASCII)

    Write-Host ("Built:  {0}" -f $ZipPath)
    Write-Host ("SHA256: {0}" -f $Hash)
    Write-Host ("Size:   {0:N0} bytes ({1:N1} KB)" -f (Get-Item $ZipPath).Length, ((Get-Item $ZipPath).Length / 1KB))
}

# Build all editions
Build-And-Package -Edition "free" -CargoArgs ""
Build-And-Package -Edition "store" -CargoArgs "--features store"
Build-And-Package -Edition "full" -CargoArgs "--features full"

Write-Host ""
Write-Host "=== All editions packaged ==="
Write-Host "Free:  $OutDir/danqing-pomodoro-free-v${Version}-win-x64.zip"
Write-Host "Store: $OutDir/danqing-pomodoro-store-v${Version}-win-x64.zip"
Write-Host "Full:  $OutDir/danqing-pomodoro-full-v${Version}-win-x64.zip"
