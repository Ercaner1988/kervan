#requires -Version 5.1
<#
.SYNOPSIS
  Kervan — araç menüsünü tarayarak ALT MENÜYÜ (Diğer araçlar) yakalar.

.DESCRIPTION
  NEDEN TARAMA: Alt menü yalnız ÜZERİNE GELİNCE açılıyor ve "Diğer araçlar"
  satırının yeri hesaba göre değişiyor (menüde 3 ile 8 satır arası). Sabit
  koordinat bir hesapta tutup diğerinde boşa gidiyor. Bu betik menü sütunu
  boyunca imleci adım adım indirir; her adımda kare alır ve DEĞİŞEN BÖLGENİN
  GENİŞLİĞİ ana menünün sağına taşarsa alt menü açıldı der.

  Alt menü açıkken elde edilen kare `alt-<hesap>.png` olarak yazılır.

.PARAMETER Hesap
  Ölçüm defterindeki hesap.

.PARAMETER Hwnd
  Pencere tanıtıcısı — `hesap-ekran-yaka.ps1` eşlemesinden.

.PARAMETER Tasi
  Pencereyi geçici olarak görünür alana taşır, sonunda geri koyar.

.EXAMPLE
  powershell -NoProfile -File araclar\hesap-alt-menu-tara.ps1 `
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
public class KervanTara {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RECT { public int L, T, R, B; }
  public struct POINT { public int X, Y; }
}
"@ -ErrorAction SilentlyContinue
Add-Type -AssemblyName System.Drawing
[void][KervanTara]::SetProcessDPIAware()

$pencere = [IntPtr]$Hwnd
$r = New-Object KervanTara+RECT
[void][KervanTara]::GetWindowRect($pencere, [ref]$r)
if (($r.R - $r.L) -lt 300 -or ($r.B - $r.T) -lt 300) {
  [void][KervanTara]::ShowWindow($pencere, 4)
  Start-Sleep -Milliseconds 400
  [void][KervanTara]::GetWindowRect($pencere, [ref]$r)
}
$w = $r.R - $r.L; $h = $r.B - $r.T
if ($w -lt 300 -or $h -lt 300) { throw "pencere hâlâ çok küçük (${w}x${h})" }
$eskiR = New-Object KervanTara+RECT
$eskiR.L = $r.L; $eskiR.T = $r.T; $eskiR.R = $r.R; $eskiR.B = $r.B

function KareAl {
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc(); [void][KervanTara]::PrintWindow($pencere, $hdc, 2); $g.ReleaseHdc($hdc); $g.Dispose()
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

$eski = New-Object KervanTara+POINT
[void][KervanTara]::GetCursorPos([ref]$eski)
$tasindi = $false
if ($Tasi) {
  [void][KervanTara]::SetWindowPos($pencere, [IntPtr]::Zero, 40, 20, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010)
  Start-Sleep -Milliseconds 400
  [void][KervanTara]::GetWindowRect($pencere, [ref]$r)
  $tasindi = $true
}

$q = "hesap=" + [uri]::EscapeDataString($Hesap) +
     "&secici=" + [uri]::EscapeDataString("#kervan-tara-eslesmez") +
     "&dugme=" + [uri]::EscapeDataString('button[aria-label*="araç" i]')
$url = "http://127.0.0.1:20129/kervan/olc?$q"
$ad = ($Hesap -replace "@", "-at-" -replace "[^a-zA-Z0-9\-]", "_")
if (-not (Test-Path $Cikti)) { [void](New-Item -ItemType Directory -Path $Cikti) }
$yol = Join-Path $Cikti "alt-$ad.png"
$sonuc = "YAKALANAMADI"

try {
  $temel = KareAl
  $is = Start-Job -ScriptBlock {
    param($u)
    try { (Invoke-WebRequest -Uri $u -UseBasicParsing -TimeoutSec 120).StatusCode } catch { "hata" }
  } -ArgumentList $url

  # Menü açık kare: eşiği aşan VE 300 ms sonra hâlâ aynı kalan kare.
  # "En büyük değişim" kare seçilirse menünün KAPANDIĞI an kazanıyor.
  $acik = $null
  $bas = Get-Date
  while (((Get-Date) - $bas).TotalSeconds -lt 12) {
    $b = KareAl
    if ((Fark $temel $b) -gt 300) {
      Start-Sleep -Milliseconds 300
      $b2 = KareAl
      $oynama = Fark $b $b2
      if ($oynama -lt 120) {
        $acik = $b2; $enIyi = Fark $temel $b2
        $b.Dispose()
        break
      }
      $b2.Dispose()
    }
    $b.Dispose()
    Start-Sleep -Milliseconds 80
  }
  if (-not $acik) { throw "menü açılmadı / oturmadı (12 sn)" }

  $s = Sinirlar $temel $acik
  if (($s.y2 - $s.y1) -lt 120) { throw "menü kutusu çok kısa ($($s.y2 - $s.y1) px) — menü çizilmemiş" }
  "menu=($($s.x1),$($s.y1))-($($s.x2),$($s.y2)) icerik=$($s.n) degisim=$enIyi"

  # Tarama yalnız SON satırlarda: "Diğer araçlar" her zaman menünün sonunda.
  # Menü ~8 sn açık kalıyor; tüm sütunu taramak süreyi aşıyor ve menü kapanınca
  # "en büyük değişim" yanlışlıkla KAPANMIŞ kareyi seçiyordu.
  # Ölçüt de daraltıldı: yalnız ana menünün SAĞINA taşan değişim alt menüdür.
  $en = @{ f = -1; bmp = $null; y = 0; sag = 0 }
  $altSol = $s.x2 - 8
  for ($y = $s.y2 - 230; $y -le $s.y2 - 6; $y += 22) {
    [void][KervanTara]::SetCursorPos(($r.L + $s.x1 + 70), ($r.T + $y))
    Start-Sleep -Milliseconds 200
    $b = KareAl
    # Sağ bölge farkı: alt menü açıldıysa burada büyük değişim olur.
    $fs = 0; $sagEn = -1
    for ($x = [Math]::Max($altSol, 0); $x -lt $b.Width; $x += 7) {
      for ($yy = 3; $yy -lt $b.Height; $yy += 7) {
        $c1 = $acik.GetPixel($x, $yy); $c2 = $b.GetPixel($x, $yy)
        if ([Math]::Abs($c1.R - $c2.R) + [Math]::Abs($c1.G - $c2.G) + [Math]::Abs($c1.B - $c2.B) -gt 26) {
          $fs++
          if ($x -gt $sagEn) { $sagEn = $x }
        }
      }
    }
    if ($fs -gt $en.f) { if ($en.bmp) { $en.bmp.Dispose() }; $en = @{ f = $fs; bmp = $b; y = $y; sag = $sagEn } }
    else { $b.Dispose() }
  }
  $durum = Receive-Job $is -Wait -AutoRemoveJob

  if ($en.bmp) {
    $en.bmp.Save($yol, [System.Drawing.Imaging.ImageFormat]::Png)
    $en.bmp.Dispose()
    $sag = if ($en.f -gt 400 -and $en.sag -gt $altSol + 60) { "ALT_MENU_ACIK" } else { "alt_menu_yok" }
    $sonuc = "HTTP=$durum satir=$($en.y) $sag sagFark=$($en.f) enSag=$($en.sag) anaSag=$($s.x2) -> $yol"
  } else { $sonuc = "HTTP=$durum TARAMA_BOS" }
  $acik.Dispose(); $temel.Dispose()
}
finally {
  [void][KervanTara]::SetCursorPos($eski.X, $eski.Y)
  if ($tasindi) { [void][KervanTara]::SetWindowPos($pencere, [IntPtr]::Zero, $eskiR.L, $eskiR.T, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010) }
}
$sonuc
