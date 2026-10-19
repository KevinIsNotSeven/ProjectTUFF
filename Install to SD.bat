@echo off
rem Installs everything in the sd_install folder onto the Switch SD card.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0sd_install\install.ps1"
echo.
pause
