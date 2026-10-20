#Requires -Version 5.1
<#
.SYNOPSIS
    Builds the MSIX package for tray-monitor.

.DESCRIPTION
    The package is unsigned. It is meant for Store submission (where Microsoft
    signs it) or for developer-mode registration. Installing it as a normal user
    requires a trusted signature.

    The version comes from the executable's VERSIONINFO and is expanded to four
    parts (0.3.1 -> 0.3.1.0), as the manifest requires.

    Identity defaults can be overridden with MSIX_IDENTITY_NAME, MSIX_PUBLISHER
    and MSIX_PUBLISHER_DISPLAY_NAME. The real values come from Partner Center.

.PARAMETER Exe
    Path to the release tray-monitor.exe. Required.

.PARAMETER OutputDir
    Directory for the .msix. Defaults to <repo>\dist.

.PARAMETER IdentityName
    Package identity name. Defaults to tinyrack.TinyTrayManager.

.PARAMETER Publisher
    Publisher subject. Defaults to the tinyrack Partner Center publisher.

.PARAMETER PublisherDisplayName
    Display name of the publisher. Defaults to tinyrack.

.PARAMETER DisplayName
    Product display name. Defaults to TinyTrayManager.

.EXAMPLE
    pwsh -File packaging/windows/build-msix.ps1 -Exe target/release/tray-monitor.exe
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Exe,

    [string]$OutputDir,

    [string]$IdentityName = $(if ($env:MSIX_IDENTITY_NAME) { $env:MSIX_IDENTITY_NAME } else { 'tinyrack.TinyTrayManager' }),

    [string]$Publisher = $(if ($env:MSIX_PUBLISHER) { $env:MSIX_PUBLISHER } else { 'CN=42FA8D93-3683-4A00-A40C-5ED8A02E8B3A' }),

    [string]$PublisherDisplayName = $(if ($env:MSIX_PUBLISHER_DISPLAY_NAME) { $env:MSIX_PUBLISHER_DISPLAY_NAME } else { 'tinyrack' }),

    [string]$DisplayName = $(if ($env:MSIX_DISPLAY_NAME) { $env:MSIX_DISPLAY_NAME } else { 'TinyTrayManager' }),

    [ValidateSet('x64', 'arm64')]
    [string]$Arch = 'x64'
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$template = Join-Path $PSScriptRoot 'AppxManifest.xml'
$assets = Join-Path $root 'resources\msix'

if (-not (Test-Path $Exe)) {
    throw "Executable not found: $Exe. Run 'cargo build --release' first."
}
$Exe = (Resolve-Path $Exe).Path

if (-not (Test-Path $template)) {
    throw "Manifest template not found: $template"
}
if (-not (Test-Path $assets)) {
    throw "MSIX assets not found: $assets. Run 'python tools/make_assets.py'."
}

if (-not $OutputDir) {
    $OutputDir = Join-Path $root 'dist'
}
$OutputDir = [IO.Path]::GetFullPath($OutputDir)
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

# VERSIONINFO의 ProductVersion을 MSIX의 4자리 버전으로 확장한다.
$version = (Get-Item $Exe).VersionInfo.ProductVersion
if (-not $version -or $version -notmatch '^\d+(\.\d+){1,3}$') {
    throw "Could not read a numeric version from $Exe (got '$version')."
}
$parts = @($version.Split('.'))
while ($parts.Count -lt 4) {
    $parts += '0'
}
foreach ($part in $parts) {
    if (([int]$part) -gt 65535) {
        throw "Version part '$part' does not fit in 16 bits."
    }
}
$quad = $parts -join '.'
$shortVersion = ($parts[0..2] -join '.')

# 스테이징은 out/ 아래에 두고, 등록 테스트에도 그대로 쓸 수 있게 남긴다.
$stage = Join-Path $root "out\msix-$Arch"
if (Test-Path $stage) {
    Remove-Item -Recurse -Force $stage
}
New-Item -ItemType Directory -Force -Path $stage | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage 'Assets') | Out-Null

Copy-Item $Exe (Join-Path $stage 'tray-monitor.exe')
Copy-Item (Join-Path $assets '*.png') (Join-Path $stage 'Assets')

$manifest = Get-Content $template -Raw -Encoding UTF8
$manifest = $manifest.Replace('@IDENTITY_NAME@', $IdentityName)
$manifest = $manifest.Replace('@PUBLISHER@', $Publisher)
$manifest = $manifest.Replace('@PUBLISHER_DISPLAY_NAME@', $PublisherDisplayName)
$manifest = $manifest.Replace('@DISPLAY_NAME@', $DisplayName)
$manifest = $manifest.Replace('@VERSION@', $quad)
$manifest = $manifest.Replace('@ARCH@', $Arch)
$manifestPath = Join-Path $stage 'AppxManifest.xml'
[IO.File]::WriteAllText($manifestPath, $manifest, (New-Object Text.UTF8Encoding($false)))

# manifest를 먼저 검증한다. makeappx 오류보다 원인이 분명하다.
$xml = [xml]$manifest
if ($xml.Package.Identity.Version -ne $quad) {
    throw 'Manifest version substitution failed.'
}
$task = $xml.Package.Applications.Application.Extensions.ChildNodes |
    Where-Object { $_.StartupTask } | Select-Object -First 1
if (-not $task -or $task.StartupTask.TaskId -ne 'TrayMonitorStartup') {
    throw 'Manifest is missing the TrayMonitorStartup startup task.'
}

function Find-MakeAppx {
    $bases = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "$env:ProgramFiles\Windows Kits\10\bin"
    )
    foreach ($base in $bases) {
        if (-not $base -or -not (Test-Path $base)) { continue }
        $candidate = Get-ChildItem $base -Directory |
            Sort-Object Name -Descending |
            ForEach-Object { Join-Path $_.FullName "$Arch\makeappx.exe" } |
            Where-Object { Test-Path $_ } |
            Select-Object -First 1
        if ($candidate) { return $candidate }
    }
    return $null
}

$makeappx = Find-MakeAppx
if (-not $makeappx) {
    throw "makeappx.exe not found under Windows Kits\10\bin. Install the Windows SDK."
}

$package = Join-Path $OutputDir "tray-monitor-$shortVersion-windows-$Arch.msix"
if (Test-Path $package) {
    Remove-Item -Force $package
}

Write-Host "makeappx: $makeappx"
Write-Host "Version: $quad"
& $makeappx pack /d $stage /p $package /o
if ($LASTEXITCODE -ne 0) {
    throw "makeappx pack failed with exit code $LASTEXITCODE."
}

# 왕복 검증. 패키지 안에 manifest/exe/자산이 실제로 들어갔는지 확인한다.
$verify = Join-Path $root "out\msix-verify-$Arch"
if (Test-Path $verify) {
    Remove-Item -Recurse -Force $verify
}
& $makeappx unpack /p $package /d $verify /o
if ($LASTEXITCODE -ne 0) {
    throw "makeappx unpack failed with exit code $LASTEXITCODE."
}
foreach ($name in @('AppxManifest.xml', 'tray-monitor.exe', 'Assets\StoreLogo.png', 'Assets\Square150x150Logo.png')) {
    if (-not (Test-Path (Join-Path $verify $name))) {
        throw "Package is missing $name."
    }
}

Write-Host "MSIX: $package"
Write-Output $package
