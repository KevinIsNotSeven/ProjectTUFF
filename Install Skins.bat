@echo off
rem Installs every skin in <SD card>\TuffCharacterSkins as its own ARCropolis mod.
rem Skins already installed on the SD card keep their slots. See
rem hdr-melee-fox-dev\source\skin_installer\install_skins.py for details.
setlocal
set SD=
for %%D in (D E F G H I J K L M N O P Q R S T U V W X Y Z) do (
    if exist "%%D:\TuffCharacterSkins" if exist "%%D:\ultimate\mods" set SD=%%D:\
)
if "%SD%"=="" (
    echo Couldn't find an SD card with a TuffCharacterSkins folder. Put it in the PC and try again.
    pause
    exit /b 1
)
set HASHES=
if exist "%~dp0..\..\ClaudeTools\Hashes_FullPath.txt" set HASHES=--hashes "%~dp0..\..\ClaudeTools\Hashes_FullPath.txt"
echo SD card: %SD%
echo.
echo ---- what will be installed ----
py -I "%~dp0hdr-melee-fox-dev\source\skin_installer\install_skins.py" "%SD%TuffCharacterSkins" "%SD%" %HASHES%
echo.
choice /m "Install these skins"
if errorlevel 2 exit /b 0
py -I "%~dp0hdr-melee-fox-dev\source\skin_installer\install_skins.py" "%SD%TuffCharacterSkins" "%SD%" %HASHES% --apply
echo.
pause
