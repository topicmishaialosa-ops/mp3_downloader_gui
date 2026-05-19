#pragma once

#include <QString>
#include <QVector>

struct LocalMediaFile {
    QString path;
    QString displayName;
    bool isVideo = false;
    qint64 sizeBytes = 0;
};

class LibraryScanner {
public:
    static QVector<LocalMediaFile> list(const QString &dirPath);
};
