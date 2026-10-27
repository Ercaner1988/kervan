#requires -Version 5.1
<#
.SYNOPSIS
  Kervan — bir hesabın "Diğer araçlar" ALT MENÜSÜNÜ açıp oturmuş kareyi yakalar.

.DESCRIPTION
  NEDEN VAR: Araç menüsünün üst düzeyinde `Deep Research` yalnız bazı
  hesaplarda görünüyor. Diğerlerinde iç içe (Diğer araçlar altında) olabilir.
  Alt menüyü seçiciyle okumak mümkün değil (ayrı bir katman, animasyonlu),
  bu yüzden ölçüm ucunun `dene=` tıklamasını kullanıp ekranı yakalıyoruz.

  Zamanlama: `dene` menüyü açar, öğeye basar, sonra 3 kez yeniden okumayı
  dener ve en sonunda Escape ile kapatır. Alt menü bu arada ~1.2 sn açık
  kalır. Her pencereden sürekli kare alınır, en büyük değişimli kare saklanır.

.EXAMPLE
  powershell -NoProfile -File araclar\hesap-alt-menu-yaka.ps1 -Hesap ercaner1988@gmail.com -AltMenu "Diğer araçlar" -Cikti olc-ekran
#>
param(
  [Parameter(Mandatory = $true)][string]$Hesap,
  [string]$AltMenu = "Diğer araçlar",
  [string]$Cikti = "olc-ekran",
  [string]$Etiket = "",
  # Seçici, tıklamadan ÖNCE eşleşmemeli; yoksa eklenti menüyü hiç açmaz
  # (kapalı menü DOM'da duruyor) ve tıklama görünmez bir öğeye gider.
  [string]$Secici = '.cdk-overlay-pane:has([role="menuitemcheckbox"])'
)

$ErrorActionPreference = "Stop"
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class KervanAlt {
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
[void][KervanAlt]::SetProcessDPIAware()

$defter = Join-Path $env:LOCALAPPDATA "kervan\olcum.json"
if (-not (Test-Path $Cikti)) { [void](New-Item -ItemType Directory -Path $Cikti) }

function ZenPencereleri {
  $liste = New-Object System.Collections.ArrayList
  $cb = [KervanAlt+EnumWindowsProc]{
    param($h, $l)
    $t = New-Object System.Text.StringBuilder 512
    [void][KervanAlt]::GetWindowTextW($h, $t, 512)
    if ($t.ToString() -like "*Google Gemini*Zen*" -and [KervanAlt]::IsWindowVisible($h)) {
      $r = New-Object KervanAlt+RECT
      [void][KervanAlt]::GetWindowRect($h, [ref]$r)
      [void]$liste.Add([pscustomobject]@{
        h = $h; x = $r.L; y = $r.T
        w = $r.R - $r.L; hh = $r.B - $r.T
        kucuk = [KervanAlt]::IsIconic($h)
      })
    }
    return $true
  }
  [void][KervanAlt]::EnumWindows($cb, [IntPtr]::Zero)
  return $liste
}
function KareAl($p) {
  $bmp = New-Object System.Drawing.Bitmap ([Math]::Max($p.w, 1)), ([Math]::Max($p.hh, 1))
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc(); [void][KervanAlt]::PrintWindow($p.h, $hdc, 2); $g.ReleaseHdc($hdc); $g.Dispose()
  return $bmp
}
function Fark($a, $b) {
  if ($null -eq $a -or $null -eq $b) { return 0 }
  if ($a.Width -ne $b.Width -or $a.Height -ne $b.Height) { return 999999 }
  $f = 0
  for ($x = 4; $x -lt $a.Width; $x += 9) {
    for ($y = 4; $y -lt $a.Height; $y += 9) {
      $c1 = $a.GetPixel($x, $y); $c2 = $b.GetPixel($x, $y)
      if ([Math]::Abs($c1.R - $c2.R) + [Math]::Abs($c1.G - $c2.G) + [Math]::Abs($c1.B - $c2.B) -gt 26) { $f++ }
    }
  }
  return $f
}
function Slug($h) { return ($h -replace "@", "-at-" -replace "[^a-zA-Z0-9\-]", "_") }

$pen = ZenPencereleri
if ($pen.Count -eq 0) { throw "hiç Gemini penceresi bulunamadı" }

$geriKucult = @()
foreach ($p in $pen) {
  if ($p.kucuk) {
    [void][KervanAlt]::ShowWindow($p.h, 4)
    Start-Sleep -Milliseconds 250
    $r = New-Object KervanAlt+RECT
    [void][KervanAlt]::GetWindowRect($p.h, [ref]$r)
    $p.w = $r.R - $r.L; $p.hh = $r.B - $r.T
    $geriKucult += $p.h
  }
}

$secici = '.cdk-overlay-pane:has([role="menuitemcheckbox"])'
$q = "hesap=" + [uri]::EscapeDataString($Hesap) +
     "&secici=" + [uri]::EscapeDataString($Secici) +
     "&dugme=" + [uri]::EscapeDataString('button[aria-label*="araç" i]') +
     "&dene=" + [uri]::EscapeDataString($AltMenu)
$url = "http://127.0.0.1:20129/kervan/olc?$q"

try {
  $temel = @{}
  foreach ($p in $pen) { $temel[$p.h] = KareAl $p }
  $is = Start-Job -ScriptBlock {
    param($u)
    try { (Invoke-WebRequest -Uri $u -UseBasicParsing -TimeoutSec 120).StatusCode } catch { "hata" }
  } -ArgumentList $url

  $en = @{}
  foreach ($p in $pen) { $en[$p.h] = @{ f = -1; bmp = $null } }
  $bas = Get-Date
  while ($is.State -eq 'Running' -and ((Get-Date) - $bas).TotalSeconds -lt 30) {
    foreach ($p in $pen) {
      $b = KareAl $p
      $f = Fark $temel[$p.h] $b
      if ($f -gt $en[$p.h].f) {
        if ($en[$p.h].bmp) { $en[$p.h].bmp.Dispose() }
        $en[$p.h] = @{ f = $f; bmp = $b }
      } else { $b.Dispose() }
    }
    Start-Sleep -Milliseconds 100
  }
  $durum = Receive-Job $is -Wait -AutoRemoveJob
  $kaz = $en.GetEnumerator() | Sort-Object { $_.Value.f } -Descending | Select-Object -First 1
  $ad = if ($Etiket) { $Etiket } else { Slug $Hesap }
  $yol = Join-Path $Cikti "alt-$ad.png"
  if ($kaz -and $kaz.Value.bmp) {
    $kaz.Value.bmp.Save($yol, [System.Drawing.Imaging.ImageFormat]::Png)
    "HTTP=$durum hwnd=$($kaz.Key) fark=$($kaz.Value.f) -> $yol"
  } else { "YAKALANAMADI HTTP=$durum" }
  foreach ($e in $en.Values) { if ($e.bmp) { $e.bmp.Dispose() } }
  foreach ($p in $pen) { $temel[$p.h].Dispose() }
}
finally {
  foreach ($h in $geriKucult) { [void][KervanAlt]::ShowWindow($h, 6) }
}
