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

mkdir -p "$OUT"
cp -f "$BUILD/$BIN" "$OUT/$BIN"
chmod +x "$OUT/$BIN"

echo "Готово: $OUT/$BIN"
ls -lh "$OUT/$BIN"
