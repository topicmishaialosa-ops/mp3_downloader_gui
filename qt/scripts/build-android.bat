@echo off
REM Android APK (Kotlin) — тот же модуль, что и в корне репозитория
cd /d "%~dp0..\.."
call scripts\build-android.bat
