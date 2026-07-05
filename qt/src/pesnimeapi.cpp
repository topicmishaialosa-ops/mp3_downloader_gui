#include "pesnimeapi.h"

#include <QDir>
#include <QFile>
#include <QRegularExpression>
#include <QSet>
#include <QUrl>

#include "appconstants.h"
#include "httpclient.h"

using namespace AppConstants;

static QString unescapeJson(const QString &s) {
    QString r = s;
    r.replace(QStringLiteral("\\\""), QStringLiteral("\""));
    r.replace(QStringLiteral("\\n"), QStringLiteral("\n"));
    r.replace(QStringLiteral("\\t"), QStringLiteral("\t"));
    return r;
}

static QString trackUrl(const QString &id) {
    return kPesniMeBase + QStringLiteral("/track/") + id;
}

static QString searchUrl(const QString &query) {
    return kPesniMeBase + QStringLiteral("/search/")
           + QString::fromUtf8(QUrl::toPercentEncoding(query.trimmed()))
           + QStringLiteral("?type=tracks");
}

static QVector<Track> extractTracks(const QString &body) {
    QVector<Track> results;
    QSet<QString> seen;

    QRegularExpression re(
        QStringLiteral(
            "\\\\\"id\\\\\":(\\d+),\\\\\"artist\\\\\":\\\\\"([^\"\\\\]*)\\\\\","
            "\\\\\"title\\\\\":\\\\\"([^\"\\\\]*)\\\\\","
            "\\\\\"version\\\\\":\\\\\"[^\"\\\\]*\\\\\",\\\\\"duration\\\\\":(\\d+),"
            "\\\\\"bitrate\\\\\":([^,]*),\\\\\"size\\\\\":([^,]*),"
            "\\\\\"play\\\\\":\\\\\"([^\"\\\\]+)\\\\\",\\\\\"download\\\\\":\\\\\"([^\"\\\\]+)\\\\\""));

    auto it = re.globalMatch(body);
    while (it.hasNext()) {
        const auto m = it.next();
        const QString id = m.captured(1).trimmed();
        if (id.isEmpty() || seen.contains(id)) {
            continue;
        }
        seen.insert(id);
        Track t;
        t.id = id;
        t.artist = unescapeJson(m.captured(2).trimmed());
        t.title = unescapeJson(m.captured(3).trimmed());
        const QString playUrl = m.captured(7).trimmed();
        const QString downloadUrl = m.captured(8).trimmed();
        t.url = !downloadUrl.isEmpty() ? downloadUrl : playUrl;
        t.source = DownloadSource::PesniMe;
        if (!t.title.isEmpty()) {
            results.append(t);
        }
    }
    return results;
}

QVector<Track> PesniMeApi::search(const QString &query, QString *error) {
    const QUrl url(searchUrl(query));
    const auto resp = HttpClient::get(url);
    if (!resp.ok()) {
        if (error) {
            *error = resp.error.isEmpty()
                         ? QStringLiteral("HTTP %1").arg(resp.status)
                         : resp.error;
        }
        return {};
    }

    const QString body = QString::fromUtf8(resp.body);
    auto results = extractTracks(body);

    const QStringList words = query.trimmed().toLower().split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
    auto match = [&](const Track &t) {
        const QString al = t.artist.toLower();
        const QString tl = t.title.toLower();
        for (const auto &w : words) {
            if (al.startsWith(w) || tl.startsWith(w)) return true;
        }
        return false;
    };
    QVector<Track> filtered;
    std::copy_if(results.begin(), results.end(), std::back_inserter(filtered), match);
    if (!filtered.isEmpty()) {
        return filtered.mid(0, 30);
    }

    if (results.isEmpty()) {
        // Попробуем через pesni.me (без music. поддомена)
        const QUrl url2(QStringLiteral("https://pesni.me/search/")
                        + QString::fromUtf8(QUrl::toPercentEncoding(query.trimmed())));
        const auto resp2 = HttpClient::get(url2);
        if (resp2.ok()) {
            results = extractTracks(QString::fromUtf8(resp2.body));
            QVector<Track> filtered2;
            std::copy_if(results.cbegin(), results.cend(), std::back_inserter(filtered2), match);
            if (!filtered2.isEmpty()) {
                return filtered2.mid(0, 30);
            }
        }
    }

    if (results.isEmpty() && error) {
        *error = QStringLiteral("Ничего не найдено на Pesni.me по запросу «%1».").arg(query);
    }
    return results.mid(0, 30);
}

Track PesniMeApi::fetchTrack(const QString &id) {
    Track t;
    t.id = id;
    t.source = DownloadSource::PesniMe;

    const QUrl url(trackUrl(id));
    const auto resp = HttpClient::get(url);
    if (!resp.ok()) {
        t.title = QStringLiteral("Трек #%1").arg(id);
        return t;
    }

    const QString body = QString::fromUtf8(resp.body);
    auto results = extractTracks(body);
    if (!results.isEmpty()) {
        t = results.first();
        t.source = DownloadSource::PesniMe;
        return t;
    }

    t.title = QStringLiteral("Трек #%1").arg(id);
    return t;
}

static QString sanitizeName(const QString &s) {
    QString r = s.trimmed();
    const QString bad = QStringLiteral("/\\:*?\"<>|");
    for (const QChar c : bad) {
        r.replace(c, QLatin1Char('_'));
    }
    return r;
}

QString PesniMeApi::download(const Track &track,
                             const QString &folder,
                             QString *error) {
    const QString dlUrl = track.url.contains(QStringLiteral("dw.pesni.me"))
                              ? track.url
                              : QString();

    QString downloadUrl = dlUrl;
    if (downloadUrl.isEmpty()) {
        // Получаем страницу трека и ищем download URL
        const QUrl pageUrl(trackUrl(track.id));
        const auto pageResp = HttpClient::get(pageUrl);
        if (!pageResp.ok()) {
            if (error) {
                *error = pageResp.error;
            }
            return {};
        }
        const auto tracks = extractTracks(QString::fromUtf8(pageResp.body));
        if (tracks.isEmpty()) {
            if (error) {
                *error = QStringLiteral("Не удалось найти ссылку на скачивание");
            }
            return {};
        }
        downloadUrl = tracks.first().url;
    }

    if (downloadUrl.isEmpty()) {
        if (error) {
            *error = QStringLiteral("Нет URL для скачивания");
        }
        return {};
    }

    const QString dest = QDir(folder).filePath(
        sanitizeName(track.artist + QStringLiteral(" - ") + track.title + QStringLiteral("_")
                      + track.id)
        + QStringLiteral(".mp3"));

    QMap<QString, QString> headers;
    headers.insert(QStringLiteral("Referer"), trackUrl(track.id));
    headers.insert(QStringLiteral("Accept"),
                   QStringLiteral("audio/mpeg,application/octet-stream,*/*;q=0.8"));

    const auto r = HttpClient::downloadToFile(QUrl(downloadUrl), dest, headers);
    if (!r.ok()) {
        if (error) {
            *error = r.error;
        }
        QFile::remove(dest);
        return {};
    }

    QFile f(dest);
    if (!f.exists() || f.size() < kMinDownloadBytes) {
        f.remove();
        if (error) {
            *error = QStringLiteral("файл слишком маленький или пустой");
        }
        return {};
    }

    return dest;
}

QString PesniMeApi::streamUrl(const Track &track, QString *error) {
    // Если URL уже play-url (pl.pesni.me)
    if (track.url.contains(QStringLiteral("pl.pesni.me"))) {
        return track.url;
    }
    // Иначе парсим страницу трека
    const QUrl pageUrl(trackUrl(track.id));
    const auto pageResp = HttpClient::get(pageUrl);
    if (!pageResp.ok()) {
        if (error) {
            *error = pageResp.error;
        }
        return {};
    }
    const auto tracks = extractTracks(QString::fromUtf8(pageResp.body));
    if (tracks.isEmpty()) {
        if (error) {
            *error = QStringLiteral("Не удалось получить URL потока");
        }
        return {};
    }
    // Берём play URL (первый, он короче)
    const QString body = QString::fromUtf8(pageResp.body);
    QRegularExpression playRe(
        QStringLiteral("\"play\":\"([^\"]+)\""));
    auto m = playRe.match(body);
    if (m.hasMatch()) {
        return m.captured(1);
    }
    if (error) {
        *error = QStringLiteral("Не найден play URL на странице");
    }
    return {};
}
