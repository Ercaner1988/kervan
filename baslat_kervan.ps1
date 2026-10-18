# Kervan GUI Başlatıcı (Windows)
$exePath = "C:\Users\buzbe\Desktop\Github\kervan-pano\target\debug\kervan.exe"
$workingDir = "C:\Users\buzbe\Desktop\Github\kervan-pano"

# Varsa eski süreci sonlandır
Get-Process -Name kervan -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Yeni süreci masaüstü etkileşimli olarak başlat
$proc = Start-Process -FilePath $exePath `
    -ArgumentList "--serve", "--gui" `
    -WorkingDirectory $workingDir `
    -WindowStyle Normal `
    -PassThru

Start-Sleep -Seconds 2

$current = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
if ($current) {
    Write-Host "Kervan GUI başarıyla başlatıldı!"
    Write-Host "PID: $($current.Id)"
    Write-Host "Başlık: $($current.MainWindowTitle)"
    Write-Host "Pencere Tanıtıcısı (HWND): $($current.MainWindowHandle)"
} else {
    Write-Host "HATA: Süreç başlatılamadı veya hemen kapandı."
}
