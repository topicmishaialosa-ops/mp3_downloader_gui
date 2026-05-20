#pragma once

#include <QDir>
#include <QStandardPaths>
#include <QString>

namespace AppPaths {

/// Папка загрузок по умолчанию: ~/mp3_downloader_gui/downloads (корректные разделители на всех ОС).
inline QString defaultDownloadFolder() {
    const QString home = QStandardPaths::writableLocation(QStandardPaths::HomeLocation);
    return QDir(home).filePath(QStringLiteral("mp3_downloader_gui/downloads"));
}

/// Склеить каталог и имя файла нативными разделителями (важно для Windows).
inline QString fileInDownloadFolder(const QString &folder, const QString &fileName) {
    return QDir(folder).filePath(fileName);
}

} // namespace AppPaths
