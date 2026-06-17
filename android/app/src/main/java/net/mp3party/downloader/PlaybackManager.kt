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

enum class LoopMode {
    NoRepeat,
    RepeatAll,
    RepeatOne,
}

data class PlaylistItem(
    val pathOrUrl: String,
    val title: String,
    val subtitle: String = "",
    val isVideo: Boolean = false,
    val isUrl: Boolean = false,
)

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

    var loopMode: LoopMode = LoopMode.NoRepeat
        private set
    val playlist: MutableList<PlaylistItem> = mutableListOf()
    var playlistIndex: Int = 0
        private set

    data class PlayerState(
        val title: String,
        val isPlaying: Boolean,
        val isVideo: Boolean,
        val file: File?,
        val streamTrackId: String? = null,
        val isStream: Boolean = false,
        val hasActiveMedia: Boolean = false,
        val loopMode: LoopMode = LoopMode.NoRepeat,
        val playlistSize: Int = 0,
        val playlistIndex: Int = 0,
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
                        if (playbackState == Player.STATE_ENDED) {
                            onTrackEnded()
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

    private fun onTrackEnded() {
        when (loopMode) {
            LoopMode.NoRepeat -> {
                if (playlistIndex + 1 < playlist.size) {
                    playlistIndex++
                    playCurrent()
                } else {
                    stop()
                }
            }
            LoopMode.RepeatAll -> {
                playlistIndex = (playlistIndex + 1) % playlist.size
                playCurrent()
            }
            LoopMode.RepeatOne -> {
                playCurrent()
            }
        }
    }

    fun playCurrent() {
        if (playlistIndex < 0 || playlistIndex >= playlist.size) {
            stop()
            return
        }
        val item = playlist[playlistIndex]
        if (item.isUrl) {
            playStream(item)
        } else {
            playFile(item)
        }
    }

    fun playNext() {
        if (playlist.isEmpty()) return
        when (loopMode) {
            LoopMode.NoRepeat -> {
                if (playlistIndex + 1 >= playlist.size) {
                    stop()
                    return
                }
                playlistIndex++
            }
            LoopMode.RepeatAll -> {
                playlistIndex = (playlistIndex + 1) % playlist.size
            }
            LoopMode.RepeatOne -> {
                if (playlistIndex >= playlist.size) playlistIndex = 0
            }
        }
        playCurrent()
    }

    fun playPrev() {
        if (playlist.isEmpty()) return
        if (loopMode == LoopMode.RepeatAll) {
            playlistIndex = if (playlistIndex == 0) playlist.size - 1 else playlistIndex - 1
        } else {
            if (playlistIndex > 0) playlistIndex--
        }
        playCurrent()
    }

    fun addToPlaylist(item: PlaylistItem) {
        playlist.add(item)
        emit()
    }

    fun clearPlaylist() {
        playlist.clear()
        emit()
    }

    fun setLoopMode(mode: LoopMode) {
        loopMode = mode
        emit()
    }

    fun advanceLoopMode() {
        loopMode = when (loopMode) {
            LoopMode.NoRepeat -> LoopMode.RepeatAll
            LoopMode.RepeatAll -> LoopMode.RepeatOne
            LoopMode.RepeatOne -> LoopMode.NoRepeat
        }
        emit()
    }

    private fun playFile(item: PlaylistItem) {
        val file = File(item.pathOrUrl)
        if (!file.exists()) {
            errorListener?.invoke("Файл не найден: ${file.name}")
            return
        }
        currentFile = file
        currentStreamTrackId = null
        isStream = false
        currentTitle = item.title
        isVideo = item.isVideo
        val p = getPlayer(/* context needed but we use app-level */ file)
        p.stop()
        p.clearMediaItems()
        p.setMediaItem(MediaItem.fromUri(uriForFile(p.context, file)))
        p.prepare()
        p.play()
        emit()
    }

    private fun playStream(item: PlaylistItem) {
        currentFile = null
        currentStreamTrackId = item.pathOrUrl
        isStream = true
        currentTitle = item.title
        isVideo = item.isVideo
        val p = getPlayer(/* context needed */ item.pathOrUrl)
        p.stop()
        p.clearMediaItems()
        p.setMediaItem(MediaItem.fromUri(Uri.parse(item.pathOrUrl)))
        p.prepare()
        p.play()
        emit()
    }

    fun play(context: Context, file: File, title: String, video: Boolean) {
        if (!file.exists()) {
            errorListener?.invoke("Файл не найден: ${file.name}")
            return
        }
        playlist.clear()
        playlistIndex = 0
        playlist.add(
            PlaylistItem(
                pathOrUrl = file.absolutePath,
                title = title,
                subtitle = if (video) "Видео" else "Локальный файл",
                isVideo = video,
                isUrl = false,
            )
        )
        playCurrent()
    }

    fun playStream(context: Context, url: String, title: String, trackId: String, video: Boolean) {
        playlist.clear()
        playlistIndex = 0
        playlist.add(
            PlaylistItem(
                pathOrUrl = url,
                title = title,
                subtitle = "Стрим",
                isVideo = video,
                isUrl = true,
            )
        )
        playCurrent()
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
        playlist.clear()
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
            loopMode = loopMode,
            playlistSize = playlist.size,
            playlistIndex = playlistIndex,
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
