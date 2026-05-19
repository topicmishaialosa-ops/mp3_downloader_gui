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

# Release signing uses ~/.android/debug.keystore unless ANDROID_KEYSTORE_PATH is set.
if [[ -z "${ANDROID_KEYSTORE_PATH:-}" ]]; then
  debug_ks="${HOME}/.android/debug.keystore"
  if [[ ! -f "$debug_ks" ]]; then
    echo "==> Creating debug keystore for release APK: $debug_ks"
    mkdir -p "${HOME}/.android"
    keytool -genkeypair -v \
      -keystore "$debug_ks" \
      -storepass android \
      -alias androiddebugkey \
      -keypass android \
      -keyalg RSA \
      -keysize 2048 \
      -validity 10000 \
      -dname "CN=MP3 Downloader, OU=CI, O=MP3Party, L=Unknown, ST=Unknown, C=US"
  fi
fi

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
