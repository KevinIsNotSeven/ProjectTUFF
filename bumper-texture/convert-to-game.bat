@echo off
rem Converts bumper_col.png (your edited image) into bumper_col.nutexb for the game.
rem Format BC1 sRGB matches the original texture. Keep the PNG at 256x512.
cd /d "%~dp0"
"C:\Users\kevin\OneDrive\Desktop\GamesD\ClaudeTools\ultimate_tex\ultimate_tex_cli.exe" -f BC1RgbaUnormSrgb bumper_col.png bumper_col.nutexb
if errorlevel 1 (echo Conversion FAILED & pause & exit /b 1)
echo Done: bumper_col.nutexb
pause
