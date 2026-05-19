#pragma once

#include <functional>

#include <QString>
#include <QVector>

#include "track.h"

class Mp3PartyApi {
public:
    static QVector<Track> search(const QString &query, QString *error = nullptr);
    static QString download(const Track &track,
                            const QString &folder,
                            QString *error = nullptr,
                            std::function<bool(qint64, qint64)> progress = nullptr);
    static QString streamUrl(const Track &track);
};
