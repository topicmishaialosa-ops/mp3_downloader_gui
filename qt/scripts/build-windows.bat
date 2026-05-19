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

set "PORTABLE=%OUT%\portable"
if not exist "%PORTABLE%" mkdir "%PORTABLE%"
copy /Y "%SRC%" "%PORTABLE%\%BIN%"

where windeployqt >nul 2>&1
if %ERRORLEVEL%==0 (
    echo ==^> windeployqt
    windeployqt --release --no-translations "%PORTABLE%\%BIN%"
) else (
    echo Предупреждение: windeployqt не в PATH — только .exe без Qt DLL
)

echo Готово: %PORTABLE%\%BIN%
dir "%PORTABLE%"
endlocal
