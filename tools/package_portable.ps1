# package_portable.ps1 - portable Windows zip for danqing release binaries.
#
# Stages target/release/[examples/]<binary>.exe (+ any *.dll), assets/,
# README and any LICENSE* files into a staging dir, then zips into
# target/package/<binary>-v<version>-win-x64.zip with SHA256.
#
# Usage (from repo root):
#   powershell -NoProfile -File tools/package_portable.ps1
#   powershell -NoProfile -File tools/package_portable.ps1 -BinaryName showcase
#   powershell -NoProfile -File tools/package_portable.ps1 -BinaryName danqing-pomodoro -Version 0.2.0
#
# If the release binary is missing, runs cargo build --release --example <name>.
# Pure ASCII by design (see repo tooling rules).

param(
    [string]$BinaryName = "showcase",
    [string]$Version = "",
    [string]$OutDir = "target\package",
    [string]$IcoPath = "assets\logo\logo.ico"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path "$PSScriptRoot\.."

# Default $Version from Cargo.toml's [package].version when -Version not passed.
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
    } else {
        Write-Host "ERROR: -Version not provided and Cargo.toml missing"
        exit 1
    }
}

$ReleaseDir = Join-Path $RepoRoot "target\release"
$Stage = Join-Path $OutDir "stage"
$ArchiveBase = "${BinaryName}-v${Version}-win-x64"
$ZipPath = Join-Path $OutDir "${ArchiveBase}.zip"

# Locate pre-built binary: try main bin first, then example path.
$BinaryPath = $null
foreach ($candidate in @(
    (Join-Path $ReleaseDir "${BinaryName}.exe"),
    (Join-Path $ReleaseDir "examples\${BinaryName}.exe")
)) {
    if (Test-Path $candidate) { $BinaryPath = $candidate; break }
}

if (-not $BinaryPath) {
    Write-Host "Binary missing; running cargo build --release --example $BinaryName ..."
    Push-Location $RepoRoot
    try {
        cargo build --release --example $BinaryName
        if ($LASTEXITCODE -ne 0) { exit 1 }
    } finally {
        Pop-Location
    }
    $BinaryPath = Join-Path $ReleaseDir "examples\${BinaryName}.exe"
}

if (-not (Test-Path $BinaryPath)) {
    Write-Host "ERROR: binary not found after build: $BinaryPath"
    exit 1
}

Write-Host ("Using binary: {0}" -f $BinaryPath)

# Clean + create stage dir.
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Path $Stage -Force | Out-Null

# Main exe.
Copy-Item $BinaryPath $Stage

# Inject the danqing logo (assets/logo/logo.ico) into the staged exe via
# tools/patch_icon.py (Win32 kernel32!UpdateResource, no windres required).
# Idempotent: re-running on a patched exe just rewrites the same RT_ICON entries.
$PatchIconScript = Join-Path $RepoRoot "tools/patch_icon.py"
if (Test-Path $PatchIconScript) {
    $Py = $null
    foreach ($c in @('python', 'python3', 'py')) {
        $w = Get-Command $c -ErrorAction SilentlyContinue
        if ($w) { $Py = $c; break }
    }
    if ($Py) {
        $StageExe = Join-Path $Stage (Split-Path $BinaryPath -Leaf)
        Write-Host "Injecting danqing logo into $StageExe ..."
        Push-Location $RepoRoot
        try {
            & $Py tools/patch_icon.py --ico $IcoPath --exe $StageExe
            if ($LASTEXITCODE -ne 0) {
                Write-Warning "patch_icon.py exited $LASTEXITCODE -- packaged exe will keep the default icon."
            }
        } finally {
            Pop-Location
        }
    } else {
        Write-Warning "Python not on PATH; skipping danqing logo injection. Install Python 3.x if you want the exe to carry the danqing logo."
    }
} else {
    Write-Warning "tools/patch_icon.py not found; skipping danqing logo injection."
}

# Runtime DLLs in release root (if any).
$Dlls = @(Get-ChildItem -Path $ReleaseDir -Filter "*.dll" -File -ErrorAction SilentlyContinue)
if ($Dlls.Count -gt 0) {
    foreach ($d in $Dlls) { Copy-Item $d.FullName $Stage }
    Write-Host ("Copied {0} runtime DLL(s)" -f $Dlls.Count)
} else {
    Write-Host "No runtime DLLs in target/release (statically linked or system-provided)."
}

# assets/.
$AssetsDir = Join-Path $RepoRoot "assets"
if (Test-Path $AssetsDir) {
    Copy-Item -Recurse $AssetsDir (Join-Path $Stage "assets")
    Write-Host "Copied assets/"
} else {
    Write-Warning "No assets/ directory at repo root."
}

# LICENSE* files (warn if missing -- Cargo.toml declares MIT OR Apache-2.0).
$LicenseNames = @("LICENSE", "LICENSE.md", "LICENSE.txt",
                  "LICENSE-APACHE", "LICENSE-APACHE.txt",
                  "LICENSE-MIT", "LICENSE-MIT.txt")
$LicenseFound = $false
foreach ($name in $LicenseNames) {
    $p = Join-Path $RepoRoot $name
    if (Test-Path $p) {
        Copy-Item $p $Stage
        $LicenseFound = $true
    }
}
if (-not $LicenseFound) {
    Write-Warning "No LICENSE* file at repo root. Cargo.toml declares 'MIT OR Apache-2.0' but no file present."
    Write-Warning "Distribution will be non-compliant. Add LICENSE(-APACHE|-MIT) and re-run."
}

# README.
$ReadmePath = Join-Path $RepoRoot "README.md"
if (Test-Path $ReadmePath) {
    Copy-Item $ReadmePath $Stage
}

# Create zip (omit -CompressionLevel for PS 5.1 compat; default = Optimal).
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
Compress-Archive -Path "$Stage\*" -DestinationPath $ZipPath

# SHA256 sidecar via .NET (avoids Get-FileHash cmdlet dependency).
# Write via WriteAllText + Encoding.ASCII to bypass Out-File's UTF-16 default.
$Bytes = [System.IO.File]::ReadAllBytes($ZipPath)
$Sha256 = [System.Security.Cryptography.SHA256]::Create()
$Hash = ([System.BitConverter]::ToString($Sha256.ComputeHash($Bytes)) -replace '-', '').ToLower()
$Sha256.Dispose()
[System.IO.File]::WriteAllText("$ZipPath.sha256", $Hash, [System.Text.Encoding]::ASCII)

Write-Host ""
Write-Host ("Built:  {0}" -f $ZipPath)
Write-Host ("SHA256: {0}" -f $Hash)
Write-Host ("Size:   {0:N0} bytes ({1:N1} KB)" -f (Get-Item $ZipPath).Length, ((Get-Item $ZipPath).Length / 1KB))