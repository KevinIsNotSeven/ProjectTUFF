@echo off
rem Converts the four soccer ball PNGs into game textures, then run "Install to SD.bat".
rem   NormalSoccer.png  -> soccerball_col.nutexb          (normal ball)
rem   SpikeSoccer.png   -> soccerball_spike_col.nutexb    (spike ball)
rem   LaunchSoccer.png  -> soccerball_launch_col.nutexb   (launch ball)
rem   GravitySoccer.png -> soccerball_gravity_col.nutexb  (gravity ball)
rem The ball is a plain sphere and each PNG wraps all the way around it like a world map:
rem 2:1 (512x256 or bigger), the middle of the image faces the front, the left and right edges
rem meet at the back, the top edge is the top of the ball. The grid lines are every 45 degrees.
setlocal
cd /d "%~dp0"
set TEX="%~dp0..\..\..\ClaudeTools\ultimate_tex\ultimate_tex_cli.exe"
set DIR=item\soccerball\model\body\c00
set STAGE=%~dp0..\sd_install\ultimate\mods\hdr-item-variants\%DIR%
set SRC=%~dp0..\hdr-melee-fox-dev\source\item_variants\mod\%DIR%
if not exist "%STAGE%" mkdir "%STAGE%"
call :conv NormalSoccer soccerball_col || goto :fail
call :conv SpikeSoccer soccerball_spike_col || goto :fail
call :conv LaunchSoccer soccerball_launch_col || goto :fail
call :conv GravitySoccer soccerball_gravity_col || goto :fail
copy /y "%SRC%\model.*" "%STAGE%\" >nul
copy /y "%SRC%\soccerball_prm.nutexb" "%STAGE%\" >nul
echo.
echo Done. Now put the SD card in and run "Install to SD.bat".
pause
exit /b 0

:conv
%TEX% -f BC7RgbaUnorm "%~1.png" "%SRC%\%~2.nutexb" || exit /b 1
copy /y "%SRC%\%~2.nutexb" "%STAGE%\%~2.nutexb" >nul
echo converted %~1.png
exit /b 0

:fail
echo Conversion FAILED
pause
exit /b 1
