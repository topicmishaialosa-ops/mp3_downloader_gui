#include "libraryscanner.h"

#include <QDir>
#include <QFileInfo>

static const QStringList kMediaExt = {
    QStringLiteral("mp3"), QStringLiteral("mp4"), QStringLiteral("m4a"),
    QStringLiteral("opus"), QStringLiteral("webm"), QStringLiteral("mkv"),
    QStringLiteral("wav"), QStringLiteral("flac"),
};
static const QStringList kVideoExt = {
    QStringLiteral("mp4"), QStringLiteral("webm"), QStringLiteral("mkv"),
};

QVector<LocalMediaFile> LibraryScanner::list(const QString &dirPath) {
    QDir dir(dirPath);
    if (!dir.exists()) {
        dir.mkpath(QStringLiteral("."));
    }
    QVector<LocalMediaFile> out;
    const auto files = dir.entryInfoList(QDir::Files, QDir::Time);
    for (const QFileInfo &fi : files) {
        const QString ext = fi.suffix().toLower();
        if (!kMediaExt.contains(ext)) {
            continue;
        }
        LocalMediaFile m;
        m.path = fi.absoluteFilePath();
        m.displayName = fi.completeBaseName();
        m.isVideo = kVideoExt.contains(ext);
        m.sizeBytes = fi.size();
        out.append(m);
    }
    return out;
}
