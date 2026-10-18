@echo off
title Kervan Sade Pano
cd /d "c:\Users\buzbe\Desktop\Github\kervan-pano"
echo ========================================================
echo   Kervan Sade Pano - Windows Dogrudan GUI Baslatici
echo ========================================================
".\target\debug\kervan.exe" --serve --gui
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Hata ile kapandi! Kod: %ERRORLEVEL%
    pause
)
