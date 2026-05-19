#!/usr/bin/env bash
# Сборка release APK для Android
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_DIR="$ROOT/android"
OUT_DIR="$ROOT/dist/android"
APK_SRC="$ANDROID_DIR/app/build/outputs/apk/release/app-release.apk"
APK_DST="$OUT_DIR/mp3-downloader-release.apk"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$ROOT/.gradle-home}"

if [[ ! -d "$ANDROID_HOME" ]]; then
    echo "Ошибка: ANDROID_HOME не найден: $ANDROID_HOME" >&2
    echo "Установите Android SDK или задайте: export ANDROID_HOME=/path/to/Sdk" >&2
    exit 1
fi

mkdir -p "$OUT_DIR" "$GRADLE_USER_HOME"

echo "==> Android: ./gradlew assembleRelease"
cd "$ANDROID_DIR"
chmod +x ./gradlew
./gradlew assembleRelease --no-daemon

if [[ ! -f "$APK_SRC" ]]; then
    echo "Ошибка: APK не найден: $APK_SRC" >&2
    exit 1
fi

cp -f "$APK_SRC" "$APK_DST"
echo "Готово: $APK_DST"
ls -lh "$APK_DST"
