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
    # На multi-config генераторах (.e.g. Ninja Multi-Config/Xcode) bundle лежит в Release/
    for candidate in \
        "$BUILD/Release/${BIN}.app" \
        "$BUILD/RelWithDebInfo/${BIN}.app" \
        "$BUILD/MinSizeRel/${BIN}.app"; do
        if [[ -d "$candidate" ]]; then
            APP_SRC="$candidate"
            break
        fi
    done
fi
if [[ ! -d "$APP_SRC" ]]; then
    echo "Ошибка: не найден ${BIN}.app в $BUILD (ожидали bundle output от CMake)." >&2
    exit 1
fi

PORTABLE="$OUT/portable"
"$ROOT/scripts/package-portable.sh" "$APP_SRC" "$PORTABLE"

echo "Готово: $PORTABLE/${BIN}.app"
ls -lh "$PORTABLE"
