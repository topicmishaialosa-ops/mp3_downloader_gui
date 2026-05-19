#pragma once

#include <functional>

#include <QString>
#include <QVector>

#include "track.h"

class DriveMusicApi {
public:
    static QVector<Track> search(const QString &query, QString *error = nullptr);
    static QString download(const Track &track,
                            const QString &folder,
                            QString *error = nullptr);
    static QString streamUrl(const Track &track, QString *error = nullptr);
};
