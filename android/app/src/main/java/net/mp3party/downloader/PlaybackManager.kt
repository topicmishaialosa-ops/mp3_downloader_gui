package net.mp3party.downloader

import android.content.Context
import android.net.Uri
import androidx.core.content.FileProvider
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import java.io.File

object PlaybackManager {
    private const val USER_AGENT =
        "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36"

    private var player: ExoPlayer? = null
    private var lastError: String? = null

    var currentFile: File? = null
        private set
    var currentStreamTrackId: String? = null
        private set
    var isStream: Boolean = false
        private set
    var currentTitle: String = ""
        private set
    var isVideo: Boolean = false
        private set

    var listener: ((PlayerState) -> Unit)? = null
    var libraryListener: ((PlayerState) -> Unit)? = null
    var errorListener: ((String) -> Unit)? = null

    data class PlayerState(
        val title: String,
        val isPlaying: Boolean,
        val isVideo: Boolean,
        val file: File?,
        val streamTrackId: String? = null,
        val isStream: Boolean = false,
        val hasActiveMedia: Boolean = false,
    )

    fun getPlayer(context: Context): ExoPlayer {
        val existing = player
        if (existing != null) return existing

        val app = context.applicationContext
        val httpFactory = DefaultHttpDataSource.Factory()
            .setUserAgent(USER_AGENT)
            .setAllowCrossProtocolRedirects(true)
            .setConnectTimeoutMs(30_000)
            .setReadTimeoutMs(60_000)
            .setDefaultRequestProperties(
                mapOf(
                    "Referer" to "https://www.youtube.com/",
                    "Origin" to "https://www.youtube.com",
                ),
            )
        val dataSourceFactory = DefaultDataSource.Factory(app, httpFactory)

        return ExoPlayer.Builder(app)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSourceFactory))
            .build()
            .also { p ->
                player = p
                p.addListener(object : Player.Listener {
                    override fun onIsPlayingChanged(isPlaying: Boolean) {
                        emit()
                    }

                    override fun onPlaybackStateChanged(playbackState: Int) {
                        if (playbackState == Player.STATE_READY) {
                            lastError = null
                        }
                        emit()
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        lastError = error.localizedMessage ?: "Ошибка воспроизведения"
                        errorListener?.invoke(lastError!!)
                        emit()
                    }
                })
            }
    }

    fun play(context: Context, file: File, title: String, video: Boolean) {
        if (!file.exists()) {
            errorListener?.invoke("Файл не найден: ${file.name}")
            return
        }
        val sameFile = currentFile?.absolutePath == file.absolutePath && !isStream
        currentFile = file
        currentStreamTrackId = null
        isStream = false
        currentTitle = title
        isVideo = video
        val p = getPlayer(context)
        if (!sameFile) {
            p.stop()
            p.clearMediaItems()
            p.setMediaItem(MediaItem.fromUri(uriForFile(context, file)))
            p.prepare()
        }
        p.play()
        emit()
    }

    fun playStream(context: Context, url: String, title: String, trackId: String, video: Boolean) {
        val sameStream = isStream && currentStreamTrackId == trackId
        currentFile = null
        currentStreamTrackId = trackId
        isStream = true
        currentTitle = title
        isVideo = video
        val p = getPlayer(context)
        if (!sameStream) {
            p.stop()
            p.clearMediaItems()
            p.setMediaItem(MediaItem.fromUri(Uri.parse(url)))
            p.prepare()
        }
        p.play()
        emit()
    }

    fun togglePlayPause(context: Context) {
        val p = player ?: return
        if (p.isPlaying) p.pause() else p.play()
        emit()
    }

    fun isCurrentFile(file: File): Boolean =
        !isStream && currentFile?.absolutePath == file.absolutePath

    fun isPlayingFile(file: File): Boolean =
        isCurrentFile(file) && player?.isPlaying == true

    fun isCurrentStream(trackId: String): Boolean =
        isStream && currentStreamTrackId == trackId

    fun isPlayingStream(trackId: String): Boolean =
        isCurrentStream(trackId) && player?.isPlaying == true

    fun hasActiveMedia(): Boolean = currentFile != null || isStream

    fun stop() {
        player?.stop()
        player?.clearMediaItems()
        currentFile = null
        currentStreamTrackId = null
        isStream = false
        currentTitle = ""
        lastError = null
        emit()
    }

    fun isPlaying(): Boolean = player?.isPlaying == true

    fun getPositionMs(): Long = player?.currentPosition ?: 0L

    fun getDurationMs(): Long {
        val d = player?.duration ?: return 0L
        return if (d > 0 && d != androidx.media3.common.C.TIME_UNSET) d else 0L
    }

    fun seekTo(positionMs: Long) {
        player?.seekTo(positionMs.coerceAtLeast(0L))
        emit()
    }

    private fun uriForFile(context: Context, file: File): Uri {
        return try {
            FileProvider.getUriForFile(
                context.applicationContext,
                "${context.applicationContext.packageName}.fileprovider",
                file,
            )
        } catch (_: Exception) {
            Uri.fromFile(file)
        }
    }

    private fun emit() {
        val state = PlayerState(
            title = currentTitle,
            isPlaying = player?.isPlaying == true,
            isVideo = isVideo,
            file = currentFile,
            streamTrackId = currentStreamTrackId,
            isStream = isStream,
            hasActiveMedia = hasActiveMedia(),
        )
        listener?.invoke(state)
        libraryListener?.invoke(state)
    }

    fun release() {
        player?.release()
        player = null
        currentFile = null
        currentStreamTrackId = null
        isStream = false
    }
}
