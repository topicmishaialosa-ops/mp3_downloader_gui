#pragma once

#include <QString>

namespace AppConstants {
inline const QString kUserAgent =
    QStringLiteral("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                   "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36");
inline const QString kDriveMusicBase = QStringLiteral("https://ru.drivemusic.me");
inline const QString kPesniMeBase = QStringLiteral("https://play.pesni.me/");
inline const qint64 kMinDownloadBytes = 50 * 1024;
inline const QString kDefaultDownloadDir = QStringLiteral("downloads");
} // namespace AppConstants
