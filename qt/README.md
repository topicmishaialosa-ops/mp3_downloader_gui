# MP3 Downloader GUI (Qt)

Desktop-версия на **Qt6** (C++): MP3Party, DriveMusic, YouTube (yt-dlp).

## Возможности

- Поиск и скачивание (MP3/MP4 для YouTube)
- **▶ Слушать** — онлайн-стрим (все источники)
- Вкладка **Мои файлы** — локальная библиотека и воспроизведение
- Плеер с перемоткой (QMediaPlayer)
- DriveMusic: URL страницы только из результатов поиска (без хардкода жанра)

Мобильное приложение — в папке [`../android/`](../android/) (Kotlin).

## Скрипты сборки

| Платформа | Скрипт | Результат |
|-----------|--------|-----------|
| Linux | [`scripts/build-linux.sh`](scripts/build-linux.sh) | `dist/linux/portable/` |
| macOS | [`scripts/build-macos.sh`](scripts/build-macos.sh) | `dist/macos/portable/*.app` |
| Windows | [`scripts/build-windows.bat`](scripts/build-windows.bat) | `dist/windows/portable/` |
| Android (Linux/Mac) | [`scripts/build-android.sh`](scripts/build-android.sh) | `../dist/android/mp3-downloader-release.apk` |
| Android (Windows) | [`scripts/build-android.bat`](scripts/build-android.bat) | то же |

## Требования

- Qt 6 (Core, Widgets, Network)
- CMake ≥ 3.16
- C++17 компилятор
- **yt-dlp** в PATH или автоустановка в `~/yt-dlp-util/bin/` при первом использовании YouTube
- **mpv** в PATH или автоустановка в `~/mpv-util/` (стриминг, видео, перемотка; без mpv — Qt-плеер)

На **Windows** папка загрузок по умолчанию — `%USERPROFILE%\mp3_downloader_gui\downloads` (через `QStandardPaths` + нативные разделители). Для mpv IPC используется именованный канал, не Unix-socket.

### Arch Linux

```bash
sudo pacman -S qt6-base cmake gcc
```

## Сборка

```bash
./scripts/build-linux.sh
```

Rust/egui-версия — в корне репозитория (`cargo build --release`).
