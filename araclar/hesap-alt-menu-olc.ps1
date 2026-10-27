#requires -Version 5.1
<#
.SYNOPSIS
  Kervan — "Diğer araçlar" alt menüsünü ÖLÇÜLEN koordinatla açar ve yakalar.

.DESCRIPTION
  NEDEN KOORDİNAT TAHMİN EDİLMİYOR: Menü, pencerenin boyutuna ve yakınlaştırma
  oranına göre yer değiştiriyor; sabit (dx,dy) bir pencerede tutup diğerinde
  boşa gidiyor. Bu betik menü açıkken alınan kareyi temel kareyle karşılaştırıp
  DEĞİŞEN BÖLGENİN SINIRLARINI ölçer; imleci o bölgenin alt satırına koyar.
  Menünün son satırı her hesapta "Diğer araçlar" (sağ ok işaretli).

  Alt menü TIKLAMAYLA değil ÜZERİNE GELİNCE açılıyor; `dene=` tıklaması
  menüyü kapatıyor. Bu yüzden gerçek imleç kullanılıyor.

.PARAMETER Hesap
  Ölçüm defterindeki hesap (ör. ercaner1988@gmail.com).

.PARAMETER Hwnd
  Pencere tanıtıcısı — `hesap-ekran-yaka.ps1` eşlemesinden.

.PARAMETER Tasi
  Pencereyi geçici olarak görünür alana taşır (imleç ekran dışına taşamaz),
  sonunda eski yerine koyar.

.EXAMPLE
  powershell -NoProfile -File araclar\hesap-alt-menu-olc.ps1 `
    -Hesap ercaner1988@gmail.com -Hwnd 18879692 -Tasi
#>
param(
  [Parameter(Mandatory = $true)][string]$Hesap,
  [Parameter(Mandatory = $true)][long]$Hwnd,
  [string]$Cikti = "olc-ekran",
  [switch]$Tasi
)

$ErrorActionPreference = "Stop"
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class KervanOlc {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RECT { public int L, T, R, B; }
  public struct POINT { public int X, Y; }
}
"@ -ErrorAction SilentlyContinue
Add-Type -AssemblyName System.Drawing
[void][KervanOlc]::SetProcessDPIAware()

$pencere = [IntPtr]$Hwnd
$r = New-Object KervanOlc+RECT
if (-not [KervanOlc]::GetWindowRect($pencere, [ref]$r)) { throw "pencere bulunamadı: $Hwnd" }
$w = $r.R - $r.L; $h = $r.B - $r.T
if ($w -lt 200 -or $h -lt 200) { throw "pencere çok küçük (${w}x${h}) — küçültülmüş olabilir" }
$eskiR = New-Object KervanOlc+RECT
$eskiR.L = $r.L; $eskiR.T = $r.T; $eskiR.R = $r.R; $eskiR.B = $r.B

function KareAl {
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc(); [void][KervanOlc]::PrintWindow($pencere, $hdc, 2); $g.ReleaseHdc($hdc); $g.Dispose()
  return $bmp
}
function Fark($a, $b) {
  $f = 0
  for ($x = 3; $x -lt $a.Width; $x += 7) {
    for ($y = 3; $y -lt $a.Height; $y += 7) {
      $c1 = $a.GetPixel($x, $y); $c2 = $b.GetPixel($x, $y)
      if ([Math]::Abs($c1.R - $c2.R) + [Math]::Abs($c1.G - $c2.G) + [Math]::Abs($c1.B - $c2.B) -gt 26) { $f++ }
    }
  }
  return $f
}
# Değişen bölgenin sınırları — menünün gerçek yerini görüntüden ölçer.
function Sinirlar($a, $b) {
  $x1 = 999999; $y1 = 999999; $x2 = -1; $y2 = -1; $n = 0
  for ($x = 3; $x -lt $a.Width; $x += 3) {
    for ($y = 3; $y -lt $a.Height; $y += 3) {
      $c1 = $a.GetPixel($x, $y); $c2 = $b.GetPixel($x, $y)
      if ([Math]::Abs($c1.R - $c2.R) + [Math]::Abs($c1.G - $c2.G) + [Math]::Abs($c1.B - $c2.B) -gt 26) {
        $n++
        if ($x -lt $x1) { $x1 = $x }; if ($x -gt $x2) { $x2 = $x }
        if ($y -lt $y1) { $y1 = $y }; if ($y -gt $y2) { $y2 = $y }
      }
    }
  }
  return [pscustomobject]@{ x1 = $x1; y1 = $y1; x2 = $x2; y2 = $y2; n = $n }
}

$eski = New-Object KervanOlc+POINT
[void][KervanOlc]::GetCursorPos([ref]$eski)
$tasindi = $false
if ($Tasi) {
  [void][KervanOlc]::SetWindowPos($pencere, [IntPtr]::Zero, 60, 40, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010)
  Start-Sleep -Milliseconds 400
  [void][KervanOlc]::GetWindowRect($pencere, [ref]$r)
  $tasindi = $true
}

$q = "hesap=" + [uri]::EscapeDataString($Hesap) +
     "&secici=" + [uri]::EscapeDataString("#kervan-olc-eslesmez") +
     "&dugme=" + [uri]::EscapeDataString('button[aria-label*="araç" i]')
$url = "http://127.0.0.1:20129/kervan/olc?$q"

$sonuc = "YAKALANAMADI"
try {
  $temel = KareAl
  $is = Start-Job -ScriptBlock {
    param($u)
    try { (Invoke-WebRequest -Uri $u -UseBasicParsing -TimeoutSec 120).StatusCode } catch { "hata" }
  } -ArgumentList $url

  # Menünün çizilmesini bekle.
  $acik = $null
  $bas = Get-Date
  while (((Get-Date) - $bas).TotalSeconds -lt 10) {
    $b = KareAl
    if ((Fark $temel $b) -gt 40) { $acik = $b; break }
    $b.Dispose()
    Start-Sleep -Milliseconds 120
  }
  if (-not $acik) { throw "menü açılmadı (10 sn)" }

  $s = Sinirlar $temel $acik
  # Son satır: menünün alt kenarından bir satır yukarısı.
  $satirY = [int]($s.y2 - 7)
  $satirX = [int]($s.x1 + 55)
  [void][KervanOlc]::SetCursorPos(($r.L + $satirX), ($r.T + $satirY))
  "menu kutusu=($($s.x1),$($s.y1))-($($s.x2),$($s.y2)) imleç=+$satirX,+$satirY"

  $en = @{ f = -1; bmp = $null }
  $bas = Get-Date
  while (((Get-Date) - $bas).TotalSeconds -lt 14) {
    $b = KareAl
    $f = Fark $temel $b
    if ($f -gt $en.f) { if ($en.bmp) { $en.bmp.Dispose() }; $en = @{ f = $f; bmp = $b } } else { $b.Dispose() }
    Start-Sleep -Milliseconds 120
  }
  $durum = Receive-Job $is -Wait -AutoRemoveJob
  $acik.Dispose()

  if (-not (Test-Path $Cikti)) { [void](New-Item -ItemType Directory -Path $Cikti) }
  $ad = ($Hesap -replace "@", "-at-" -replace "[^a-zA-Z0-9\-]", "_")
  $yol = Join-Path $Cikti "alt-$ad.png"
  if ($en.bmp) {
    $en.bmp.Save($yol, [System.Drawing.Imaging.ImageFormat]::Png)
    $en.bmp.Dispose()
    $sonuc = "HTTP=$durum fark=$($en.f) -> $yol"
  }
  $temel.Dispose()
}
finally {
  [void][KervanOlc]::SetCursorPos($eski.X, $eski.Y)
  if ($tasindi) {
    [void][KervanOlc]::SetWindowPos($pencere, [IntPtr]::Zero, $eskiR.L, $eskiR.T, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010)
  }
}
$sonuc
