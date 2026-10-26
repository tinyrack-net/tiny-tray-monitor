# 주 모니터 전체를 물리 픽셀 그대로 캡처한다.
# PowerShell은 DPI를 인식하지 않는 프로세스라서 배율이 100%가 아니면 축소·확대된 화면이 찍힌다.
# 캡처 전에 PerMonitorV2로 전환해 실제 해상도를 받는다.
param(
    [string]$Out = "out\screen.png"
)
$ErrorActionPreference = "Stop"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Dpi {
    [DllImport("user32.dll")]
    public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")]
    public static extern int GetSystemMetrics(int index);
    [DllImport("user32.dll")]
    public static extern uint GetDpiForSystem();
}
"@
# DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = -4
[Dpi]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

Add-Type -AssemblyName System.Drawing
$width = [Dpi]::GetSystemMetrics(0)
$height = [Dpi]::GetSystemMetrics(1)

$bmp = New-Object System.Drawing.Bitmap($width, $height)
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
$gfx.CopyFromScreen(0, 0, 0, 0, $bmp.Size)
$gfx.Dispose()

$path = [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $Out))
$dir = Split-Path $path -Parent
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
$bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()

Write-Output "$path ${width}x${height} dpi=$([Dpi]::GetDpiForSystem())"
