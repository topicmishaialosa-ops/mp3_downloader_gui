#pragma once

#include <QString>
#include <QVector>

/// Один запрос из пакетного ввода.
struct BatchQuery {
    QString raw;       ///< Исходная строка (после trim).
    QString artist;    ///< Исполнитель (пусто, если разделителя не было).
    QString title;     ///< Название (или весь текст, если без разделителя).
    QString url;       ///< Если строка была распознана как URL.
    bool isUrl() const { return !url.isEmpty(); }

    /// Полная поисковая строка для API.
    QString searchText() const {
        if (isUrl()) return url;
        if (artist.isEmpty()) return title;
        if (title.isEmpty()) return artist;
        return artist + QStringLiteral(" - ") + title;
    }
};

/// Парсер многострочного ввода.
class BatchQueries {
public:
    /// Разобрать текст на запросы. Пустые строки и `#`-комментарии пропускаются,
    /// нумерация в начале строки (`1.`, `12)`) снимается.
    static QVector<BatchQuery> parse(const QString &input);

private:
    static QString stripNumbering(const QString &s);
    static QString stripTrailingComment(const QString &s);
    static BatchQuery parseSingle(const QString &line);
    static bool isUrl(const QString &s);
};
