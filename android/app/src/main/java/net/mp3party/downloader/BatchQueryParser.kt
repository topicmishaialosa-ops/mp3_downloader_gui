package net.mp3party.downloader

/**
 * Пакетный (batch) парсер для многострочного ввода.
 *
 * Формат (по строке на трек):
 *   "Исполнитель - Название"      — дефис, en-dash или em-dash
 *   "Просто название"             — без разделителя
 *   "https://..."                 — URL
 *
 * Допускается нумерация ("1. ", "12)") и комментарии после "# ".
 */
object BatchQueryParser {
    data class Query(
        val raw: String,
        val artist: String,
        val title: String,
        val url: String?,
    ) {
        val isUrl: Boolean get() = !url.isNullOrEmpty()
        fun searchText(): String = when {
            isUrl -> url!!
            artist.isEmpty() -> title
            title.isEmpty() -> artist
            else -> "$artist - $title"
        }
    }

    fun parse(input: String): List<Query> {
        val out = mutableListOf<Query>()
        for (line in input.split(Regex("[\r\n]+"))) {
            val cleaned = stripNumbering(line).let(::stripTrailingComment).trim()
            if (cleaned.isEmpty() || cleaned.startsWith("#")) continue
            out.add(parseSingle(cleaned))
        }
        return out
    }

    private fun stripNumbering(s: String): String {
        var i = 0
        while (i < s.length && s[i].isWhitespace()) i++
        val startDigits = i
        while (i < s.length && s[i].isDigit()) i++
        if (i == startDigits) return s
        while (i < s.length && s[i].isWhitespace()) i++
        if (i >= s.length || (s[i] != '.' && s[i] != ')')) return s
        i++ // съедаем . или )
        while (i < s.length && s[i].isWhitespace()) i++
        return s.substring(i)
    }

    private fun stripTrailingComment(s: String): String {
        val idx = s.indexOf(" #")
        return if (idx < 0) s else s.substring(0, idx)
    }

    private fun isUrl(s: String): Boolean {
        val t = s.trim().lowercase()
        return t.startsWith("http://") || t.startsWith("https://")
    }

    private fun parseSingle(line: String): Query {
        if (isUrl(line)) {
            return Query(line, "", "", line.trim())
        }
        val seps = listOf(" - ", " \u2013 ", " \u2014 ")
        for (sep in seps) {
            val idx = line.indexOf(sep)
            if (idx > 0) {
                return Query(
                    raw = line,
                    artist = line.substring(0, idx).trim(),
                    title = line.substring(idx + sep.length).trim(),
                    url = null,
                )
            }
        }
        return Query(line, "", line.trim(), null)
    }
}
