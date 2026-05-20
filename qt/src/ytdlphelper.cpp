#include "ytdlphelper.h"

#include <QDir>
#include <QFileInfo>
#include <QProcess>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QtGlobal>

static QPair<QString, QString> splitTitle(const QString &full, const QString &channel) {
    const int idx = full.indexOf(QStringLiteral(" - "));
    if (idx > 0) {
        return {full.left(idx).trimmed(), full.mid(idx + 3).trimmed()};
    }
    if (!channel.isEmpty() && channel != full) {
        return {channel.trimmed(), full.trimmed()};
    }
    return {QStringLiteral("YouTube"), full.trimmed()};
}

QString YtDlpHelper::resolveBinary(QString *error) {
    QStringList candidates;
    const QString home = QDir::homePath();
#if defined(Q_OS_WIN)
    candidates << QDir(home).filePath(
        QStringLiteral("yt-dlp-util/.yt-dlp-venv/Scripts/yt-dlp.exe"));
#else
    candidates << QDir(home).filePath(QStringLiteral("yt-dlp-util/.yt-dlp-venv/bin/yt-dlp"));
#endif
    candidates << QStandardPaths::findExecutable(QStringLiteral("yt-dlp"));
    candidates << QStandardPaths::findExecutable(QStringLiteral("yt-dlp.exe"));
    for (const QString &p : candidates) {
        if (!p.isEmpty() && QFileInfo::exists(p)) {
            return p;
        }
    }
    if (error) {
#if defined(Q_OS_WIN)
        *error = QStringLiteral(
            "yt-dlp не найден. Установите yt-dlp в PATH "
            "или в %USERPROFILE%\\yt-dlp-util\\.yt-dlp-venv\\Scripts\\");
#else
        *error = QStringLiteral(
            "yt-dlp не найден. Установите yt-dlp в PATH "
            "или в ~/yt-dlp-util/.yt-dlp-venv/bin/");
#endif
    }
    return {};
}

QVector<Track> YtDlpHelper::search(const QString &query, QString *error) {
    const QString ytdlp = resolveBinary(error);
    if (ytdlp.isEmpty()) {
        return {};
    }

    QProcess proc;
    proc.setProgram(ytdlp);
    proc.setArguments({
        QStringLiteral("--flat-playlist"),
        QStringLiteral("--playlist-end"),
        QStringLiteral("20"),
        QStringLiteral("--print"),
        QStringLiteral("%(id)s|||%(title)s|||%(channel)s"),
        QStringLiteral("ytsearch20:") + query.trimmed(),
    });
    proc.setProcessChannelMode(QProcess::MergedChannels);
    proc.start();
    if (!proc.waitForFinished(45000)) {
        proc.kill();
        if (error) {
            *error = QStringLiteral("Таймаут поиска YouTube");
        }
        return {};
    }

    if (proc.exitCode() != 0) {
        if (error) {
            *error = QString::fromUtf8(proc.readAllStandardOutput()).trimmed();
            if (error->isEmpty()) {
                *error = QStringLiteral("yt-dlp: код %1").arg(proc.exitCode());
            }
        }
        return {};
    }

    QVector<Track> results;
    const QString out = QString::fromUtf8(proc.readAllStandardOutput());
    for (const QString &line : out.split(QLatin1Char('\n'))) {
        const QStringList parts = line.trimmed().split(QStringLiteral("|||"));
        if (parts.size() < 2) {
            continue;
        }
        const QString id = parts[0].trimmed();
        const QString fullTitle = parts[1].trimmed();
        const QString channel = parts.size() > 2 ? parts[2].trimmed() : QString();
        if (id.isEmpty() || id == QStringLiteral("NA") || fullTitle.isEmpty()) {
            continue;
        }
        const auto pair = splitTitle(fullTitle, channel);
        Track t;
        t.id = id;
        t.artist = pair.first;
        t.title = pair.second;
        t.url = QStringLiteral("https://www.youtube.com/watch?v=") + id;
        t.source = DownloadSource::YtDlp;
        bool dup = false;
        for (const auto &x : results) {
            if (x.id == id) {
                dup = true;
                break;
            }
        }
        if (!dup) {
            results.append(t);
        }
    }

    if (results.isEmpty() && error) {
        *error = QStringLiteral("Ничего не найдено на YouTube по запросу «%1».").arg(query);
    }
    return results;
}

QString YtDlpHelper::download(const Track &track,
                              const QString &folder,
                              YtFormat format,
                              QString *error) {
    const QString ytdlp = resolveBinary(error);
    if (ytdlp.isEmpty()) {
        return {};
    }

    QString target = track.url;
    if (!target.startsWith(QStringLiteral("http"))) {
        target = QStringLiteral("ytsearch1:") + track.artist + QStringLiteral(" - ") + track.title;
    }

    const QString ext = format == YtFormat::Mp4 ? QStringLiteral("mp4") : QStringLiteral("mp3");
    const QString archive =
        QDir(folder).filePath(QStringLiteral(".yt-dlp-archive-") + ext);
    const QString outTpl = QDir(folder).filePath(QStringLiteral("%(title)s.%(ext)s"));

    QStringList args = {
        QStringLiteral("--force-ipv4"),
        QStringLiteral("--continue"),
        QStringLiteral("--download-archive"),
        archive,
        QStringLiteral("--no-playlist"),
        QStringLiteral("--newline"),
        QStringLiteral("-o"),
        outTpl,
        QStringLiteral("--print"),
        QStringLiteral("after_move:AFTERMOVE:%(filepath)s"),
        target,
    };

    if (format == YtFormat::Mp3) {
        args << QStringLiteral("-x") << QStringLiteral("--audio-format") << QStringLiteral("mp3")
             << QStringLiteral("--audio-quality") << QStringLiteral("0");
    } else {
        args << QStringLiteral("-f") << QStringLiteral("bv*+ba/b")
             << QStringLiteral("--merge-output-format") << QStringLiteral("mp4");
    }

    QProcess proc;
    proc.setProgram(ytdlp);
    proc.setArguments(args);
    proc.setProcessChannelMode(QProcess::MergedChannels);
    proc.start();
    if (!proc.waitForFinished(-1)) {
        proc.kill();
        if (error) {
            *error = QStringLiteral("yt-dlp: прервано");
        }
        return {};
    }

    const QString output = QString::fromUtf8(proc.readAllStandardOutput());
    QString completed;
    for (const QString &line : output.split(QLatin1Char('\n'))) {
        if (line.contains(QStringLiteral("AFTERMOVE:"))) {
            completed = line.section(QStringLiteral("AFTERMOVE:"), 1).trimmed();
        }
    }

    if (proc.exitCode() == 0 && !completed.isEmpty()) {
        return completed;
    }

    if (error) {
        *error = output.trimmed().isEmpty()
                     ? QStringLiteral("yt-dlp: код %1").arg(proc.exitCode())
                     : output.trimmed();
    }
    return {};
}

QString YtDlpHelper::streamUrl(const Track &track, YtFormat format, QString *error) {
    const QString ytdlp = resolveBinary(error);
    if (ytdlp.isEmpty()) {
        return {};
    }
    QString target = track.url;
    if (!target.startsWith(QStringLiteral("http"))) {
        target = QStringLiteral("ytsearch1:") + track.artist + QStringLiteral(" - ") + track.title;
    }
    const QString formatArg = format == YtFormat::Mp4
                                  ? QStringLiteral("best[height<=720][ext=mp4]/best[ext=mp4]/best")
                                  : QStringLiteral("bestaudio[ext=m4a]/bestaudio/best");

    QProcess proc;
    proc.setProgram(ytdlp);
    proc.setArguments({
        QStringLiteral("--no-playlist"),
        QStringLiteral("-g"),
        QStringLiteral("-f"),
        formatArg,
        target,
    });
    proc.start();
    if (!proc.waitForFinished(60000)) {
        proc.kill();
        if (error) {
            *error = QStringLiteral("Таймаут получения URL потока");
        }
        return {};
    }
    const QString out = QString::fromUtf8(proc.readAllStandardOutput());
    for (const QString &line : out.split(QLatin1Char('\n'))) {
        const QString t = line.trimmed();
        if (t.startsWith(QStringLiteral("http"))) {
            return t;
        }
    }
    if (error) {
        *error = QString::fromUtf8(proc.readAllStandardError()).trimmed();
        if (error->isEmpty()) {
            *error = QStringLiteral("yt-dlp не вернул URL потока");
        }
    }
    return {};
}
