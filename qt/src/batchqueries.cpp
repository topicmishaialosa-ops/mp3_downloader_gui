#include "batchqueries.h"

#include <QRegularExpression>

QVector<BatchQuery> BatchQueries::parse(const QString &input) {
    QVector<BatchQuery> out;
#if QT_VERSION >= QT_VERSION_CHECK(5, 14, 0)
    const auto lines = input.split(QRegularExpression(QStringLiteral("[\\r\\n]+")), Qt::SkipEmptyParts);
#else
    const auto lines = input.split(QRegularExpression(QStringLiteral("[\\r\\n]+")), QString::SkipEmptyParts);
#endif
    for (const QString &raw : lines) {
        const QString stripped = stripNumbering(raw);
        const QString noComment = stripTrailingComment(stripped);
        const QString trimmed = noComment.trimmed();
        if (trimmed.isEmpty() || trimmed.startsWith(QLatin1Char('#'))) {
            continue;
        }
        out.append(parseSingle(trimmed));
    }
    return out;
}

QString BatchQueries::stripNumbering(const QString &s) {
    int i = 0;
    const int n = s.size();
    auto isSpace = [](QChar c) { return c.isSpace(); };
    while (i < n && isSpace(s[i])) i++;
    const int startDigits = i;
    while (i < n && s[i].isDigit()) i++;
    if (i == startDigits) return s;
    while (i < n && isSpace(s[i])) i++;
    if (i >= n || (s[i] != QLatin1Char('.') && s[i] != QLatin1Char(')'))) {
        return s;
    }
    i++;
    while (i < n && isSpace(s[i])) i++;
    return s.mid(i);
}

QString BatchQueries::stripTrailingComment(const QString &s) {
    const int idx = s.indexOf(QStringLiteral(" #"));
    if (idx < 0) return s;
    return s.left(idx);
}

bool BatchQueries::isUrl(const QString &s) {
    const QString t = s.trimmed();
    return t.startsWith(QStringLiteral("http://"), Qt::CaseInsensitive)
        || t.startsWith(QStringLiteral("https://"), Qt::CaseInsensitive);
}

BatchQuery BatchQueries::parseSingle(const QString &line) {
    if (isUrl(line)) {
        BatchQuery q;
        q.raw = line;
        q.url = line.trimmed();
        return q;
    }
    static const QVector<QString> seps = {
        QStringLiteral(" - "),
        QString::fromUtf8(" \xE2\x80\x93 "),  // en-dash
        QString::fromUtf8(" \xE2\x80\x94 "),  // em-dash
    };
    for (const QString &sep : seps) {
        const int idx = line.indexOf(sep);
        if (idx > 0) {
            BatchQuery q;
            q.raw = line;
            q.artist = line.left(idx).trimmed();
            q.title = line.mid(idx + sep.size()).trimmed();
            return q;
        }
    }
    BatchQuery q;
    q.raw = line;
    q.title = line.trimmed();
    return q;
}
