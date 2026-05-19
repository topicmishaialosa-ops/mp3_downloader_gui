@echo off
setlocal EnableExtensions

REM Сборка Qt desktop для Windows (запускать на Windows)
cd /d "%~dp0.."
set "BUILD=%CD%\build"
set "OUT=%CD%\dist\windows"
set "BIN=mp3_downloader_gui_qt.exe"

if not exist "%OUT%" mkdir "%OUT%"

echo ==^> Qt Windows: cmake + build
cmake -S "%CD%" -B "%BUILD%" -DCMAKE_BUILD_TYPE=Release
if errorlevel 1 exit /b 1

cmake --build "%BUILD%" --config Release
if errorlevel 1 exit /b 1

set "SRC=%BUILD%\Release\%BIN%"
if not exist "%SRC%" set "SRC=%BUILD%\%BIN%"

if not exist "%SRC%" (
    echo Ошибка: не найден %BIN% в %BUILD%
    exit /b 1
)

copy /Y "%SRC%" "%OUT%\%BIN%"
echo Готово: %OUT%\%BIN%
dir "%OUT%\%BIN%"
endlocal
