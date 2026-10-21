@echo off
title Kervan Ag Gecidi (arayuzsuz)
cd /d "%~dp0"
echo ========================================================
echo   Kervan - Arayuzsuz Sunucu (arayuz icin: OpenChamber)
echo ========================================================
set "KERVAN_EXE=%~dp0target\release\kervan.exe"
if not exist "%KERVAN_EXE%" set "KERVAN_EXE=%~dp0target\debug\kervan.exe"
"%KERVAN_EXE%" --serve
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Hata ile kapandi! Kod: %ERRORLEVEL%
    pause
)
