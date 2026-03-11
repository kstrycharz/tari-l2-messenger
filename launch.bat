@echo off
title Tari Messenger
echo ================================
echo  Tari Messenger
echo ================================
echo.

cd /d "%~dp0"

:: Build the WASM template if not already built
if not exist "messaging_template\target\wasm32-unknown-unknown\release\messaging_template.wasm" (
    echo [1/2] Building messaging template...
    cd messaging_template
    cargo build --target wasm32-unknown-unknown --release
    if errorlevel 1 (
        echo ERROR: Template build failed.
        pause
        exit /b 1
    )
    cd ..
    echo Template built successfully.
    echo.
) else (
    echo [1/2] Template already built, skipping.
    echo.
)

:: Launch the web app
echo [2/2] Starting web server...
echo.
echo  Open http://localhost:3000 in your browser
echo  Press Ctrl+C to stop
echo.

cd messaging_app
cargo run

pause
