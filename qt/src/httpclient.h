#pragma once

#include <functional>

#include <QByteArray>
#include <QMap>
#include <QString>
#include <QUrl>

class HttpClient {
public:
    struct Response {
        int status = 0;
        QByteArray body;
        QString error;
        QByteArray rawHeaders;
        bool ok() const { return status >= 200 && status < 300 && error.isEmpty(); }
    };

    static Response get(const QUrl &url,
                        const QMap<QString, QString> &headers = {},
                        int timeoutMs = 60000);
    static Response downloadToFile(const QUrl &url,
                                   const QString &destPath,
                                   const QMap<QString, QString> &headers,
                                   std::function<bool(qint64, qint64)> progress = nullptr);

    static QString extractFileNameFromDisposition(const QByteArray &rawHeaders);
    static QString cleanDispositionFilename(const QString &name);
};
