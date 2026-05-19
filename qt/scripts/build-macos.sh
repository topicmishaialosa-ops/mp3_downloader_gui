#!/usr/bin/env bash
# Сборка Qt desktop для macOS (только на Mac)
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "Ошибка: запустите на macOS." >&2
    exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="$ROOT/build"
OUT="$ROOT/dist/macos"
BIN="mp3_downloader_gui_qt"

echo "==> Qt macOS: cmake + build"
cmake -S "$ROOT" -B "$BUILD" -DCMAKE_BUILD_TYPE=Release
cmake --build "$BUILD" -j"$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"

APP_SRC="$BUILD/${BIN}.app"
if [[ ! -d "$APP_SRC" ]]; then
    echo "Ошибка: не найден $APP_SRC (нужен MACOSX_BUNDLE в CMake)." >&2
    exit 1
fi

PORTABLE="$OUT/portable"
"$ROOT/scripts/package-portable.sh" "$APP_SRC" "$PORTABLE"

echo "Готово: $PORTABLE/${BIN}.app"
ls -lh "$PORTABLE"
