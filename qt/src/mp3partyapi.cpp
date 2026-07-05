#include "mp3partyapi.h"

#include <QDir>
#include <QFile>
#include <QRegularExpression>
#include <QUrlQuery>

#include "appconstants.h"
#include "httpclient.h"

using namespace AppConstants;

static QString onlineUrlForId(const QString &id) {
    return QStringLiteral("https://dl2.mp3party.net/online/%1.mp3").arg(id);
}

static QString downloadUrl(const QString &id) {
    return QStringLiteral("https://dl2.mp3party.net/download/%1").arg(id);
}

static void pushUnique(QStringList *list, const QString &url) {
    if (url.startsWith(QStringLiteral("http")) && !list->contains(url)) {
        list->append(url);
    }
}

static QVector<Track> parsePanels(const QString &html) {
    QVector<Track> out;
    QRegularExpression panelRe(
        QStringLiteral(
            "data-js-id=\"(\\d+)\"[^>]*data-js-artist-name=\"([^\"]*)\"[^>]*data-js-song-title=\"([^\"]*)\""),
        QRegularExpression::DotMatchesEverythingOption);
    auto it = panelRe.globalMatch(html);
    while (it.hasNext()) {
        const auto m = it.next();
        Track t;
        t.id = m.captured(1);
        t.artist = m.captured(2);
        t.title = m.captured(3);
        t.url = onlineUrlForId(t.id);
        t.source = DownloadSource::Mp3Party;
        if (!t.id.isEmpty() && !t.title.isEmpty()) {
            bool dup = false;
            for (const auto &x : out) {
                if (x.id == t.id) {
                    dup = true;
                    break;
                }
            }
            if (!dup) {
                out.append(t);
            }
        }
    }
    return out;
}

QVector<Track> Mp3PartyApi::search(const QString &query, QString *error) {
    const QUrl url(QStringLiteral("https://mp3party.net/search?q=")
                   + QString::fromUtf8(QUrl::toPercentEncoding(query.trimmed())));

    const auto resp = HttpClient::get(url);
    if (!resp.ok()) {
        if (error) {
            *error = resp.error.isEmpty()
                         ? QStringLiteral("HTTP %1").arg(resp.status)
                         : resp.error;
        }
        return {};
    }

    auto results = parsePanels(QString::fromUtf8(resp.body));
    if (results.isEmpty()) {
        QRegularExpression anyRe(
            QStringLiteral(
                "data-js-id=\"(\\d+)\".*?data-js-artist-name=\"([^\"]*)\".*?data-js-song-title=\"([^\"]*)\""),
            QRegularExpression::DotMatchesEverythingOption);
        auto it = anyRe.globalMatch(QString::fromUtf8(resp.body));
        while (it.hasNext()) {
            const auto m = it.next();
            Track t;
            t.id = m.captured(1);
            t.artist = m.captured(2);
            t.title = m.captured(3);
            t.url = onlineUrlForId(t.id);
            t.source = DownloadSource::Mp3Party;
            if (!t.id.isEmpty() && !t.title.isEmpty()) {
                results.append(t);
            }
        }
    }

    if (results.isEmpty() && error) {
        *error = QStringLiteral("Ничего не найдено по запросу «%1».").arg(query);
    }
    return results;
}

Track Mp3PartyApi::fetchTrack(const QString &id) {
    Track t;
    t.id = id;
    t.source = DownloadSource::Mp3Party;
    t.url = onlineUrlForId(id);

    const QUrl url(QStringLiteral("https://mp3party.net/music/%1").arg(id));
    const auto resp = HttpClient::get(url);
    if (!resp.ok()) {
        t.title = QStringLiteral("Трек #%1").arg(id);
        return t;
    }

    const QString html = QString::fromUtf8(resp.body);

    auto panels = parsePanels(html);
    if (!panels.isEmpty()) {
        t.artist = panels.first().artist;
        t.title = panels.first().title;
        return t;
    }

    QRegularExpression ogRe(QStringLiteral("property=\"og:title\"\\s+content=\"([^\"]+)\""));
    auto ogM = ogRe.match(html);
    if (ogM.hasMatch()) {
        const QString content = ogM.captured(1);
        const auto parts = content.split(QStringLiteral(" - "), Qt::SkipEmptyParts);
        if (parts.size() >= 2) {
            t.artist = parts[0].trimmed();
            t.title = parts.mid(1).join(QStringLiteral(" - ")).trimmed();
        } else {
            t.title = content.trimmed();
        }
        return t;
    }

    t.title = QStringLiteral("Трек #%1").arg(id);
    return t;
}

static QStringList downloadCandidates(const QString &pageBody, const Track &track) {
    QStringList urls;
    QRegularExpression btnRe(
        QStringLiteral("href=\"(https?://[^\"]+)\"[^>]*data-track-id=\"%1\"")
            .arg(QRegularExpression::escape(track.id)));
    auto it = btnRe.globalMatch(pageBody);
    while (it.hasNext()) {
        pushUnique(&urls, it.next().captured(1));
    }

    QRegularExpression urlRe(
        QStringLiteral("data-js-id=\"%1\"[^>]*data-js-url=\"(https?://[^\"]+)\"|data-js-url=\"(https?://[^\"]+)\"[^>]*data-js-id=\"%1\"")
            .arg(QRegularExpression::escape(track.id),
                 QRegularExpression::escape(track.id)),
        QRegularExpression::DotMatchesEverythingOption);
    auto it2 = urlRe.globalMatch(pageBody);
    while (it2.hasNext()) {
        const auto m = it2.next();
        pushUnique(&urls, m.captured(1).isEmpty() ? m.captured(2) : m.captured(1));
    }

    pushUnique(&urls, downloadUrl(track.id));
    if (track.url.startsWith(QStringLiteral("http"))) {
        pushUnique(&urls, track.url);
    }
    pushUnique(&urls, onlineUrlForId(track.id));
    return urls;
}

static QString sanitizeName(const QString &s) {
    QString r = s.trimmed();
    const QString bad = QStringLiteral("/\\:*?\"<>|");
    for (const QChar c : bad) {
        r.replace(c, QLatin1Char('_'));
    }
    return r;
}

QString Mp3PartyApi::download(const Track &track,
                              const QString &folder,
                              QString *error,
                              std::function<bool(qint64, qint64)> progress) {
    const QString trackPage =
        QStringLiteral("https://mp3party.net/music/%1").arg(track.id);
    const auto pageResp = HttpClient::get(QUrl(trackPage));
    if (!pageResp.ok()) {
        if (error) {
            *error = QStringLiteral("Страница трека: %1")
                         .arg(pageResp.error.isEmpty() ? QString::number(pageResp.status)
                                                       : pageResp.error);
        }
        return {};
    }

    const QString page = QString::fromUtf8(pageResp.body);
    const auto candidates = downloadCandidates(page, track);
    const QString dest = QDir(folder).filePath(
        sanitizeName(track.artist + QStringLiteral(" - ") + track.title) + QStringLiteral(".mp3"));

    QMap<QString, QString> headers;
    headers.insert(QStringLiteral("Referer"), trackPage);
    headers.insert(QStringLiteral("Origin"), QStringLiteral("https://mp3party.net"));
    headers.insert(QStringLiteral("Accept"),
                   QStringLiteral("audio/mpeg,application/octet-stream,*/*;q=0.8"));

    QString lastErr;
    for (const QString &url : candidates) {
        const auto r = HttpClient::downloadToFile(QUrl(url), dest, headers, progress);
        if (r.ok()) {
            QFile f(dest);
            if (f.open(QIODevice::ReadOnly) && f.size() >= kMinDownloadBytes) {
                // Переименовать по Content-Disposition
                const QString cdName = HttpClient::extractFileNameFromDisposition(r.rawHeaders);
                if (!cdName.isEmpty() && cdName.endsWith(QStringLiteral(".mp3"), Qt::CaseInsensitive)) {
                    const QString newPath = QDir(folder).filePath(cdName);
                    if (newPath != dest) {
                        QFile::rename(dest, newPath);
                        return newPath;
                    }
                }
                return dest;
            }
            f.remove();
            lastErr = QStringLiteral("файл слишком маленький");
        } else {
            lastErr = r.error.isEmpty() ? QStringLiteral("HTTP %1").arg(r.status) : r.error;
        }
        QFile::remove(dest);
    }

    if (error) {
        *error = QStringLiteral("MP3Party: %1").arg(lastErr);
    }
    return {};
}

QString Mp3PartyApi::streamUrl(const Track &track) {
    if (track.url.startsWith(QStringLiteral("http"))) {
        return track.url;
    }
    return QStringLiteral("https://dl2.mp3party.net/online/%1.mp3").arg(track.id);
}
