@echo off
title Tari Messenger - Dev (2 clients)
echo ================================
echo  Tari Messenger - Dev Mode
echo  Ootle:    http://localhost:3000
echo  Minotari: http://localhost:3001
echo ================================
echo.

cd /d "%~dp0"

:: Kill any existing instances
taskkill /f /im messaging-app.exe >nul 2>&1

:: Build
echo Building messaging app...
cd messaging_app
cargo build
if errorlevel 1 (
    echo ERROR: Build failed.
    pause
    exit /b 1
)
cd ..
echo Build successful.
echo.

:: Launch Ootle in a new window
echo Starting Ootle on port 3000...
start "Ootle - port 3000" cmd /k "cd /d %~dp0messaging_app && cargo run -- --port 3000 --state messaging-state-ootle.json"

:: Brief pause so Ootle starts binding first
timeout /t 2 /nobreak >nul

:: Launch Minotari in a new window
echo Starting Minotari on port 3001...
start "Minotari - port 3001" cmd /k "cd /d %~dp0messaging_app && cargo run -- --port 3001 --state messaging-state-minotari.json"

echo.
echo Both clients launching...
echo   Ootle:    http://localhost:3000
echo   Minotari: http://localhost:3001
echo.
echo Close the individual windows to stop each client.
pause
