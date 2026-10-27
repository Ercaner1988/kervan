# Kervan — Gemini hesap pencereleri (Windows)
#
# NE İŞE YARAR: her Gemini hesabı kendi Zen profilinde (`-profile <dizin>`)
# çalışır. Bu betik o profilleri tek tek açar/kapatır ve köprünün hangi
# hesapları gördüğünü söyler.
#
# NEDEN GEREKLİ: 7 hesabı birden açmak RAM'i tüketir (profil başına ~400-600 MB,
# makinede ~23 GB). Hesap yalnız gerektiğinde açılır; boşta kapatma zaten
# Kervan tarafında 15 dk.
#
# KİMLİK: profil dizinindeki eklenti kopyasında `kervan-profil.json`
# ({"ad": "<hesap>"}) durur. O dosya olmadan ayrı profillerin hepsi aynı
# `cookieStoreId`yi bildirir ve Kervan'da TEK kayda çöker. Dosyayı silme;
# eklenti güncellenirken de korunur (yalnız *.js + manifest.json kopyalanır).
#
# KULLANIM
#   .\gemini-hesap.ps1 -Durum
#   .\gemini-hesap.ps1 -Ac ercaner1988@gmail.com
#   .\gemini-hesap.ps1 -Kapat kralarthas@gmail.com
#   .\gemini-hesap.ps1 -Hepsi            # kervan.toml'daki tüm Gemini hesapları
param(
    [string]$Ac,
    [string]$Kapat,
    [string]$Onar,
    [switch]$Durum,
    [switch]$Hepsi
)

$ErrorActionPreference = "Stop"
$repoRoot = $PSScriptRoot
$profilesRoot = Join-Path $env:LOCALAPPDATA "kervan\profiles"
$zen = "C:\Program Files\Zen Browser\zen.exe"
$kervanUc = "http://127.0.0.1:20129/eklenti/sekmeler"
$geminiAdres = "https://gemini.google.com/app"

function GeminiHesaplari {
    # Tek kaynak kervan.toml: `name = "..."` + `providers = [...]` çiftleri.
    # TOML ayrıştırıcısı çağırmaya değmez; biçim bu depoda sabit.
    $satirlar = Get-Content (Join-Path $repoRoot "kervan.toml")
    $ad = $null
    foreach ($s in $satirlar) {
        if ($s -match '^\s*name\s*=\s*"([^"]+)"') { $ad = $matches[1]; continue }
        if ($ad -and $s -match '^\s*providers\s*=\s*\[([^\]]*)\]') {
            if ($matches[1] -match 'gemini') { $ad }
            $ad = $null
        }
    }
}

function ProfilDizini([string]$hesap) { Join-Path $profilesRoot $hesap }

function AcikMi([string]$hesap) {
    $dizin = ProfilDizini $hesap
    $surec = Get-CimInstance Win32_Process -Filter "Name='zen.exe'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -and $_.CommandLine -match [regex]::Escape("-profile $dizin ") }
    return [bool]$surec
}

function HesapAc([string]$hesap) {
    $dizin = ProfilDizini $hesap
    if (-not (Test-Path $dizin)) {
        Write-Host "HATA: profil dizini yok: $dizin"
        return
    }
    $kimlik = Join-Path $dizin "extensions\kervan-bridge@ercan.dev\kervan-profil.json"
    if (-not (Test-Path $kimlik)) {
        Write-Host "UYARI: kimlik dosyası yok ($kimlik). Hesap 'firefox-default' olarak kaydolur ve diğerleriyle çakışır."
    }
    if (AcikMi $hesap) { Write-Host "$hesap zaten açık."; return }
    Start-Process $zen -ArgumentList @("-no-remote", "-profile", $dizin, $geminiAdres)
    Write-Host "$hesap açılıyor..."
}

function HesapKapat([string]$hesap) {
    $dizin = ProfilDizini $hesap
    $surecler = Get-CimInstance Win32_Process -Filter "Name='zen.exe'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -and $_.CommandLine -match [regex]::Escape("-profile $dizin ") }
    if (-not $surecler) { Write-Host "$hesap açık değil."; return }
    $surecler | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Write-Host "$hesap kapatıldı."
}

# Eklenti kaydı bayatladığında (sideload sonrası `startupData` boş kalır ve arka
# plan betiği hiç çalışmaz) zemini temizler. Ölçüldü 2026-10-24: kralarthas ve
# erercan1988 profilleri bu yüzden köprüye hiç bağlanmadı; kayıt dosyaları
# silinip yeniden açılınca bağlandılar.
function Onar([string]$hesap) {
    if (AcikMi $hesap) {
        Write-Host "UYARI: $hesap açık. Önce -Kapat ile kapat."
        return
    }
    $dizin = ProfilDizini $hesap
    if (-not (Test-Path $dizin)) { Write-Host "HATA: profil dizini yok: $dizin"; return }
    Remove-Item (Join-Path $dizin "extensions.json") -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $dizin "addonStartup.json.lz4") -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $dizin "startupCache") -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "${hesap}: eklenti kaydı sıfırlandı."
}

function DurumYaz {
    try {
        $y = Invoke-WebRequest -Uri $kervanUc -UseBasicParsing -TimeoutSec 5
        $govde = $y.Content | ConvertFrom-Json
        $bagli = @($govde.hesaplar | Where-Object { $_ -like 'gemini:*' })
        Write-Host "Kervan'ın gördüğü Gemini hesapları ($($bagli.Count)):"
        if ($bagli.Count -eq 0) { Write-Host "  (yok)" } else { $bagli | ForEach-Object { Write-Host "  $_" } }
    } catch {
        Write-Host "Kervan'a ulaşılamadı ($kervanUc): $($_.Exception.Message)"
    }
    Write-Host ""
    Write-Host "Profiller:"
    foreach ($h in GeminiHesaplari) {
        $acik = if (AcikMi $h) { "açık " } else { "kapalı" }
        Write-Host ("  {0,-30} {1}" -f $h, $acik)
    }
}

if ($Durum -or (-not $Ac -and -not $Kapat -and -not $Hepsi -and -not $Onar)) { DurumYaz }
if ($Hepsi) { GeminiHesaplari | ForEach-Object { HesapAc $_; Start-Sleep -Seconds 20 } }
if ($Ac) { HesapAc $Ac }
if ($Kapat) { HesapKapat $Kapat }
if ($Onar) { Onar $Onar }
