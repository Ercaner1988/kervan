@echo off
setlocal
title Kervan Ag Gecidi (arayuzsuz)
cd /d "%~dp0"
echo ========================================================
echo   Kervan - Arayuzsuz Sunucu (arayuz icin: OpenChamber)
echo ========================================================

set "KERVAN_EXE=%~dp0target\release\kervan.exe"

rem --- Bayat ikili korumasi -------------------------------------------
rem 28.10.2026'da yasandi: kervan.toml'a yeni bir alan eklendi, ama
rem target\release\kervan.exe eski derlemeden kaldi ve sunucu
rem "unknown field" hatasiyla dustu. Sebep kodda degil, ikilinin yasta
rem olmasiydi. Cozum: cargo varsa her acilista once derle. Cargo guncel
rem derlemede saniyenin altinda doner; kaynak yeniyse gercekten derler.
rem NOT: Bu dosya SAF ASCII olmali. Kutu cizim karakterleri cmd'nin
rem      kod sayfasinda satiri boluyor ve degiskenler bos kaliyor.
set "DERLEDI=0"
where cargo >nul 2>nul
if errorlevel 1 goto :derlemesiz

echo [derleme] cargo build --release ...
cargo build --release --quiet
if errorlevel 1 goto :derlemeHatasi
set "DERLEDI=1"
goto :calistir

:derlemesiz
echo [uyari] cargo PATH'te yok; mevcut ikili ile devam ediliyor.
echo [uyari] Yapilandirma hatasi alirsaniz once: cargo build --release
goto :calistir

:derlemeHatasi
echo.
echo [hata] Derleme basarisiz. Eski ikili CALISTIRILMAYACAK:
echo        bayat ikili, cozulmus bir hatayi yeniden gosterebilir.
echo        Derleme ciktisini yukarida gorun.
pause
endlocal
exit /b 1

:calistir
if not exist "%KERVAN_EXE%" goto :ikiliYok
"%KERVAN_EXE%" --serve
set "KOD=%ERRORLEVEL%"
if not "%KOD%"=="0" goto :sunucuHatasi
endlocal
exit /b 0

:ikiliYok
echo.
echo [hata] %KERVAN_EXE% yok ve derleme uretmedi.
pause
endlocal
exit /b 1

:sunucuHatasi
echo.
echo Hata ile kapandi! Kod: %KOD%
if "%DERLEDI%"=="1" echo Ikili bu acilista tazelendi; hata kaynakta.
pause
endlocal
exit /b %KOD%
