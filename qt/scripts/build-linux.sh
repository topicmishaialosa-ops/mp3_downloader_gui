#!/usr/bin/env bash
# Сборка Qt desktop для Linux
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="$ROOT/build"
OUT="$ROOT/dist/linux"
BIN="mp3_downloader_gui_qt"

echo "==> Qt Linux: cmake + build"
cmake -S "$ROOT" -B "$BUILD" -DCMAKE_BUILD_TYPE=Release
cmake --build "$BUILD" -j"$(nproc 2>/dev/null || echo 4)"

mkdir -p "$OUT"
cp -f "$BUILD/$BIN" "$OUT/$BIN"
chmod +x "$OUT/$BIN"

PORTABLE="$OUT/portable"
"$ROOT/scripts/package-portable.sh" "$OUT/$BIN" "$PORTABLE"

echo "Готово: $OUT/$BIN"
echo "Portable: $PORTABLE"
ls -lh "$OUT/$BIN"
