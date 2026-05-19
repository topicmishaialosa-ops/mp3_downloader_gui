# MP3 Downloader GUI

Кроссплатформенный поиск и скачивание музыки с **MP3Party**, **DriveMusic** и **YouTube** (через yt-dlp).

## Компоненты

| Платформа | Стек | Папка |
|-----------|------|-------|
| Desktop (Rust) | egui/eframe | `src/` |
| Desktop (Qt) | Qt6, C++ | [`qt/`](qt/) |
| Android | Kotlin, ExoPlayer, youtubedl-android | `android/` |

## Возможности

- Поиск и скачивание MP3/MP4
- Библиотека локальных файлов, открытие папки музыки
- Встроенный плеер с перемоткой
- Стриминг YouTube без полного скачивания
- Тёмная/светлая тема (desktop)

## Скрипты сборки

### Rust (egui) — `scripts/`

| Платформа | Скрипт | Результат |
|-----------|--------|-----------|
| Linux | [`scripts/build-linux.sh`](scripts/build-linux.sh) | `dist/linux/mp3_downloader_gui` |
| macOS | [`scripts/build-macos.sh`](scripts/build-macos.sh) | `dist/macos/mp3_downloader_gui` |
| Windows | [`scripts/build-windows.bat`](scripts/build-windows.bat) | `dist/windows/mp3_downloader_gui.exe` |
| Android (Linux/Mac) | [`scripts/build-android.sh`](scripts/build-android.sh) | `dist/android/mp3-downloader-release.apk` |
| Android (Windows) | [`scripts/build-android.bat`](scripts/build-android.bat) | `dist/android/mp3-downloader-release.apk` |

### Qt (C++) — [`qt/scripts/`](qt/scripts/)

| Платформа | Скрипт | Результат |
|-----------|--------|-----------|
| Linux | [`qt/scripts/build-linux.sh`](qt/scripts/build-linux.sh) | `qt/dist/linux/mp3_downloader_gui_qt` |
| macOS | [`qt/scripts/build-macos.sh`](qt/scripts/build-macos.sh) | `qt/dist/macos/mp3_downloader_gui_qt` |
| Windows | [`qt/scripts/build-windows.bat`](qt/scripts/build-windows.bat) | `qt/dist/windows/mp3_downloader_gui_qt.exe` |
| Android | [`qt/scripts/build-android.sh`](qt/scripts/build-android.sh) | тот же Kotlin APK (см. корень) |

Подробнее: [qt/README.md](qt/README.md).

### Linux / macOS (shell)

```bash
chmod +x scripts/*.sh
./scripts/build-linux.sh    # только Linux
./scripts/build-macos.sh    # только на Mac
./scripts/build-android.sh  # нужен Android SDK (ANDROID_HOME)
```

### Windows (cmd)

```bat
scripts\build-windows.bat
scripts\build-android.bat
```

Требования: [Rust](https://rustup.rs/), для Android — [Android SDK](https://developer.android.com/studio) и `ANDROID_HOME`.

## Ручная сборка

```bash
cargo build --release
cd android && ./gradlew assembleRelease
```

## Лицензия

Соблюдайте авторские права и условия источников (MP3Party, DriveMusic, YouTube).
