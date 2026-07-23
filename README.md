# MP3 Downloader GUI

Кроссплатформенный поиск и скачивание музыки с **MP3Party**, **DriveMusic** и **YouTube** (через yt-dlp).

[![Build](https://github.com/topicmishaialosa-ops/mp3_downloader_gui/actions/workflows/build.yml/badge.svg)](https://github.com/topicmishaialosa-ops/mp3_downloader_gui/actions/workflows/build.yml)

## Скриншоты

![Главное()

## Готовые сборки (Releases)

Скачайте бинарники на странице **[Releases](https://github.com/topicmishaialosa-ops/mp3_downloader_gui/releases)** (без сборки):

| Файл | Описание |
|------|----------|
| `mp3-downloader-<версия>.apk` | Android (Kotlin; сборка из `android/` и `qt/scripts/build-android`) |
| `mp3-downloader-android-<версия>.apk` | то же APK (алиас) |
| `mp3_downloader_gui-linux-x86_64.zip` | Desktop **Rust** (Linux, один бинарник) |
| `mp3_downloader_gui-windows-x86_64.zip` | Desktop **Rust** (Windows `.exe`) |
| `mp3_downloader_gui-macos-arm64.zip` | Desktop **Rust** (macOS Apple Silicon) |
| `mp3_downloader_gui_qt-linux-x86_64-portable.zip` | Desktop **Qt** (Linux: бинарник + `lib/` + `plugins/`) |
| `mp3_downloader_gui_qt-windows-x86_64-portable.zip` | Desktop **Qt** (Windows: `.exe` + Qt DLL через windeployqt) |
| `mp3_downloader_gui_qt-macos-arm64-portable.zip` | Desktop **Qt** (macOS: `.app` с frameworks) |

Сборки создаёт **GitHub Actions** (`.github/workflows/build.yml`):
- при push в `master` — артефакты в Actions;
- при теге `v*` (например `v1.4.7`) — автоматический **Release**;
- вручную: Actions → Build → Run workflow → «Создать GitHub Release».

Для **YouTube** нужен [yt-dlp](https://github.com/yt-dlp/yt-dlp). Во **всех** версиях приложения (Rust, Qt, Android), если зависимости нет, показывается предложение скачать/установить: desktop — бинарник в `~/yt-dlp-util/bin/` (Windows: `%USERPROFILE%\yt-dlp-util\bin\`), Android — встроенная библиотека при первом запуске YouTube.

Для **стриминга, видео и перемотки** в desktop (Rust, Qt) рекомендуется [mpv](https://mpv.io/). Если mpv нет в PATH, приложение предложит установку: Windows/macOS — portable в `~/mpv-util/` (нужен 7-Zip для распаковки), Linux — подсказка `pacman`/`apt` и ссылка на mpv.io. Без mpv воспроизведение возможно в ограниченном режиме (Rust — загрузка потока в файл, Qt — встроенный плеер).

## Компоненты

| Платформа | Стек | Папка |
|-----------|------|-------|
| Desktop (Rust) | egui/eframe, rodio | `src/` |
| Desktop (Qt) | Qt6, C++ | [`qt/`](qt/) |
| Android | Kotlin, ExoPlayer | `android/` |

## Возможности

- Поиск и скачивание MP3/MP4
- **Пакетный поиск**: вставьте много треков сразу (по одному на строку) — приложение найдёт их по очереди в выбранном источнике
- Библиотека «Мои файлы», встроенный плеер, стриминг YouTube
- DriveMusic: URL страницы только из поиска (без хардкода жанра)

### Пакетный поиск

Рядом с обычным поиском есть кнопка **📋 Список** (Rust/Qt) или **📋 Пакетный поиск** (Android).
Открывает многострочное поле, в которое можно вставить сразу несколько треков:

```
Кино - Группа крови
Агата Кристи - Опиум для никого
1. Сектор Газа - Лирика
2. Король и Шут - Лесник
Просто название   # будет искать только по названию
https://www.youtube.com/watch?v=…   # URL
# комментарий игнорируется
```

Поддерживаемые форматы в строке:
- `Исполнитель - Название` (дефис, en-dash `–`, em-dash `—`)
- Только название (если нет разделителя)
- URL `https://…` (прямая ссылка)

Дополнительно: нумерация (`1.`, `12)`) в начале строки автоматически снимается,
комментарии после `#` игнорируются. Каждый запрос ищется в **текущем выбранном источнике**
(MP3Party / DriveMusic / YouTube), результаты аккумулируются в общий список
(дубликаты по `id` отбрасываются), после чего работает обычная кнопка «📥 Скачать все» /
«Скачать все».

### Папка загрузок по умолчанию (Rust)

`~/mp3_downloader_gui/downloads` (Linux/macOS) или `%USERPROFILE%\mp3_downloader_gui\downloads` (Windows). На Windows сборка без отдельного окна консоли (`windows_subsystem`).

## Компиляторы и зависимости

### Общее

| Инструмент | Версия | Зачем |
|------------|--------|--------|
| **Git** | любая | клонирование репозитория |

### Rust (egui) — Linux

| Пакет (Arch) | Зачем |
|--------------|--------|
| `rust` / **rustup** | компилятор Rust |
| `gcc` | линковка Linux |
| `alsa-lib` | звук (rodio), runtime |
| `pkgconf` | сборка зависимостей |

```bash
sudo pacman -S rustup base-devel alsa-lib pkgconf
# или только rust из репозитория для локальной сборки Linux
```

### Rust → Windows `.exe` на Linux (без Docker)

| Инструмент | Зачем |
|------------|--------|
| **rustup** | таргет `x86_64-pc-windows-msvc` |
| **cargo-xwin** | MSVC SDK и линковка без Visual Studio |
| `clang` | зависимость xwin (обычно уже есть) |

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo install cargo-xwin --locked
./scripts/build-windows-cross.sh
```

Результат: `dist/windows/mp3_downloader_gui.exe`

### Rust — Windows / macOS (нативно)

| ОС | Инструменты |
|----|-------------|
| **Windows** | [Rust](https://rustup.rs/), Visual Studio Build Tools *или* `cargo install cargo-xwin` → `scripts/build-windows.bat` |
| **macOS** | Xcode Command Line Tools, rustup → `scripts/build-macos.sh` |

### Qt (C++) — Linux

| Пакет (Arch) | Зачем |
|--------------|--------|
| `qt6-base` `qt6-multimedia` | Qt6 Widgets + плеер |
| `cmake` `ninja` | сборка |
| `gcc` | компилятор C++ |

```bash
sudo pacman -S qt6-base qt6-multimedia cmake ninja gcc
./qt/scripts/build-linux.sh
```

Qt на **Windows** / **macOS** — собирайте на соответствующей ОС (`qt/scripts/build-windows.bat`, `build-macos.sh`).

### Android APK

| Инструмент | Зачем |
|------------|--------|
| **JDK 17+** | Gradle |
| **Android SDK** | `ANDROID_HOME` |
| ~8–12 ГБ диска | Gradle + SDK |

```bash
export ANDROID_HOME=~/Android/Sdk
./scripts/build-android.sh
```

## Скрипты сборки

### Rust — `scripts/`

| Платформа | Скрипт | Результат |
|-----------|--------|-----------|
| Linux | `build-linux.sh` | `dist/linux/mp3_downloader_gui` |
| Windows (на Linux, без Docker) | **`build-windows-cross.sh`** | `dist/windows/mp3_downloader_gui.exe` |
| Windows (на Windows) | `build-windows.bat` | `dist/windows/mp3_downloader_gui.exe` |
| macOS | `build-macos.sh` | `dist/macos/mp3_downloader_gui` |
| Android | `build-android.sh` / `.bat` | `dist/android/mp3-downloader-release.apk` |

### Qt — `qt/scripts/`

| Платформа | Скрипт | Результат |
|-----------|--------|-----------|
| Linux | `build-linux.sh` | `qt/dist/linux/portable/` (бинарник + Qt libs/plugins) |
| Windows | `build-windows.bat` | `qt/dist/windows/portable/` (exe + DLL) |
| macOS | `build-macos.sh` | `qt/dist/macos/portable/*.app` |
| Android | `build-android.sh` | `dist/android/mp3-downloader-release.apk` (Kotlin) |

Подробнее: [qt/README.md](qt/README.md).

## Лицензия

Соблюдайте авторские права и условия источников (MP3Party, DriveMusic, YouTube).
