#!/usr/bin/env bash
# Сборка desktop-приложения для macOS (нативная архитектура хоста)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$ROOT/dist/macos"
BIN_NAME="mp3_downloader_gui"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "Ошибка: сборка macOS-бинарника возможна только на Mac (Darwin)." >&2
    echo "Запустите: ./scripts/build-macos.sh на компьютере с macOS." >&2
    exit 1
fi

echo "==> macOS: cargo build --release"
cd "$ROOT"
cargo build --release

mkdir -p "$OUT_DIR"
cp -f "$ROOT/target/release/$BIN_NAME" "$OUT_DIR/$BIN_NAME"
chmod +x "$OUT_DIR/$BIN_NAME"

echo "Готово: $OUT_DIR/$BIN_NAME"
ls -lh "$OUT_DIR/$BIN_NAME"
