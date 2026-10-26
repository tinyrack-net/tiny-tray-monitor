# 실행 중인 TinyTrayMonitor의 설정 창을 언어별로 PrintWindow 캡처한다.
#
# 이미 떠 있는 인스턴스(Store 버전 포함)를 그대로 쓴다. 두 번째 실행은 IPC로 넘어가므로
# `--settings`로 창만 열고, 언어 콤보를 바꿔 가며 찍은 뒤 원래 선택으로 되돌린다.
# 새 인스턴스를 띄우거나 기존 인스턴스를 강제로 끄지 않는다(AGENTS.md 규칙 24).
#
# -RunAtSignIn: 캐처하는 동안만 `로그인할 때 실행`을 켜고 끝나면 원래대로 돌린다.
# 새 설치의 기본값이 켜짐이라 스토어 이미지는 켜진 쪽을 보여 준다.
param(
    [string]$Exe = "target\release\tiny-tray-monitor.exe",
    [string]$OutDir = "out",
    [switch]$RunAtSignIn
)
$ErrorActionPreference = "Stop"

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Cap {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr lParam);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);

    public static IntPtr Find() {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            var c = new StringBuilder(64); GetClassName(h, c, 64);
            var t = new StringBuilder(256); GetWindowText(h, t, 256);
            if (c.ToString() == "#32770" && IsWindowVisible(h) && t.ToString().StartsWith("TinyTrayMonitor")) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@
[Cap]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
Add-Type -AssemblyName System.Drawing

$IDC_LANGUAGE = 1028
$CB_GETCURSEL = 0x147
$CB_SETCURSEL = 0x14E
$WM_COMMAND = 0x111
$CBN_SELCHANGE = 1

$hwnd = [Cap]::Find()
if ($hwnd -eq [IntPtr]::Zero) {
    Start-Process -FilePath $Exe -ArgumentList "--settings"
    for ($i = 0; $i -lt 50 -and $hwnd -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 200; $hwnd = [Cap]::Find() }
    if ($hwnd -eq [IntPtr]::Zero) { throw "settings window did not appear" }
}
$combo = [Cap]::GetDlgItem($hwnd, $IDC_LANGUAGE)
$original = [int][Cap]::SendMessage($combo, $CB_GETCURSEL, [IntPtr]::Zero, [IntPtr]::Zero)

function Select-Language([int]$index) {
    [Cap]::SendMessage($combo, $CB_SETCURSEL, [IntPtr]$index, [IntPtr]::Zero) | Out-Null
    $wParam = [IntPtr](($CBN_SELCHANGE -shl 16) -bor $IDC_LANGUAGE)
    [Cap]::SendMessage($hwnd, $WM_COMMAND, $wParam, $combo) | Out-Null
    Start-Sleep -Milliseconds 600
}

function Save-Window([string]$path) {
    [Cap]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 300
    $r = New-Object Cap+RECT
    [Cap]::GetWindowRect($hwnd, [ref]$r) | Out-Null
    # DWM 프레임(보이는 테두리)만 남기도록 확장 프레임 경계로 자른다.
    $f = New-Object Cap+RECT
    [Cap]::DwmGetWindowAttribute($hwnd, 9, [ref]$f, 16) | Out-Null
    $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap($w, $h)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [Cap]::PrintWindow($hwnd, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc); $g.Dispose()
    $crop = New-Object System.Drawing.Rectangle(($f.Left - $r.Left), ($f.Top - $r.Top), ($f.Right - $f.Left), ($f.Bottom - $f.Top))
    $out = $bmp.Clone($crop, $bmp.PixelFormat)
    $out.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose(); $out.Dispose()
    Write-Output "$path $($crop.Width)x$($crop.Height)"
}

$IDC_AUTOSTART = 1012
$BM_GETCHECK = 0xF0
$BM_CLICK = 0xF5
$check = [Cap]::GetDlgItem($hwnd, $IDC_AUTOSTART)
function Get-Checked { [int][Cap]::SendMessage($check, $BM_GETCHECK, [IntPtr]::Zero, [IntPtr]::Zero) }

# 앱은 StartupTask 변경을 자기 메시지로 미뤄 처리한다(규칙 16). 반영될 때까지 기다린다.
function Set-Checked([int]$want) {
    if ((Get-Checked) -eq $want) { return }
    [Cap]::SendMessage($check, $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    for ($i = 0; $i -lt 25 -and (Get-Checked) -ne $want; $i++) { Start-Sleep -Milliseconds 200 }
    Start-Sleep -Milliseconds 800
    if ((Get-Checked) -ne $want) { throw "Run at sign-in did not change to $want" }
}

# 언어를 바꾸면 창이 컨트롤을 다시 채우므로, 체크는 언어를 바꾼 뒤 캡처 직전에 맞춘다.
$was = Get-Checked
$langs = @{ en = 1; ko = 2; ja = 3 }
foreach ($lang in @("en", "ko", "ja")) {
    Select-Language $langs[$lang]
    if ($RunAtSignIn) { Set-Checked 1 }
    Save-Window (Join-Path (Resolve-Path $OutDir) "shot-$lang.png")
    if ($RunAtSignIn) { Set-Checked $was }
}
Select-Language $original
Write-Output "language restored to index $original, run at sign-in checked=$(Get-Checked)"
