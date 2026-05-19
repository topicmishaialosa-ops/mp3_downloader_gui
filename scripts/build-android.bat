@echo off
setlocal EnableExtensions EnableDelayedExpansion

REM Сборка release APK для Android (запускать на Windows)
cd /d "%~dp0.."
set "ANDROID_DIR=%CD%\android"
set "OUT_DIR=%CD%\dist\android"
set "APK_SRC=%ANDROID_DIR%\app\build\outputs\apk\release\app-release.apk"
set "APK_DST=%OUT_DIR%\mp3-downloader-release.apk"

if "%ANDROID_HOME%"=="" (
    if exist "%LOCALAPPDATA%\Android\Sdk" (
        set "ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk"
    ) else if exist "%USERPROFILE%\Android\Sdk" (
        set "ANDROID_HOME=%USERPROFILE%\Android\Sdk"
    )
)

if "%ANDROID_HOME%"=="" (
    echo Ошибка: задайте ANDROID_HOME ^(путь к Android SDK^)
    exit /b 1
)

if not exist "%OUT_DIR%" mkdir "%OUT_DIR%"
if "%GRADLE_USER_HOME%"=="" set "GRADLE_USER_HOME=%CD%\.gradle-home"
if not exist "%GRADLE_USER_HOME%" mkdir "%GRADLE_USER_HOME%"

echo ANDROID_HOME=%ANDROID_HOME%
echo GRADLE_USER_HOME=%GRADLE_USER_HOME%

cd /d "%ANDROID_DIR%"
echo ==^> Android: gradlew.bat assembleRelease
call gradlew.bat assembleRelease --no-daemon
if errorlevel 1 exit /b 1

if not exist "%APK_SRC%" (
    echo Ошибка: APK не найден: %APK_SRC%
    exit /b 1
)

copy /Y "%APK_SRC%" "%APK_DST%"
echo Готово: %APK_DST%
dir "%APK_DST%"
endlocal
