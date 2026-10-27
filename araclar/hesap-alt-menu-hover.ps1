#requires -Version 5.1
<#
.SYNOPSIS
  Kervan — "Diğer araçlar" ALT MENÜSÜNÜ gerçek imleçle açıp yakalar.

.DESCRIPTION
  NEDEN AYRI BETİK: Alt menü TIKLAMAYLA değil ÜZERİNE GELİNCE (hover)
  açılıyor. `dene=` tıklaması menüyü kapatıyor; bu yüzden alt menü ölçüm
  ucuyla okunamıyor. Bu betik menüyü ölçüm ucundan açar (eşleşmeyen seçici
  → menü açık kalır), imleci satırın üzerine taşır, alt menü çizilince kareyi
  alır ve imleci ESKİ YERİNE döndürür.

.PARAMETER Hesap
  Ölçüm defterindeki hesap (ör. ercaner1988@gmail.com).

.PARAMETER Hwnd
  Pencerenin tanıtıcısı. `hesap-ekran-yaka.ps1` çıktısındaki eşlemeden gelir:
  aynı hesap her turda aynı pencereye düşüyor.

.PARAMETER SatirDx
  Menü satırının pencere İÇİ koordinatı (dp). Varsayılan 1216x653'lük
  yakalamada "Diğer araçlar" satırı (447,430) → /1.25 = (358,344).

.EXAMPLE
  powershell -NoProfile -File araclar\hesap-alt-menu-hover.ps1 `
    -Hesap ercaner1988@gmail.com -Hwnd 18879692 -Cikti olc-ekran
#>
param(
  [Parameter(Mandatory = $true)][string]$Hesap,
  [Parameter(Mandatory = $true)][long]$Hwnd,
  [string]$Cikti = "olc-ekran",
  [int]$SatirDx = 495,
  [int]$SatirDy = 430,
  # Pencere ekran dışına taşıyorsa imleç satıra ulaşamaz (SetCursorPos kırpılır).
  # -Tasi: pencereyi geçici olarak görünür alana taşır, sonunda eski yerine koyar.
  [switch]$Tasi
)

$ErrorActionPreference = "Stop"
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class KervanHover {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RECT { public int L, T, R, B; }
  public struct POINT { public int X, Y; }
}
"@ -ErrorAction SilentlyContinue
Add-Type -AssemblyName System.Drawing
[void][KervanHover]::SetProcessDPIAware()

$pencere = [IntPtr]$Hwnd
$r = New-Object KervanHover+RECT
if (-not [KervanHover]::GetWindowRect($pencere, [ref]$r)) { throw "pencere bulunamadı: $Hwnd" }
$w = $r.R - $r.L; $h = $r.B - $r.T
"pencere ${w}x${h} @$($r.L),$($r.T)"
$eskiR = New-Object KervanHover+RECT
$eskiR.L = $r.L; $eskiR.T = $r.T; $eskiR.R = $r.R; $eskiR.B = $r.B

function KareAl {
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc(); [void][KervanHover]::PrintWindow($pencere, $hdc, 2); $g.ReleaseHdc($hdc); $g.Dispose()
  return $bmp
}
function Fark($a, $b) {
  $f = 0
  for ($x = 4; $x -lt $a.Width; $x += 9) {
    for ($y = 4; $y -lt $a.Height; $y += 9) {
      $c1 = $a.GetPixel($x, $y); $c2 = $b.GetPixel($x, $y)
      if ([Math]::Abs($c1.R - $c2.R) + [Math]::Abs($c1.G - $c2.G) + [Math]::Abs($c1.B - $c2.B) -gt 26) { $f++ }
    }
  }
  return $f
}

$eski = New-Object KervanHover+POINT
[void][KervanHover]::GetCursorPos([ref]$eski)
"imleç eski yer: $($eski.X),$($eski.Y)"

$tasindi = $false
if ($Tasi) {
  # SWP_NOSIZE|SWP_NOZORDER|SWP_NOACTIVATE
  [void][KervanHover]::SetWindowPos($pencere, [IntPtr]::Zero, 60, 60, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010)
  Start-Sleep -Milliseconds 400
  [void][KervanHover]::GetWindowRect($pencere, [ref]$r)
  $tasindi = $true
  "pencere taşındı: @$($r.L),$($r.T)"
}

# Eşleşmeyen seçici: eklenti menüyü açar, hiçbir şey bulamaz, menü açık kalır.
$q = "hesap=" + [uri]::EscapeDataString($Hesap) +
     "&secici=" + [uri]::EscapeDataString("#kervan-hover-eslesmez") +
     "&dugme=" + [uri]::EscapeDataString('button[aria-label*="araç" i]')
$url = "http://127.0.0.1:20129/kervan/olc?$q"

try {
  $temel = KareAl
  $is = Start-Job -ScriptBlock {
    param($u)
    try { (Invoke-WebRequest -Uri $u -UseBasicParsing -TimeoutSec 120).StatusCode } catch { "hata" }
  } -ArgumentList $url

  # Menünün çizilmesini bekle.
  $bas = Get-Date
  $menuAcik = $false
  while (((Get-Date) - $bas).TotalSeconds -lt 8) {
    $b = KareAl
    if ((Fark $temel $b) -gt 60) { $menuAcik = $true; $b.Dispose(); break }
    $b.Dispose()
    Start-Sleep -Milliseconds 120
  }
  if (-not $menuAcik) { throw "menü açılmadı (8 sn)" }

  # İmleci satırın üzerine taşı → alt menü hover ile açılır.
  $hedefX = $r.L + $SatirDx
  $hedefY = $r.T + $SatirDy
  [void][KervanHover]::SetCursorPos($hedefX, $hedefY)
  "imleç taşındı: $hedefX,$hedefY"

  $en = @{ f = -1; bmp = $null }
  $bas = Get-Date
  while (((Get-Date) - $bas).TotalSeconds -lt 12) {
    $b = KareAl
    $f = Fark $temel $b
    if ($f -gt $en.f) { if ($en.bmp) { $en.bmp.Dispose() }; $en = @{ f = $f; bmp = $b } } else { $b.Dispose() }
    Start-Sleep -Milliseconds 120
  }
  $durum = Receive-Job $is -Wait -AutoRemoveJob

  if (-not (Test-Path $Cikti)) { [void](New-Item -ItemType Directory -Path $Cikti) }
  $ad = ($Hesap -replace "@", "-at-" -replace "[^a-zA-Z0-9\-]", "_")
  $yol = Join-Path $Cikti "alt-hover-$ad.png"
  if ($en.bmp) {
    $en.bmp.Save($yol, [System.Drawing.Imaging.ImageFormat]::Png)
    "HTTP=$durum fark=$($en.f) -> $yol"
    $en.bmp.Dispose()
  }
  $temel.Dispose()
}
finally {
  [void][KervanHover]::SetCursorPos($eski.X, $eski.Y)
  "imleç geri: $($eski.X),$($eski.Y)"
  if ($tasindi) {
    [void][KervanHover]::SetWindowPos($pencere, [IntPtr]::Zero, $eskiR.L, $eskiR.T, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010)
    "pencere geri: @$($eskiR.L),$($eskiR.T)"
  }
}
