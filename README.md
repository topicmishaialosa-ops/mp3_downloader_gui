# MP3 Downloader GUI

Кроссплатформенный поиск и скачивание музыки с **MP3Party**, **DriveMusic** и **YouTube** (через yt-dlp).

## Компоненты

| Платформа | Стек | Папка |
|-----------|------|-------|
| Desktop | Rust, egui/eframe | `src/` |
| Android | Kotlin, ExoPlayer, youtubedl-android | `android/` |

## Возможности

- Поиск и скачивание MP3/MP4
- Библиотека локальных файлов, открытие папки музыки
- Встроенный плеер с **перемоткой** (ползунок и время)
- Стриминг YouTube без полного скачивания
- Тёмная/светлая тема (desktop)

## Сборка

### Desktop (Linux)

```bash
cargo build --release
cargo run --release
```

### Android APK

```bash
cd android
./gradlew assembleRelease
```

APK: `android/app/build/outputs/apk/release/app-release.apk`


