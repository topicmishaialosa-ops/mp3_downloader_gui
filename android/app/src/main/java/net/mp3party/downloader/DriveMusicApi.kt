package net.mp3party.downloader

import okhttp3.Cookie
import okhttp3.CookieJar
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.net.URLEncoder
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.regex.Pattern
import java.net.URLDecoder

object DriveMusicApi {
    private const val BASE = "https://ru.drivemusic.me"
    private const val USER_AGENT =
        "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36"
    private const val MIN_DOWNLOAD_BYTES = 50 * 1024

    private val cookieStore = ConcurrentHashMap<String, MutableList<Cookie>>()

    private val client = OkHttpClient.Builder()
        .cookieJar(object : CookieJar {
            override fun saveFromResponse(url: HttpUrl, cookies: List<Cookie>) {
                cookieStore.getOrPut(url.host) { mutableListOf() }.apply {
                    cookies.forEach { c ->
                        removeAll { it.name == c.name }
                        add(c)
                    }
                }
            }

            override fun loadForRequest(url: HttpUrl): List<Cookie> =
                cookieStore[url.host].orEmpty()
        })
        .followRedirects(true)
        .followSslRedirects(true)
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .writeTimeout(120, TimeUnit.SECONDS)
        .build()

    private val mp3UrlPattern = Pattern.compile(
        """https://[a-z0-9.-]*drivemusic\.me/dl/[^"\s<>]+\.mp3""",
        Pattern.CASE_INSENSITIVE,
    )

    private val searchItemPattern = Pattern.compile(
        """(?s)href="(/[a-z0-9_]+/(\d+)-[^"]+\.html)"[^>]*class="popular-play-author"[^>]*>([^<]*)</a>.*?popular-play-composition.*?>(?:<a[^>]*>)?([^<]*)""",
        Pattern.CASE_INSENSITIVE,
    )

    fun search(query: String): List<Track> {
        val encoded = URLEncoder.encode(query.trim(), Charsets.UTF_8.name())
        val url = "$BASE/?do=search&subaction=search&story=$encoded"
        val body = get(url, BASE) ?: return emptyList()

        val results = mutableListOf<Track>()
        val seen = mutableSetOf<String>()
        val m = searchItemPattern.matcher(body)
        while (m.find()) {
            val id = m.group(2)?.trim().orEmpty()
            if (id.isEmpty() || !seen.add(id)) continue
            val title = decode(m.group(3)?.trim().orEmpty())
            val artist = decode(m.group(4)?.trim().orEmpty())
            if (title.isEmpty()) continue
            val path = m.group(1)?.trim().orEmpty()
            val pageUrl = if (path.startsWith("http")) path else "$BASE$path"
            results.add(
                Track(
                    id = id,
                    artist = artist,
                    title = title,
                    streamUrl = pageUrl,
                    source = DownloadSource.DriveMusic,
                ),
            )
        }
        return results.take(30)
    }

    fun download(track: Track, destFile: File): String {
        val pageUrl = pageUrl(track)
        val pageBody = get(pageUrl, BASE) ?: ""
        val candidates = downloadCandidates(pageBody, track)

        var lastErr = "нет доступных URL"
        for (url in candidates) {
            try {
                val request = Request.Builder()
                    .url(url)
                    .header("User-Agent", USER_AGENT)
                    .header("Referer", pageUrl)
                    .header("Origin", BASE)
                    .header("Accept", "audio/mpeg,application/octet-stream,*/*;q=0.8")
                    .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
                    .build()

                client.newCall(request).execute().use { resp ->
                    if (!resp.isSuccessful) {
                        lastErr = "HTTP ${resp.code}"
                        return@use
                    }
                    val body = resp.body ?: run {
                        lastErr = "пустой ответ"
                        return@use
                    }
                    destFile.parentFile?.mkdirs()
                    var written = 0L
                    destFile.outputStream().use { out ->
                        body.byteStream().use { input ->
                            val buf = ByteArray(8192)
                            while (true) {
                                val n = input.read(buf)
                                if (n <= 0) break
                                out.write(buf, 0, n)
                                written += n
                            }
                        }
                    }
                    if (written < MIN_DOWNLOAD_BYTES) {
                        destFile.delete()
                        lastErr = "файл слишком маленький ($written B), ссылка могла устареть"
                        return@use
                    }

                    // Переименовать файл по Content-Disposition
                    val cdHeader = resp.header("Content-Disposition") ?: ""
                    val cdName = extractFileNameFromDisposition(cdHeader)
                    if (cdName != null && cdName.endsWith(".mp3", ignoreCase = true)) {
                        val cleaned = MusicLibrary.cleanDispositionFilename(cdName)
                        val renamed = File(destFile.parentFile, cleaned)
                        if (renamed.absolutePath != destFile.absolutePath) {
                            destFile.renameTo(renamed)
                            return renamed.absolutePath
                        }
                    }

                    return destFile.absolutePath
                }
            } catch (e: Exception) {
                lastErr = e.message ?: e.toString()
                destFile.delete()
            }
        }
        throw IllegalStateException("DriveMusic: $lastErr")
    }

    /** Прямой URL для стрима (online), если есть на странице. */
    fun resolveStreamUrl(track: Track): String? {
        val pageUrl = pageUrl(track)
        val body = get(pageUrl, BASE) ?: return null
        val urls = extractMp3Urls(body)
        return urls.firstOrNull { it.contains("/dl/online/") }
            ?: urls.firstOrNull()
    }

    private fun pageUrl(track: Track): String {
        val u = track.streamUrl.trim()
        when {
            u.contains("drivemusic.me") && u.endsWith(".html") -> return normalizeUrl(u)
            u.startsWith("/") && u.endsWith(".html") -> return "$BASE$u"
        }
        throw IllegalStateException(
            "DriveMusic: нет ссылки на страницу трека — найдите трек через поиск.",
        )
    }

    private fun downloadCandidates(pageBody: String, track: Track): List<String> {
        val urls = extractMp3Urls(pageBody).toMutableList()
        val sorted = urls.distinct().sortedWith(
            compareBy(
                { if (it.contains("/dl/online/")) 1 else 0 },
                { it.length },
            ),
        )
        return sorted
    }

    private fun extractMp3Urls(html: String): List<String> {
        val out = mutableListOf<String>()
        val m = mp3UrlPattern.matcher(html)
        while (m.find()) {
            out.add(m.group())
        }
        return out
    }

    private fun normalizeUrl(url: String): String {
        var u = decode(url.trim())
        if (u.startsWith("//")) u = "https:$u"
        if (u.startsWith("/")) u = "$BASE$u"
        return u
    }

    private fun extractFileNameFromDisposition(header: String): String? {
        val utf8 = Regex("filename\\*=UTF-8''([^;\\s]+)", RegexOption.IGNORE_CASE)
            .find(header)?.groupValues?.get(1)
        if (utf8 != null) {
            return try { URLDecoder.decode(utf8, "UTF-8") } catch (_: Exception) { null }
        }
        val plain = Regex("""filename="?([^";\s]+)"?""", RegexOption.IGNORE_CASE)
            .find(header)?.groupValues?.get(1)
        if (plain != null) {
            return try { URLDecoder.decode(plain, "UTF-8") } catch (_: Exception) { null }
        }
        return null
    }

    private fun get(url: String, referer: String?): String? {
        val builder = Request.Builder()
            .url(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
        if (referer != null) builder.header("Referer", referer)
        client.newCall(builder.build()).execute().use { resp ->
            if (!resp.isSuccessful) return null
            return resp.body?.string()
        }
    }

    private fun decode(s: String): String =
        s.replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
}
