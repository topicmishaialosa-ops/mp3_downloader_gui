#pragma once

#include <QObject>
#include <QVector>

#include "downloadsource.h"
#include "track.h"

class DownloadManager : public QObject {
    Q_OBJECT
public:
    explicit DownloadManager(QObject *parent = nullptr);

    void setDownloadFolder(const QString &folder);
    QString downloadFolder() const;

    void enqueue(const Track &track, DownloadSource source, YtFormat ytFormat = YtFormat::Mp3);
    void cancelAll();

signals:
    void logLine(const QString &line);
    void taskProgress(int index, int percent, const QString &status);
    void taskFinished(int index, bool ok, const QString &message);
    void allIdle();

private:
    struct Task {
        Track track;
        DownloadSource source;
        YtFormat ytFormat;
        bool running = false;
        bool done = false;
        bool ok = false;
        QString message;
    };

    void startNext();
    void runTask(int index);

    QString m_folder;
    QVector<Task> m_tasks;
    int m_active = 0;
};
