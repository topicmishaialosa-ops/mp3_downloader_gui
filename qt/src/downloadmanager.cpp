#include "downloadmanager.h"

#include <QDateTime>
#include <QDir>
#include <QTimer>
#include <QtConcurrent>

#include "drivemusicapi.h"
#include "mp3partyapi.h"
#include "pesnimeapi.h"
#include "ytdlphelper.h"

DownloadManager::DownloadManager(QObject *parent) : QObject(parent) {}

void DownloadManager::setDownloadFolder(const QString &folder) {
    m_folder = folder;
    QDir().mkpath(m_folder);
}

QString DownloadManager::downloadFolder() const {
    return m_folder;
}

void DownloadManager::enqueue(const Track &track,
                             DownloadSource source,
                             YtFormat ytFormat) {
    Task t;
    t.track = track;
    t.source = source;
    t.ytFormat = ytFormat;
    m_tasks.append(t);
    emit logLine(QStringLiteral("[%1] В очередь: %2 — %3")
                     .arg(QDateTime::currentDateTime().toString(QStringLiteral("hh:mm:ss")),
                          track.artist,
                          track.title));
    startNext();
}

void DownloadManager::cancelAll() {
    // упрощённо: новые задачи не стартуют; текущая дорабатывает
    m_tasks.clear();
    m_active = 0;
    emit allIdle();
}

void DownloadManager::startNext() {
    if (m_active > 0) {
        return;
    }
    for (int i = 0; i < m_tasks.size(); ++i) {
        if (!m_tasks[i].running && !m_tasks[i].done) {
            runTask(i);
            return;
        }
    }
    emit allIdle();
}

void DownloadManager::runTask(int index) {
    if (index < 0 || index >= m_tasks.size()) {
        return;
    }
    m_active = 1;
    m_tasks[index].running = true;
    emit taskProgress(index, 0, QStringLiteral("Старт…"));

    const Task taskCopy = m_tasks[index];
    const QString folder = m_folder;

    (void)QtConcurrent::run([this, index, taskCopy, folder]() {
        QString err;
        QString path;
        const Track &track = taskCopy.track;

        switch (taskCopy.source) {
        case DownloadSource::Mp3Party:
            path = Mp3PartyApi::download(track, folder, &err);
            break;
        case DownloadSource::DriveMusic:
            path = DriveMusicApi::download(track, folder, &err);
            break;
        case DownloadSource::PesniMe:
            path = PesniMeApi::download(track, folder, &err);
            break;
        case DownloadSource::YtDlp:
            path = YtDlpHelper::download(track, folder, taskCopy.ytFormat, &err);
            break;
        }

        QTimer::singleShot(0, this, [this, index, path, err]() {
            if (index >= m_tasks.size()) {
                return;
            }
            m_tasks[index].running = false;
            m_tasks[index].done = true;
            m_tasks[index].ok = !path.isEmpty();
            m_tasks[index].message = m_tasks[index].ok ? path : err;
            m_active = 0;

            if (m_tasks[index].ok) {
                emit logLine(QStringLiteral("✅ %1").arg(path));
                emit taskProgress(index, 100, QStringLiteral("Готово"));
                emit taskFinished(index, true, path);
            } else {
                emit logLine(QStringLiteral("❌ %1").arg(err));
                emit taskFinished(index, false, err);
            }
            startNext();
        });
    });
}
