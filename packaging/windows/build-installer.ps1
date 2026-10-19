#Requires -Version 5.1
<#
.SYNOPSIS
    Builds the Inno Setup installer for tray-monitor.

.DESCRIPTION
    The version comes from the built executable's VERSIONINFO, so the installer
    always matches the binary and the release tag. ISCC is discovered from
    $env:ISCC, the per-user winget location, or the machine-wide install.

.PARAMETER Exe
    Path to the release tray-monitor.exe. Required.

.PARAMETER OutputDir
    Directory for the setup executable. Defaults to <repo>\dist.

.PARAMETER Iscc
    Explicit path to ISCC.exe. Usually unnecessary.

.EXAMPLE
    pwsh -File packaging/windows/build-installer.ps1 -Exe target/release/tray-monitor.exe
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Exe,

    [string]$OutputDir,

    [string]$Iscc
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$iss = Join-Path $PSScriptRoot 'tray-monitor.iss'

if (-not (Test-Path $Exe)) {
    throw "Executable not found: $Exe. Run 'cargo build --release' first."
}
$Exe = (Resolve-Path $Exe).Path

if (-not $OutputDir) {
    $OutputDir = Join-Path $root 'dist'
}
$OutputDir = [IO.Path]::GetFullPath($OutputDir)
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

# build.rs는 CARGO_PKG_VERSION을 VERSIONINFO에 넣는다. 별도 버전 파일이 없다.
$version = (Get-Item $Exe).VersionInfo.ProductVersion
if (-not $version -or $version -notmatch '^\d+\.\d+\.\d+') {
    throw "Could not read a semantic version from $Exe (got '$version')."
}

if (-not $Iscc) {
    $candidates = @(
        $env:ISCC,
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
        'C:\Program Files\Inno Setup 6\ISCC.exe'
    ) | Where-Object { $_ -and (Test-Path $_) }
    $Iscc = $candidates | Select-Object -First 1
}
if (-not $Iscc) {
    throw 'ISCC.exe not found. Install Inno Setup 6 or set $env:ISCC.'
}

$arguments = @(
    "/DAppVersion=$version",
    "/DSourceExe=$Exe",
    "/DSourceRoot=$root",
    "/DOutputDir=$OutputDir",
    $iss
)

Write-Host "ISCC $($Iscc)"
Write-Host "Version $version"
& $Iscc @arguments
if ($LASTEXITCODE -ne 0) {
    throw "ISCC failed with exit code $LASTEXITCODE."
}

$setup = Join-Path $OutputDir "tray-monitor-$version-windows-x64-setup.exe"
if (-not (Test-Path $setup)) {
    throw "ISCC reported success but $setup is missing."
}
Write-Host "Installer: $setup"
Write-Output $setup
