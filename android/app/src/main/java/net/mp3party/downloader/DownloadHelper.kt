package net.mp3party.downloader

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

object DownloadHelper {
    suspend fun download(
        context: Context,
        track: Track,
        format: YtFormat,
        onProgress: (Float, String) -> Unit = { _, _ -> },
    ): File = withContext(Dispatchers.IO) {
        val dir = MusicLibrary.musicDir(context)
        when (track.source) {
            DownloadSource.DriveMusic -> {
                onProgress(0.1f, "DriveMusic…")
                val base = when {
                    track.artist.isNotBlank() && track.title.isNotBlank() ->
                        "${track.artist} - ${track.title}"
                    track.title.isNotBlank() -> track.title
                    else -> "track_${track.id}"
                }
                val safeName = base
                    .replace(Regex("""[/\\:*?"<>|]"""), "_")
                    .take(100)
                val file = File(dir, "${safeName}_${track.id}.mp3")
                val path = DriveMusicApi.download(track, file)
                onProgress(1f, "Готово")
                File(path)
            }
            DownloadSource.MP3Party -> {
                onProgress(0.1f, "MP3Party…")
                val base = when {
                    track.artist.isNotBlank() && track.title.isNotBlank() ->
                        "${track.artist} - ${track.title}"
                    track.title.isNotBlank() -> track.title
                    else -> "track_${track.id}"
                }
                val safeName = base
                    .replace(Regex("""[/\\:*?"<>|]"""), "_")
                    .take(100)
                val file = File(dir, "${safeName}_${track.id}.mp3")
                val path = Mp3PartyApi.download(track, file)
                onProgress(1f, "Готово")
                File(path)
            }
            DownloadSource.YouTube -> {
                YtDlpHelper.download(context, track, dir, format, onProgress)
            }
        }
    }
}
