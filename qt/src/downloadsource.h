#pragma once

enum class DownloadSource {
    Mp3Party,
    DriveMusic,
    YtDlp,
    PesniMe,
};

enum class YtFormat {
    Mp3,
    Mp4,
};

inline QString downloadSourceLabel(DownloadSource s) {
    switch (s) {
    case DownloadSource::Mp3Party: return QStringLiteral("MP3Party");
    case DownloadSource::DriveMusic: return QStringLiteral("DriveMusic");
    case DownloadSource::YtDlp: return QStringLiteral("YouTube (yt-dlp)");
    case DownloadSource::PesniMe: return QStringLiteral("Pesni.me");
    }
    return QStringLiteral("?");
}
