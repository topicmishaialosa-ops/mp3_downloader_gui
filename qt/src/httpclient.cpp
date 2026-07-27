#include "httpclient.h"

#include <QEventLoop>
#include <QFile>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QRegularExpression>
#include <QTimer>
#include <QUrlQuery>

#include "appconstants.h"

using namespace AppConstants;

static QNetworkRequest makeRequest(const QUrl &url,
                                   const QMap<QString, QString> &headers) {
    QNetworkRequest req(url);
    req.setHeader(QNetworkRequest::UserAgentHeader, kUserAgent);
    for (auto it = headers.constBegin(); it != headers.constEnd(); ++it) {
        req.setRawHeader(it.key().toUtf8(), it.value().toUtf8());
    }
    return req;
}

HttpClient::Response HttpClient::get(const QUrl &url,
                                     const QMap<QString, QString> &headers,
                                     int timeoutMs) {
    Response out;
    QNetworkAccessManager nam;
    QEventLoop loop;
    QTimer timer;
    timer.setSingleShot(true);

    auto *reply = nam.get(makeRequest(url, headers));
    QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
    QObject::connect(&timer, &QTimer::timeout, &loop, &QEventLoop::quit);
    timer.start(timeoutMs);
    loop.exec();

    if (!timer.isActive()) {
        reply->abort();
        out.error = QStringLiteral("Таймаут запроса");
        reply->deleteLater();
        return out;
    }

    out.status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    if (reply->error() != QNetworkReply::NoError) {
        out.error = reply->errorString();
    } else {
        out.rawHeaders = reply->rawHeaderPairs().isEmpty()
            ? QByteArray()
            : [&]() {
                QByteArray h;
                for (const auto &pair : reply->rawHeaderPairs()) {
                    h.append(pair.first);
                    h.append(": ");
                    h.append(pair.second);
                    h.append("\r\n");
                }
                return h;
            }();
        out.body = reply->readAll();
    }
    reply->deleteLater();
    return out;
}

QString HttpClient::extractFileNameFromDisposition(const QByteArray &rawHeaders) {
    QString headers = QString::fromUtf8(rawHeaders);

    // filename*=UTF-8''...  (RFC 5987)
    QRegularExpression utf8Re(QStringLiteral("filename\\*=UTF-8''([^;\\s]+)"),
                             QRegularExpression::CaseInsensitiveOption);
    auto utf8Match = utf8Re.match(headers);
    if (utf8Match.hasMatch()) {
        return QUrl::fromPercentEncoding(utf8Match.captured(1).toUtf8());
    }

    // filename="..." или filename=...
    QRegularExpression plainRe(QStringLiteral("filename=\"?([^\";\\s]+)\"?"),
                              QRegularExpression::CaseInsensitiveOption);
    auto plainMatch = plainRe.match(headers);
    if (plainMatch.hasMatch()) {
        return QUrl::fromPercentEncoding(plainMatch.captured(1).toUtf8());
    }

    return {};
}

QString HttpClient::cleanDispositionFilename(const QString &name) {
    QString s = name;
    // Определить расширение
    QString ext;
    if (s.endsWith(QStringLiteral(".mp3"), Qt::CaseInsensitive)) ext = QStringLiteral(".mp3");
    else if (s.endsWith(QStringLiteral(".m4a"), Qt::CaseInsensitive)) ext = QStringLiteral(".m4a");
    else if (s.endsWith(QStringLiteral(".mp4"), Qt::CaseInsensitive)) ext = QStringLiteral(".mp4");
    if (!ext.isEmpty()) s.chop(ext.length());

    // Убрать track<digits> в начале
    QRegularExpression trackRe(QStringLiteral("^track\\d+"), QRegularExpression::CaseInsensitiveOption);
    s.replace(trackRe, QString());
    s = s.trimmed();

    // Убрать pesnifm/mp3party/ pesni.me суффиксы в конце
    QRegularExpression suffixRe(QStringLiteral("\\s*pesni(?:fm|me|party).*$"), QRegularExpression::CaseInsensitiveOption);
    s.replace(suffixRe, QString());
    s = s.trimmed();

    return s.isEmpty() ? QStringLiteral("track") + ext : s + ext;
}

HttpClient::Response HttpClient::downloadToFile(
    const QUrl &url,
    const QString &destPath,
    const QMap<QString, QString> &headers,
    std::function<bool(qint64, qint64)> progress) {
    Q_UNUSED(progress);
    Response out;
    QNetworkAccessManager nam;
    QEventLoop loop;

    auto *reply = nam.get(makeRequest(url, headers));
    QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
    loop.exec();

    out.status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    if (reply->error() != QNetworkReply::NoError) {
        out.error = reply->errorString();
        reply->deleteLater();
        return out;
    }

    out.rawHeaders = [&]() {
        QByteArray h;
        for (const auto &pair : reply->rawHeaderPairs()) {
            h.append(pair.first);
            h.append(": ");
            h.append(pair.second);
            h.append("\r\n");
        }
        return h;
    }();

    QFile file(destPath);
    if (!file.open(QIODevice::WriteOnly)) {
        out.error = QStringLiteral("Не удалось создать файл");
        reply->deleteLater();
        return out;
    }
    file.write(reply->readAll());
    file.close();
    if (out.status == 0) {
        out.status = 200;
    }
    reply->deleteLater();
    return out;
}
