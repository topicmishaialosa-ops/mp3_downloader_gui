package net.mp3party.downloader

import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.net.URLEncoder
import java.util.concurrent.TimeUnit
import java.util.regex.Pattern

object PesniMeApi {
    private const val BASE = "https://music.pesni.me"
    private const val USER_AGENT =
        "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36"
    private const val MIN_DOWNLOAD_BYTES = 50 * 1024

    private val client = OkHttpClient.Builder()
        .followRedirects(true)
        .followSslRedirects(true)
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .writeTimeout(120, TimeUnit.SECONDS)
        .build()

    private val trackPattern = Pattern.compile(
        """\\"id\\":(\d+),\\"artist\\":\\"([^"\\]*)\\",\\"title\\":\\"([^"\\]*)\\",\\"version\\":\\"[^"\\]*\\",\\"duration\\":(\d+),\\"bitrate\\":([^,]*),\\"size\\":([^,]*),\\"play\\":\\"([^"\\]+)\\",\\"download\\":\\"([^"\\]+)\\"""",
    )

    fun search(query: String): List<Track> {
        val encoded = URLEncoder.encode(query.trim(), Charsets.UTF_8.name())
        val url = "$BASE/search/$encoded?type=tracks"
        val body = get(url) ?: run {
            // fallback to pesni.me (without music. subdomain)
            val fallbackUrl = "https://pesni.me/search/$encoded"
            get(fallbackUrl) ?: return emptyList()
        }

        val allResults = extractFromPage(body)
        val q = query.trim().lowercase()
        val filtered = allResults.filter { t ->
            t.artist.lowercase().startsWith(q) || t.title.lowercase().startsWith(q)
        }
        if (filtered.isNotEmpty()) return filtered.take(30)

        val results = allResults
        if (results.isNotEmpty()) return results.take(30)

        return emptyList()
    }

    fun download(track: Track, destFile: File): String {
        val downloadUrl = if (track.streamUrl.contains("dw.pesni.me")) {
            track.streamUrl
        } else {
            val pageBody = get(trackUrl(track.id)) ?: ""
            val tracks = extractFromPage(pageBody)
            tracks.firstOrNull()?.streamUrl
                ?: throw IllegalStateException("Pesni.me: не найден download URL")
        }

        val request = Request.Builder()
            .url(downloadUrl)
            .header("User-Agent", USER_AGENT)
            .header("Referer", trackUrl(track.id))
            .header("Accept", "audio/mpeg,application/octet-stream,*/*;q=0.8")
            .build()

        client.newCall(request).execute().use { resp ->
            if (!resp.isSuccessful) {
                throw IllegalStateException("Pesni.me: HTTP ${resp.code}")
            }
            val body = resp.body ?: throw IllegalStateException("Pesni.me: пустой ответ")
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
                throw IllegalStateException("Pesni.me: файл слишком маленький ($written B)")
            }
            return destFile.absolutePath
        }
    }

    fun resolveStreamUrl(track: Track): String? {
        if (track.streamUrl.contains("pl.pesni.me")) {
            return track.streamUrl
        }
        val body = get(trackUrl(track.id)) ?: return null
        val playRe = Pattern.compile(""""play":"([^"]+)"""")
        val m = playRe.matcher(body)
        return if (m.find()) m.group(1) else null
    }

    private fun trackUrl(id: String) = "$BASE/track/$id"

    private fun extractFromPage(body: String): List<Track> {
        val results = mutableListOf<Track>()
        val seen = mutableSetOf<String>()
        val m = trackPattern.matcher(body)
        while (m.find()) {
            val id = m.group(1)?.trim().orEmpty()
            if (id.isEmpty() || !seen.add(id)) continue
            val artist = unescape(m.group(2)?.trim().orEmpty())
            val title = unescape(m.group(3)?.trim().orEmpty())
            val playUrl = m.group(4)?.trim().orEmpty()
            val downloadUrl = m.group(5)?.trim().orEmpty()
            val url = if (downloadUrl.isNotEmpty()) downloadUrl else playUrl
            if (title.isEmpty()) continue
            results.add(Track(id, artist, title, url, DownloadSource.PesniMe))
        }
        return results
    }

    private fun get(url: String): String? {
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .build()
        client.newCall(request).execute().use { resp ->
            if (!resp.isSuccessful) return null
            return resp.body?.string()
        }
    }

    private fun unescape(s: String): String =
        s.replace("\\\"", "\"")
            .replace("\\n", "\n")
            .replace("\\t", "\t")
}
