#include "drivemusicapi.h"

#include <QDir>
#include <QFile>
#include <QRegularExpression>
#include <QSet>
#include <QUrl>
#include <algorithm>

#include "appconstants.h"
#include "httpclient.h"

using namespace AppConstants;

static QString decodeHtml(const QString &s) {
    QString r = s;
    r.replace(QStringLiteral("&amp;"), QStringLiteral("&"));
    r.replace(QStringLiteral("&quot;"), QStringLiteral("\""));
    r.replace(QStringLiteral("&#39;"), QStringLiteral("'"));
    return r;
}

static QString pageUrl(const Track &track) {
    const QString u = track.url.trimmed();
    if (u.contains(QStringLiteral("drivemusic.me")) && u.endsWith(QStringLiteral(".html"))) {
        return u;
    }
    if (u.startsWith(QLatin1Char('/')) && u.endsWith(QStringLiteral(".html"))) {
        return kDriveMusicBase + u;
    }
    return {};
}

static QStringList extractMp3Urls(const QString &html) {
    QStringList urls;
    QRegularExpression re(
        QStringLiteral("https://[a-z0-9.-]*drivemusic\\.me/dl/[^\"\\s<>]+\\.mp3"),
        QRegularExpression::CaseInsensitiveOption);
    auto it = re.globalMatch(html);
    while (it.hasNext()) {
        urls.append(it.next().captured(0));
    }
    urls.removeDuplicates();
    std::sort(urls.begin(), urls.end(), [](const QString &a, const QString &b) {
        const bool ao = a.contains(QStringLiteral("/dl/online/"));
        const bool bo = b.contains(QStringLiteral("/dl/online/"));
        if (ao != bo) {
            return ao;
        }
        return a.length() < b.length();
    });
    return urls;
}

QVector<Track> DriveMusicApi::search(const QString &query, QString *error) {
    const QUrl url(kDriveMusicBase + QStringLiteral("/?do=search&subaction=search&story=")
                   + QString::fromUtf8(QUrl::toPercentEncoding(query.trimmed())));

    QMap<QString, QString> headers;
    headers.insert(QStringLiteral("Referer"), kDriveMusicBase);

    const auto resp = HttpClient::get(url, headers);
    if (!resp.ok()) {
        if (error) {
            *error = resp.error.isEmpty()
                         ? QStringLiteral("HTTP %1").arg(resp.status)
                         : resp.error;
        }
        return {};
    }

    const QString body = QString::fromUtf8(resp.body);
    QVector<Track> results;
    QSet<QString> seen;

    QRegularExpression itemRe(
        QStringLiteral(
            "(?s)href=\"(/[a-z0-9_]+/(\\d+)-[^\"]+\\.html)\"[^>]*class=\"popular-play-author\"[^>]*>([^<]*)</a>"
            ".*?popular-play-composition.*?(?:<a[^>]*>)?([^<]*)"),
        QRegularExpression::CaseInsensitiveOption);

    auto it = itemRe.globalMatch(body);
    while (it.hasNext()) {
        const auto m = it.next();
        const QString id = m.captured(2).trimmed();
        if (id.isEmpty() || seen.contains(id)) {
            continue;
        }
        seen.insert(id);
        const QString path = m.captured(1).trimmed();
        const QString page = path.startsWith(QStringLiteral("http")) ? path : kDriveMusicBase + path;
        Track t;
        t.id = id;
        t.title = decodeHtml(m.captured(3).trimmed());
        t.artist = decodeHtml(m.captured(4).trimmed());
        t.url = page;
        t.source = DownloadSource::DriveMusic;
        if (!t.title.isEmpty()) {
            results.append(t);
        }
    }

    if (results.isEmpty() && error) {
        *error = QStringLiteral("Ничего не найдено на DriveMusic по запросу «%1».").arg(query);
    }
    return results.mid(0, 30);
}

static QString sanitizeName(const QString &s) {
    QString r = s.trimmed();
    const QString bad = QStringLiteral("/\\:*?\"<>|");
    for (const QChar c : bad) {
        r.replace(c, QLatin1Char('_'));
    }
    return r;
}

QString DriveMusicApi::download(const Track &track,
                                const QString &folder,
                                QString *error) {
    const QString page = pageUrl(track);
    if (page.isEmpty()) {
        if (error) {
            *error = QStringLiteral(
                "DriveMusic: нет ссылки на страницу трека — найдите трек через поиск.");
        }
        return {};
    }

    QMap<QString, QString> headers;
    headers.insert(QStringLiteral("Referer"), kDriveMusicBase);

    const auto pageResp = HttpClient::get(QUrl(page), headers);
    if (!pageResp.ok()) {
        if (error) {
            *error = QStringLiteral("Страница: %1").arg(pageResp.error);
        }
        return {};
    }

    const auto urls = extractMp3Urls(QString::fromUtf8(pageResp.body));
    const QString dest = QDir(folder).filePath(
        sanitizeName(track.artist + QStringLiteral(" - ") + track.title + QStringLiteral("_")
                      + track.id)
        + QStringLiteral(".mp3"));

    QMap<QString, QString> dlHeaders = headers;
    dlHeaders.insert(QStringLiteral("Referer"), page);
    dlHeaders.insert(QStringLiteral("Origin"), kDriveMusicBase);
    dlHeaders.insert(QStringLiteral("Accept"),
                     QStringLiteral("audio/mpeg,application/octet-stream,*/*;q=0.8"));

    QString lastErr;
    for (const QString &url : urls) {
        const auto r = HttpClient::downloadToFile(QUrl(url), dest, dlHeaders);
        if (r.ok()) {
            QFile f(dest);
            if (f.exists() && f.size() >= kMinDownloadBytes) {
                return dest;
            }
            f.remove();
            lastErr = QStringLiteral("файл слишком маленький");
        } else {
            lastErr = r.error;
        }
        QFile::remove(dest);
    }

    if (error) {
        *error = QStringLiteral("DriveMusic: %1").arg(lastErr);
    }
    return {};
}

QString DriveMusicApi::streamUrl(const Track &track, QString *error) {
    const QString page = pageUrl(track);
    QMap<QString, QString> headers;
    headers.insert(QStringLiteral("Referer"), kDriveMusicBase);
    const auto pageResp = HttpClient::get(QUrl(page), headers);
    if (!pageResp.ok()) {
        if (error) {
            *error = pageResp.error;
        }
        return {};
    }
    const auto urls = extractMp3Urls(QString::fromUtf8(pageResp.body));
    QString u;
    for (const QString &url : urls) {
        if (url.contains(QStringLiteral("/dl/online/"))) {
            u = url;
            break;
        }
    }
    if (u.isEmpty() && !urls.isEmpty()) {
        u = urls.first();
    }
    if (u.isEmpty() && error) {
        *error = QStringLiteral("На странице нет MP3 URL");
    }
    return u;
}
