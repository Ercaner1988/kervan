#requires -Version 5.1
<#
.SYNOPSIS
  Kervan — bağlı hesapların Gemini ekranını yakalar, menüyü açık hâlde
  "oturmuş kare" seçer ve dörtlü/üçlü pano görselleri üretir.

.DESCRIPTION
  NEDEN VAR: Menü metnini seçiciyle okumak güvenilmez. `[role="menu"]`
  düğümleri sayfada kapalı hâlde de duruyor ve menü animasyonu sırasında
  eksik metin dönüyor. Ekran görüntüsü tam menüyü veriyor, ama yalnız
  animasyon bittikten sonra alınırsa. Bu betik her hesap için menüyü açar,
  pencere karelerini örnekler ve DEĞİŞİMİN DURULDUĞU kareyi seçer.

  Pencere tespiti: başlığında "Google Gemini" ve "Zen" geçen görünür
  pencereler. PrintWindow ile çizilir; pencere önde olmak zorunda değil.
  Küçültülmüş pencere çizilemez: geçici olarak geri yüklenir, sonunda
  yeniden küçültülür.

.PARAMETER Cikti
  Görsellerin yazılacağı klasör. Yoksa oluşturulur.

.PARAMETER Siralama
  Pano görsellerinde hesap sırası. Boşsa ölçüm defterindeki sıra kullanılır.

.EXAMPLE
  powershell -NoProfile -File araclar\hesap-ekran-yaka.ps1 -Cikti olc-ekran
#>
param(
  [string]$Cikti = "olc-ekran",
  [string[]]$Siralama = @(),
  [int]$PanoBoyu = 4
)

$ErrorActionPreference = "Stop"
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class KervanYaka {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr l);
  public delegate bool EnumWindowsProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RECT { public int L, T, R, B; }
}
"@ -ErrorAction SilentlyContinue
Add-Type -AssemblyName System.Drawing
[void][KervanYaka]::SetProcessDPIAware()

$kok = Split-Path -Parent $PSScriptRoot
$defter = Join-Path $env:LOCALAPPDATA "kervan\olcum.json"

function ZenPencereleri {
  $liste = New-Object System.Collections.ArrayList
  $cb = [KervanYaka+EnumWindowsProc]{
    param($h, $l)
    $t = New-Object System.Text.StringBuilder 512
    [void][KervanYaka]::GetWindowTextW($h, $t, 512)
    if ($t.ToString() -like "*Google Gemini*Zen*" -and [KervanYaka]::IsWindowVisible($h)) {
      $r = New-Object KervanYaka+RECT
      [void][KervanYaka]::GetWindowRect($h, [ref]$r)
      [void]$liste.Add([pscustomobject]@{
        h = $h
        x = $r.L; y = $r.T
        w = $r.R - $r.L; hh = $r.B - $r.T
        kucuk = [KervanYaka]::IsIconic($h)
      })
    }
    return $true
  }
  [void][KervanYaka]::EnumWindows($cb, [IntPtr]::Zero)
  return $liste
}

function KareAl($p) {
  $bmp = New-Object System.Drawing.Bitmap ([Math]::Max($p.w, 1)), ([Math]::Max($p.hh, 1))
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc()
  [void][KervanYaka]::PrintWindow($p.h, $hdc, 2)
  $g.ReleaseHdc($hdc); $g.Dispose()
  return $bmp
}

# Fark: seyrek örnekleme yeterli; tam piksel taraması dakikalara çıkar.
function Fark($a, $b) {
  if ($null -eq $a -or $null -eq $b) { return 0 }
  if ($a.Width -ne $b.Width -or $a.Height -ne $b.Height) { return 999999 }
  $adim = 12
  $f = 0
  for ($x = 4; $x -lt $a.Width; $x += $adim) {
    for ($y = 4; $y -lt $a.Height; $y += $adim) {
      $c1 = $a.GetPixel($x, $y); $c2 = $b.GetPixel($x, $y)
      if ([Math]::Abs($c1.R - $c2.R) + [Math]::Abs($c1.G - $c2.G) + [Math]::Abs($c1.B - $c2.B) -gt 26) { $f++ }
    }
  }
  return $f
}

function Slug($h) { return ($h -replace "@", "-at-" -replace "[^a-zA-Z0-9\-]", "_") }

# Menüyü açar ama HİÇBİR SEÇİCİ eşleşmez: eklenti menüyü açık bırakıp
# zaman aşımına düşer. Bu aralık yakalama penceremizdir.
function MenuAcBaslat($hesap) {
  $q = "hesap=" + [uri]::EscapeDataString($hesap) +
       "&secici=" + [uri]::EscapeDataString("#kervan-yaka-eslesmez") +
       "&dugme=" + [uri]::EscapeDataString('button[aria-label*="araç" i]')
  $url = "http://127.0.0.1:20129/kervan/olc?$q"
  return Start-Job -ScriptBlock {
    param($u)
    try { (Invoke-WebRequest -Uri $u -UseBasicParsing -TimeoutSec 120).StatusCode } catch { "hata" }
  } -ArgumentList $url
}

if (-not (Test-Path $defter)) { throw "ölçüm defteri yok: $defter" }
$hesaplar = (Get-Content $defter -Raw -Encoding UTF8 | ConvertFrom-Json) | ForEach-Object { $_.hesap }
if ($Siralama.Count -gt 0) {
  $hesaplar = $Siralama | Where-Object { $hesaplar -contains $_ }
  if ($hesaplar.Count -eq 0) { throw "sıralamadaki hesapların hiçbiri defterde yok" }
}

$pen = ZenPencereleri
if ($pen.Count -eq 0) { throw "hiç Gemini penceresi bulunamadı (Zen açık mı?)" }
if (-not (Test-Path $Cikti)) { [void](New-Item -ItemType Directory -Path $Cikti) }

# Küçültülmüş pencereleri geçici olarak geri yükle: PrintWindow onları çizemez.
$geriKucult = @()
foreach ($p in $pen) {
  if ($p.kucuk) {
    [void][KervanYaka]::ShowWindow($p.h, 4)  # SW_SHOWNOACTIVATE
    Start-Sleep -Milliseconds 250
    $r = New-Object KervanYaka+RECT
    [void][KervanYaka]::GetWindowRect($p.h, [ref]$r)
    $p.w = $r.R - $r.L; $p.hh = $r.B - $r.T
    $geriKucult += $p.h
  }
}

$sonuc = @()
try {
  $i = 0
  foreach ($hesap in $hesaplar) {
    $i++
    $temel = @{}
    foreach ($p in $pen) { $temel[$p.h] = KareAl $p }

    $is = MenuAcBaslat $hesap
    $en = @{}
    foreach ($p in $pen) { $en[$p.h] = @{ f = -1; bmp = $null } }
    $gecmis = @{}
    foreach ($p in $pen) { $gecmis[$p.h] = New-Object System.Collections.ArrayList }

    $bas = Get-Date
    while ($is.State -eq 'Running' -and ((Get-Date) - $bas).TotalSeconds -lt 25) {
      foreach ($p in $pen) {
        $b = KareAl $p
        $f = Fark $temel[$p.h] $b
        [void]$gecmis[$p.h].Add($f)
        if ($f -gt $en[$p.h].f) {
          if ($en[$p.h].bmp) { $en[$p.h].bmp.Dispose() }
          $en[$p.h] = @{ f = $f; bmp = $b }
        } else { $b.Dispose() }
      }
      Start-Sleep -Milliseconds 120
    }
    $durum = Receive-Job $is -Wait -AutoRemoveJob

    $kaz = $en.GetEnumerator() | Sort-Object { $_.Value.f } -Descending | Select-Object -First 1
    $slug = Slug $hesap
    $yol = Join-Path $Cikti "$slug.png"
    $gec = $gecmis[$kaz.Key]
    # Oturma ölçüsü: en yüksek farka ulaşıldıktan sonra kaç örnek boyunca
    # değişim %5'in altında kaldı. Küçük değer = kare hâlâ oynuyor.
    $zirve = ($gec | Measure-Object -Maximum).Maximum
    $dizi = @($gec | ForEach-Object { [int]$_ })
    $zirveIdx = 0
    for ($k = 0; $k -lt $dizi.Count; $k++) { if ($dizi[$k] -eq [int]$zirve) { $zirveIdx = $k; break } }
    $sonra = @($dizi | Select-Object -Skip $zirveIdx)
    $oturdu = 0
    foreach ($d in $sonra) { if ([Math]::Abs($d - $zirve) -le [Math]::Max(2, $zirve * 0.05)) { $oturdu++ } else { break } }

    if ($kaz.Value.bmp) {
      $kaz.Value.bmp.Save($yol, [System.Drawing.Imaging.ImageFormat]::Png)
      "[{0}/{1}] {2,-30} hwnd={3} fark={4} oturdu={5} ornek={6} -> {7}" -f `
        $i, $hesaplar.Count, $hesap, $kaz.Key, $kaz.Value.f, $oturdu, $gec.Count, $yol
      $sonuc += [pscustomobject]@{ hesap = $hesap; hwnd = $kaz.Key; fark = $kaz.Value.f; oturdu = $oturdu; yol = $yol }
    }

    foreach ($e in $en.Values) { if ($e.bmp) { $e.bmp.Dispose() } }
    foreach ($p in $pen) { $temel[$p.h].Dispose() }
    Start-Sleep -Milliseconds 400
  }
}
finally {
  foreach ($h in $geriKucult) { [void][KervanYaka]::ShowWindow($h, 6) }  # SW_MINIMIZE
}

$sonuc | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $Cikti "esleme.json") -Encoding UTF8

# --- Pano: hesap başına bir karo, dörderli sayfalar ---
$kutu = 2
$sutun = 2
$karoG = 640
$etiketH = 26

function Pano($grup, $ad) {
  $satirlar = [Math]::Ceiling($grup.Count / $sutun)
  $ilk = [System.Drawing.Image]::FromFile($grup[0].yol)
  $oran = $karoG / $ilk.Width
  $karoH = [int]($ilk.Height * $oran)
  $ilk.Dispose()
  $g = $karoG * $sutun
  $y = $karoH * $satirlar
  $bmp = New-Object System.Drawing.Bitmap $g, $y
  $gr = [System.Drawing.Graphics]::FromImage($bmp)
  $gr.Clear([System.Drawing.Color]::FromArgb(16, 16, 20))
  $gr.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $font = New-Object System.Drawing.Font "Segoe UI", 12, ([System.Drawing.FontStyle]::Bold)
  $firca = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(235, 235, 240))
  $idx = 0
  foreach ($s in $grup) {
    $im = [System.Drawing.Image]::FromFile($s.yol)
    $cx = ($idx % $sutun) * $karoG
    $cy = [Math]::Floor($idx / $sutun) * $karoH
    $gr.DrawImage($im, $cx, $cy + $etiketH, $karoG, $karoH - $etiketH)
    $gr.DrawString($s.hesap, $font, $firca, ($cx + 6), ($cy + 4))
    $im.Dispose()
    $idx++
  }
  $gr.Dispose()
  $yol = Join-Path $Cikti "pano-$ad.png"
  $bmp.Save($yol, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  return $yol
}

$sayfa = 1
for ($b = 0; $b -lt $sonuc.Count; $b += $PanoBoyu) {
  $grup = @($sonuc | Select-Object -Skip $b -First $PanoBoyu)
  $yol = Pano $grup $sayfa
  "pano ${sayfa}: $($grup.Count) hesap -> $yol"
  $sayfa++
}
