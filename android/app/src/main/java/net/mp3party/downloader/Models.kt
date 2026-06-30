package net.mp3party.downloader

import java.io.File

enum class DownloadSource {
    MP3Party,
    DriveMusic,
    YouTube,
    PesniMe,
}

enum class YtFormat {
    MP3,
    MP4,
}

data class Track(
    val id: String,
    val artist: String,
    val title: String,
    val streamUrl: String,
    val source: DownloadSource = DownloadSource.MP3Party,
) {
    val youtubeWatchUrl: String
        get() = if (streamUrl.startsWith("http")) streamUrl
        else "https://www.youtube.com/watch?v=$id"
}

data class LocalMediaFile(
    val file: File,
    val displayName: String,
    val isVideo: Boolean,
    val sizeBytes: Long,
)
