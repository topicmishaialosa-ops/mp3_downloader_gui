#pragma once

#include <QString>
#include <QVector>

#include "track.h"

class PesniMeApi {
public:
    static QVector<Track> search(const QString &query, QString *error = nullptr);
    static QString download(const Track &track,
                            const QString &folder,
                            QString *error = nullptr);
    static QString streamUrl(const Track &track, QString *error = nullptr);
};
