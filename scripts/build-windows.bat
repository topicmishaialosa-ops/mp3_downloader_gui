@echo off
setlocal EnableExtensions

REM Сборка desktop .exe для Windows (запускать на Windows)
cd /d "%~dp0.."
set "OUT_DIR=%CD%\dist\windows"
set "BIN_NAME=mp3_downloader_gui.exe"
set "SRC=%CD%\target\release\mp3_downloader_gui.exe"

if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"

echo ==^> Windows: cargo build --release
cargo build --release
if errorlevel 1 exit /b 1

if not exist "%SRC%" (
    echo Ошибка: не найден %SRC%
    echo Установите Rust: https://rustup.rs/
    exit /b 1
)

copy /Y "%SRC%" "%OUT_DIR%\%BIN_NAME%"
echo Готово: %OUT_DIR%\%BIN_NAME%
dir "%OUT_DIR%\%BIN_NAME%"
endlocal
