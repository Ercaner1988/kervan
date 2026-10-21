# Kervan Sunucu Başlatıcı (Windows, arayüzsüz)
# Arayüz kararı (2026-10-19): masaüstü GUI'si devre dışı; kontrol yüzeyi
# OpenChamber uzantısı olacak. Sunucu yalnız ağ geçidi olarak koşar.
# Betik, depo kökünü kendi konumundan türetir (taşınabilir).
$repoRoot = $PSScriptRoot
$release = Join-Path $repoRoot "target\release\kervan.exe"
$debug = Join-Path $repoRoot "target\debug\kervan.exe"
$exePath = if (Test-Path $release) { $release }
           elseif (Test-Path $debug) { $debug }
           else {
               Write-Host "HATA: kervan.exe bulunamadı. Önce 'cargo build --release' çalıştırın."
               exit 1
           }
$workingDir = $repoRoot

# Varsa eski süreci sonlandır
Get-Process -Name kervan -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Yeni süreci masaüstü etkileşimli olarak başlat
$proc = Start-Process -FilePath $exePath `
    -ArgumentList "--serve" `
    -WorkingDirectory $workingDir `
    -WindowStyle Normal `
    -PassThru

Start-Sleep -Seconds 2

$current = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
if ($current) {
    Write-Host "Kervan sunucu başarıyla başlatıldı (arayüzsuz)."
    Write-Host "PID: $($current.Id)"
    Write-Host "Sağlık: http://127.0.0.1:20129/health"
} else {
    Write-Host "HATA: Süreç başlatılamadı veya hemen kapandı."
}
