#pragma once

#include <QString>
#include <QVector>

#include "downloadsource.h"
#include "track.h"

class YtDlpHelper {
public:
    static QString resolveBinary(QString *error = nullptr);
    static QVector<Track> search(const QString &query, QString *error = nullptr);
    static QString download(const Track &track,
                            const QString &folder,
                            YtFormat format,
                            QString *error = nullptr);
    static QString streamUrl(const Track &track, YtFormat format, QString *error = nullptr);
};
