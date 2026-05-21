#!/usr/bin/env bash
# Упаковка Qt-бинарника с зависимостями (portable) для Linux / macOS.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAME="mp3_downloader_gui_qt"
OS="$(uname -s)"

usage() {
    echo "Usage: $0 <path-to-binary-or-app> <output-dir>" >&2
    exit 1
}

[[ $# -ge 2 ]] || usage
SRC="$1"
OUT="$2"

if [[ ! -e "$SRC" ]]; then
    echo "Ошибка: не найден: $SRC" >&2
    exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT"

case "$OS" in
    Darwin)
        APP_NAME="${BIN_NAME}.app"
        if [[ "$SRC" == *.app ]]; then
            cp -R "$SRC" "$OUT/$APP_NAME"
        else
            mkdir -p "$OUT/$APP_NAME/Contents/MacOS"
            cp "$SRC" "$OUT/$APP_NAME/Contents/MacOS/$BIN_NAME"
            chmod +x "$OUT/$APP_NAME/Contents/MacOS/$BIN_NAME"
            cat >"$OUT/$APP_NAME/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>mp3_downloader_gui_qt</string>
  <key>CFBundleIdentifier</key><string>net.mp3party.downloader.qt</string>
  <key>CFBundleName</key><string>MP3 Downloader Qt</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
        fi
        if command -v macdeployqt >/dev/null 2>&1; then
            macdeployqt "$OUT/$APP_NAME" -always-overwrite
        else
            echo "Предупреждение: macdeployqt не найден, .app без Qt frameworks." >&2
        fi
        ;;
    Linux)
        mkdir -p "$OUT/lib" "$OUT/plugins"
        cp -f "$SRC" "$OUT/$BIN_NAME"
        chmod +x "$OUT/$BIN_NAME"

        qt_query() {
            if command -v qmake6 >/dev/null 2>&1; then
                qmake6 -query "$1"
            elif command -v qmake >/dev/null 2>&1; then
                qmake -query "$1"
            else
                return 1
            fi
        }

        if plugins_root="$(qt_query QT_INSTALL_PLUGINS 2>/dev/null)"; then
            for plugin_dir in platforms xcbglintegrations tls multimedia; do
                if [[ -d "$plugins_root/$plugin_dir" ]]; then
                    cp -a "$plugins_root/$plugin_dir" "$OUT/plugins/"
                fi
            done
        fi

        if command -v ldd >/dev/null 2>&1; then
            while IFS= read -r lib; do
                case "$lib" in
                    /lib/*|/lib64/*|*/libc.so*|*/libm.so*|*/libpthread*|*/libdl.so*|*/libgcc_s*|*/libstdc++*|*/ld-linux*) continue ;;
                esac
                cp -L "$lib" "$OUT/lib/" 2>/dev/null || true
            done < <(ldd "$OUT/$BIN_NAME" | awk '/=> \// { print $3 }' | sort -u)
        fi

        if command -v patchelf >/dev/null 2>&1; then
            patchelf --set-rpath '$ORIGIN/lib' "$OUT/$BIN_NAME"
        fi

        cat >"$OUT/README.txt" <<'TXT'
MP3 Downloader GUI (Qt) — portable Linux

Запуск:
  ./mp3_downloader_gui_qt

Если не стартует, установите системный Qt6:
  sudo apt install qt6-base-dev qt6-multimedia-dev   # Debian/Ubuntu
  sudo pacman -S qt6-base qt6-multimedia              # Arch

YouTube: yt-dlp в PATH или автоустановка в ~/yt-dlp-util/bin/ при первом использовании.
Стрим/видео: mpv в PATH или ~/mpv-util/ (Windows/macOS — скачивание из приложения).
TXT
        ;;
    *)
        echo "Ошибка: package-portable.sh для $OS не поддерживается (используйте windeployqt на Windows)." >&2
        exit 1
        ;;
esac

echo "Portable: $OUT"
du -sh "$OUT" 2>/dev/null || ls -la "$OUT"
