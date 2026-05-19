#!/usr/bin/env bash
# Сборка desktop-приложения для Linux (x86_64)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$ROOT/dist/linux"
BIN_NAME="mp3_downloader_gui"

echo "==> Linux: cargo build --release"
cd "$ROOT"
cargo build --release

mkdir -p "$OUT_DIR"
cp -f "$ROOT/target/release/$BIN_NAME" "$OUT_DIR/$BIN_NAME"
chmod +x "$OUT_DIR/$BIN_NAME"

echo "Готово: $OUT_DIR/$BIN_NAME"
ls -lh "$OUT_DIR/$BIN_NAME"
