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

object Mp3PartyApi {
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

    private val panelPattern = Pattern.compile(
        """data-js-id="(\d+)"[^>]*data-js-artist-name="([^"]*)"[^>]*data-js-song-title="([^"]*)"[^>]*data-js-url="([^"]+)"""",
        Pattern.DOTALL,
    )
    private val panelPatternAlt = Pattern.compile(
        """data-js-id="(\d+)".*?data-js-artist-name="([^"]*)".*?data-js-song-title="([^"]*)".*?data-js-url="([^"]+)"""",
        Pattern.DOTALL,
    )

    fun fetchTrack(id: String): Track? {
        val url = "https://mp3party.net/music/$id"
        val body = get(url, null) ?: return null

        var artist = ""
        var title = ""

        val panelM = panelPattern.matcher(body)
        if (panelM.find()) {
            val a = panelM.group(2)
            val t = panelM.group(3)
            if (a != null) artist = decode(a)
            if (t != null) title = decode(t)
        }

        if (title.isEmpty()) {
            val ogM = java.util.regex.Pattern.compile(
                """property="og:title"\s+content="([^"]+)""""
            ).matcher(body)
            if (ogM.find()) {
                val content = ogM.group(1) ?: ""
                val parts = content.split(" - ", limit = 2)
                if (parts.size == 2) {
                    artist = parts[0].trim()
                    title = parts[1].trim()
                } else {
                    title = content.trim()
                }
            }
        }

        if (title.isEmpty()) {
            title = "Трек #$id"
        }

        return Track(id, artist, title, streamUrl(id))
    }

    fun search(query: String): List<Track> {
        val encoded = URLEncoder.encode(query.trim(), Charsets.UTF_8.name())
        val url = "https://mp3party.net/search?q=$encoded"
        val body = get(url, null) ?: return emptyList()

        val results = mutableListOf<Track>()
        val seen = mutableSetOf<String>()

        fun addMatch(m: java.util.regex.Matcher) {
            val id = m.group(1) ?: return
            if (!seen.add(id)) return
            val artist = decode(m.group(2) ?: "")
            val title = decode(m.group(3) ?: "")
            val stream = normalizeUrl(m.group(4) ?: streamUrl(id))
            if (title.isNotEmpty()) {
                results.add(Track(id, artist, title, stream))
            }
        }

        panelPattern.matcher(body).let { m ->
            while (m.find()) addMatch(m)
        }
        if (results.isEmpty()) {
            panelPatternAlt.matcher(body).let { m ->
                while (m.find()) addMatch(m)
            }
        }

        return results.take(30)
    }

    fun download(track: Track, destFile: File): String {
        val page = "https://mp3party.net/music/${track.id}"
        val pageBody = get(page, "https://mp3party.net/") ?: ""
        val candidates = downloadCandidates(pageBody, track)

        var lastErr = "нет доступных URL"
        var serverUnavailable = false

        for (url in candidates) {
            try {
                val request = Request.Builder()
                    .url(url)
                    .header("User-Agent", USER_AGENT)
                    .header("Referer", page)
                    .header("Origin", "https://mp3party.net")
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
                        val preview = if (written > 0) destFile.readBytes() else byteArrayOf()
                        destFile.delete()
                        serverUnavailable = isMp3PartyErrorBody(preview)
                        lastErr = if (serverUnavailable) {
                            "трек недоступен на CDN MP3Party"
                        } else {
                            "файл слишком маленький (${written} B)"
                        }
                        return@use
                    }

                    return destFile.absolutePath
                }
            } catch (e: Exception) {
                lastErr = e.message ?: e.toString()
                destFile.delete()
            }
        }

        val hint = if (serverUnavailable) {
            "Трек ID ${track.id} недоступен на CDN — попробуйте другой результат или YouTube"
        } else {
            "MP3Party: $lastErr"
        }
        throw IllegalStateException(hint)
    }

    /** URL для скачивания — как на десктопе: кнопка на странице, panel, download, stream. */
    private fun downloadCandidates(pageBody: String, track: Track): List<String> {
        val out = LinkedHashSet<String>()
        val id = Pattern.quote(track.id)

        fun add(raw: String?) {
            val url = normalizeUrl(raw ?: return)
            if (url.startsWith("http")) out.add(url)
        }

        // <a class="js-dw-btn" data-track-id="ID" href="...">
        Pattern.compile("""data-track-id="$id"[^>]*href="([^"]+)"""", Pattern.DOTALL)
            .matcher(pageBody).let { m ->
                while (m.find()) add(m.group(1))
            }
        Pattern.compile("""href="([^"]+)"[^>]*data-track-id="$id"""", Pattern.DOTALL)
            .matcher(pageBody).let { m ->
                while (m.find()) add(m.group(1))
            }

        // div.track__user-panel data-js-url
        Pattern.compile("""data-js-id="$id"[^>]*data-js-url="([^"]+)"""", Pattern.DOTALL)
            .matcher(pageBody).let { m ->
                while (m.find()) add(m.group(1))
            }
        Pattern.compile("""data-js-url="([^"]+)"[^>]*data-js-id="$id"""", Pattern.DOTALL)
            .matcher(pageBody).let { m ->
                while (m.find()) add(m.group(1))
            }

        add(downloadUrl(track.id))
        add(track.streamUrl)
        add(streamUrl(track.id))

        return out.toList()
    }

    private fun streamUrl(id: String) = "https://dl2.mp3party.net/online/$id.mp3"

    private fun downloadUrl(id: String) = "https://dl2.mp3party.net/download/$id"

    private fun normalizeUrl(url: String): String {
        var u = decode(url.trim())
        if (u.startsWith("//")) u = "https:$u"
        if (u.startsWith("/")) u = "https://mp3party.net$u"
        return u
    }

    private fun isMp3PartyErrorBody(bytes: ByteArray): Boolean {
        if (bytes.size > 512) return false
        val preview = String(bytes, Charsets.UTF_8)
        return preview.contains("failed to get file", ignoreCase = true) ||
            preview.contains("file not found", ignoreCase = true) ||
            preview.contains("nil")
    }

    private fun get(url: String, referer: String?): String? {
        val builder = Request.Builder()
            .url(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
        if (referer != null) {
            builder.header("Referer", referer)
        }
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
