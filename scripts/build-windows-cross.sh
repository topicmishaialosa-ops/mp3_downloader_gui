#!/usr/bin/env bash
# Сборка .exe для Windows на Linux без Docker.
# Вариант A: llvm-mingw (GNU) — по умолчанию
# Вариант B: cargo-xwin (MSVC) — BUILD_WINDOWS_MSVC=1
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$ROOT/dist/windows"
BIN_NAME="mp3_downloader_gui.exe"

if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
export PATH="${HOME}/.cargo/bin:${PATH}"

if ! command -v rustup >/dev/null 2>&1; then
    echo "Установите rustup: https://rustup.rs/" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"
cd "$ROOT"

if [ "${BUILD_WINDOWS_MSVC:-0}" = "1" ]; then
    TARGET="x86_64-pc-windows-msvc"
    if ! command -v cargo-xwin >/dev/null 2>&1; then
        echo "==> Установка cargo-xwin…"
        cargo install cargo-xwin --locked
    fi
    rustup target add "$TARGET"
    echo "==> cargo xwin build --release --target $TARGET"
    cargo xwin build --release --target "$TARGET"
    SRC="$ROOT/target/$TARGET/release/$BIN_NAME"
    if [ ! -f "$SRC" ]; then
        SRC="$ROOT/target/$TARGET/release/mp3_downloader_gui"
    fi
    cp -f "$SRC" "$OUT_DIR/$BIN_NAME"
else
    TARGET="x86_64-pc-windows-gnu"
    # Актуальный тег llvm-mingw можно переопределить: LLVM_MINGW_TAG=20260602 ./scripts/build-windows-cross.sh
    LLVM_MINGW_TAG="${LLVM_MINGW_TAG:-20260602}"
    LLVM_DIR="${LLVM_DIR:-$HOME/.local/llvm-mingw-$LLVM_MINGW_TAG}"
    if [ ! -x "$LLVM_DIR/bin/x86_64-w64-mingw32-clang" ]; then
        echo "==> Скачивание llvm-mingw $LLVM_MINGW_TAG (один раз)…"
        curl -fL --retry 3 \
            "https://github.com/mstorsjo/llvm-mingw/releases/download/${LLVM_MINGW_TAG}/llvm-mingw-${LLVM_MINGW_TAG}-ucrt-ubuntu-22.04-x86_64.tar.xz" \
            -o /tmp/llvm-mingw.tar.xz
        mkdir -p "$HOME/.local"
        tar -xJf /tmp/llvm-mingw.tar.xz -C "$HOME/.local"
    fi
    export PATH="$LLVM_DIR/bin:$PATH"
    rustup target add "$TARGET"
    echo "==> cargo build --release --target $TARGET (llvm-mingw)"
    cargo build --release --target "$TARGET"
    cp -f "$ROOT/target/$TARGET/release/mp3_downloader_gui.exe" "$OUT_DIR/$BIN_NAME"
fi

echo "Готово: $OUT_DIR/$BIN_NAME"
ls -lh "$OUT_DIR/$BIN_NAME"
file "$OUT_DIR/$BIN_NAME"
