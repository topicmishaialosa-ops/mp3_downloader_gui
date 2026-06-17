#pragma once

#include <QString>

#include "downloadsource.h"

struct Track {
    QString id;
    QString artist;
    QString title;
    QString url;
    QString streamUrl;
    DownloadSource source = DownloadSource::Mp3Party;
};
