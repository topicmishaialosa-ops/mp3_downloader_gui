package net.mp3party.downloader

import android.content.Context
import com.yausername.ffmpeg.FFmpeg
import com.yausername.youtubedl_android.YoutubeDL
import com.yausername.youtubedl_android.YoutubeDLException
import com.yausername.youtubedl_android.YoutubeDLRequest
import com.yausername.youtubedl_android.YoutubeDLResponse
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

object YtDlpHelper {
    private val initMutex = Mutex()

    @Volatile
    var isReady: Boolean = false
        private set

    @Volatile
    var initError: String? = null
        private set

    suspend fun init(context: Context) = initMutex.withLock {
        if (isReady) return@withLock
        withContext(Dispatchers.IO) {
            val app = context.applicationContext
            try {
                YoutubeDL.getInstance().init(app)
                try {
                    YoutubeDL.getInstance().updateYoutubeDL(app)
                } catch (_: Exception) {
                    // обновление опционально
                }
                FFmpeg.getInstance().init(app)
                isReady = true
                initError = null
            } catch (e: Exception) {
                val msg = when (e) {
                    is YoutubeDLException -> e.message
                    else -> e.message
                } ?: e.javaClass.simpleName
                initError = "Ошибка инициализации: $msg"
                isReady = false
                throw IllegalStateException(initError)
            }
        }
    }

    private suspend fun ensureReady(context: Context) {
        if (!isReady) init(context)
    }

    private fun appendNetworkOptions(request: YoutubeDLRequest) {
        request.addOption("--no-warnings")
        request.addOption("--ignore-errors")
        request.addOption("--socket-timeout", "25")
        request.addOption("--retries", "3")
        request.addOption("--extractor-retries", "3")
        request.addOption("--geo-bypass")
    }

    suspend fun search(context: Context, query: String): List<Track> = withContext(Dispatchers.IO) {
        ensureReady(context)
        val q = query.trim()
        if (q.isEmpty()) return@withContext emptyList()

        // JSON надёжнее на Android, чем --print
        val jsonResults = searchJson(q)
        if (jsonResults.isNotEmpty()) return@withContext jsonResults

        val textResults = searchText(q)
        if (textResults.isNotEmpty()) return@withContext textResults

        throw IllegalStateException(
            initError ?: "Ничего не найдено на YouTube. Проверьте интернет и повторите.",
        )
    }

    private fun searchJson(query: String): List<Track> {
        val request = YoutubeDLRequest("ytsearch20:$query")
        appendNetworkOptions(request)
        request.addOption("--flat-playlist")
        request.addOption("--playlist-end", "20")
        request.addOption("-J")

        val response = executeOrThrow(request, allowStdoutOnError = true)
        val raw = response.out?.trim().orEmpty()
        if (raw.isEmpty()) return emptyList()
        return parseSearchJson(raw)
    }

    private fun searchText(query: String): List<Track> {
        val request = YoutubeDLRequest("ytsearch20:$query")
        appendNetworkOptions(request)
        request.addOption("--flat-playlist")
        request.addOption("--playlist-end", "20")
        request.addOption("--print", "%(id)s|||%(title)s|||%(channel)s")

        val response = executeOrThrow(request, allowStdoutOnError = true)
        return if (!response.out.isNullOrBlank()) {
            parseSearchOutput(response.out)
        } else {
            emptyList()
        }
    }

    private fun executeOrThrow(
        request: YoutubeDLRequest,
        allowStdoutOnError: Boolean = false,
    ): YoutubeDLResponse {
        val response = YoutubeDL.getInstance().execute(request, null, null)
        val code = response.exitCode
        val hasOut = !response.out.isNullOrBlank()
        if (code != 0 && (!allowStdoutOnError || !hasOut)) {
            val err = response.err?.trim().orEmpty()
            val tail = err.lines().takeLast(6).joinToString("\n")
            throw IllegalStateException(
                if (tail.isNotEmpty()) "YouTube/yt-dlp:\n$tail"
                else "yt-dlp завершился с кодом $code",
            )
        }
        return response
    }

    private fun parseSearchJson(raw: String): List<Track> {
        val results = mutableListOf<Track>()
        val seen = mutableSetOf<String>()
        try {
            val root = JSONObject(raw)
            val entries: JSONArray = when {
                root.has("entries") -> root.getJSONArray("entries")
                else -> JSONArray().put(root)
            }
            for (i in 0 until entries.length()) {
                val entry = entries.optJSONObject(i) ?: continue
                val id = entry.optString("id").trim()
                if (id.isEmpty() || id == "NA" || id.length < 6 || !seen.add(id)) continue
                val fullTitle = entry.optString("title").trim()
                if (fullTitle.isEmpty() || fullTitle == "NA") continue
                val channel = entry.optString("channel", entry.optString("uploader", "")).trim()
                val (artist, title) = splitTitle(fullTitle, channel)
                results.add(
                    Track(
                        id = id,
                        artist = artist,
                        title = title,
                        streamUrl = "https://www.youtube.com/watch?v=$id",
                        source = DownloadSource.YouTube,
                    ),
                )
            }
        } catch (_: Exception) {
            return emptyList()
        }
        return results
    }

    private fun parseSearchOutput(stdout: String): List<Track> {
        val results = mutableListOf<Track>()
        val seen = mutableSetOf<String>()
        for (line in stdout.lines()) {
            val trimmed = line.trim()
            if (trimmed.isEmpty() || trimmed.startsWith("[")) continue
            val parts = trimmed.split("|||")
            if (parts.size < 2) continue
            val id = parts[0].trim()
            if (id.isEmpty() || id == "NA" || id.length < 6 || !seen.add(id)) continue
            val fullTitle = parts[1].trim()
            val channel = parts.getOrNull(2)?.trim().orEmpty()
            if (fullTitle.isEmpty() || fullTitle == "NA") continue
            val (artist, title) = splitTitle(fullTitle, channel)
            results.add(
                Track(
                    id = id,
                    artist = artist,
                    title = title,
                    streamUrl = "https://www.youtube.com/watch?v=$id",
                    source = DownloadSource.YouTube,
                ),
            )
        }
        return results
    }

    private fun splitTitle(full: String, channel: String): Pair<String, String> {
        val parts = full.split(" - ", limit = 2)
        return if (parts.size >= 2) {
            parts[0].trim() to parts[1].trim()
        } else if (channel.isNotEmpty() && channel != full) {
            channel to full
        } else {
            "YouTube" to full
        }
    }

    suspend fun getStreamUrl(
        context: Context,
        track: Track,
        format: YtFormat,
    ): String = withContext(Dispatchers.IO) {
        ensureReady(context)
        val request = YoutubeDLRequest(track.youtubeWatchUrl)
        appendNetworkOptions(request)
        request.addOption("--no-playlist")
        request.addOption("-g")
        when (format) {
            YtFormat.MP3 -> request.addOption("-f", "bestaudio[ext=m4a]/bestaudio/best")
            YtFormat.MP4 -> request.addOption("-f", "best[height<=720][ext=mp4]/best[ext=mp4]/best")
        }

        val response = executeOrThrow(request)
        val urls = response.out
            ?.lines()
            ?.map { it.trim() }
            ?.filter { it.startsWith("http://") || it.startsWith("https://") }
            .orEmpty()

        if (urls.isEmpty()) {
            throw IllegalStateException("yt-dlp не вернул URL потока")
        }
        urls.first()
    }

    suspend fun download(
        context: Context,
        track: Track,
        destDir: File,
        format: YtFormat,
        onProgress: (Float, String) -> Unit,
    ): File = withContext(Dispatchers.IO) {
        ensureReady(context)
        destDir.mkdirs()
        val template = File(destDir, "%(title)s.%(ext)s").absolutePath
        val request = YoutubeDLRequest(track.youtubeWatchUrl)
        appendNetworkOptions(request)
        request.addOption("--no-playlist")
        request.addOption("--no-mtime")
        request.addOption("--newline")
        request.addOption("-o", template)

        when (format) {
            YtFormat.MP3 -> {
                request.addOption("-f", "bestaudio/best")
                request.addOption("-x")
                request.addOption("--audio-format", "mp3")
                request.addOption("--audio-quality", "0")
            }
            YtFormat.MP4 -> {
                request.addOption("-f", "bv*[height<=720]+ba/b[height<=720]/best")
                request.addOption("--merge-output-format", "mp4")
            }
        }

        var lastPct = 0f
        val response = YoutubeDL.getInstance().execute(request, null) { progress, _, line ->
            val pct = (progress.toFloat() / 100f).coerceIn(0f, 0.99f)
            if (pct >= lastPct) {
                lastPct = pct
                onProgress(pct, line?.take(80) ?: "")
            }
        }

        if (response.exitCode != 0) {
            val err = response.err?.trim().orEmpty()
            if (err.isNotEmpty()) {
                throw IllegalStateException(err.lines().takeLast(4).joinToString("\n"))
            }
        }

        val outFile = findNewestMedia(destDir, format)
            ?: response.out?.let { parseAfterMove(it) }
            ?: throw IllegalStateException("yt-dlp не вернул файл")

        if (outFile.length() < 50 * 1024) {
            throw IllegalStateException("Файл слишком маленький")
        }
        onProgress(1f, "Готово")
        outFile
    }

    private fun parseAfterMove(line: String): File? {
        val marker = "AFTERMOVE:"
        val idx = line.indexOf(marker)
        if (idx < 0) return null
        val path = line.substring(idx + marker.length).trim()
        if (path.isEmpty()) return null
        val f = File(path)
        return if (f.exists()) f else null
    }

    private fun findNewestMedia(dir: File, format: YtFormat): File? {
        val ext = if (format == YtFormat.MP3) listOf("mp3", "m4a", "opus") else listOf("mp4", "mkv", "webm")
        return dir.listFiles()
            ?.filter { f -> f.isFile && ext.any { e -> f.name.endsWith(".$e", true) } }
            ?.maxByOrNull { it.lastModified() }
    }
}
