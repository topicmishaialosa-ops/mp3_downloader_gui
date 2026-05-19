package net.mp3party.downloader

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.core.view.isVisible
import androidx.recyclerview.widget.RecyclerView
import net.mp3party.downloader.databinding.ItemTrackBinding

class TrackAdapter(
    private val onDownload: (Track, Int) -> Unit,
    private val onStream: (Track, Int) -> Unit,
) : RecyclerView.Adapter<TrackAdapter.Holder>() {

    private val items = mutableListOf<Track>()
    private var downloadingPosition: Int = RecyclerView.NO_POSITION
    private var streamingPosition: Int = RecyclerView.NO_POSITION

    fun submit(list: List<Track>) {
        items.clear()
        items.addAll(list)
        downloadingPosition = RecyclerView.NO_POSITION
        streamingPosition = RecyclerView.NO_POSITION
        notifyDataSetChanged()
    }

    fun setDownloadingPosition(position: Int) {
        val old = downloadingPosition
        downloadingPosition = position
        if (old != RecyclerView.NO_POSITION) notifyItemChanged(old)
        if (position != RecyclerView.NO_POSITION) notifyItemChanged(position)
    }

    fun setStreamingPosition(position: Int) {
        val old = streamingPosition
        streamingPosition = position
        if (old != RecyclerView.NO_POSITION) notifyItemChanged(old)
        if (position != RecyclerView.NO_POSITION) notifyItemChanged(position)
    }

    fun clearDownloading() {
        setDownloadingPosition(RecyclerView.NO_POSITION)
    }

    fun clearStreaming() {
        setStreamingPosition(RecyclerView.NO_POSITION)
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): Holder {
        val binding = ItemTrackBinding.inflate(
            LayoutInflater.from(parent.context),
            parent,
            false,
        )
        return Holder(binding)
    }

    override fun onBindViewHolder(holder: Holder, position: Int) {
        holder.bind(
            items[position],
            isDownloading = position == downloadingPosition,
            isStreaming = position == streamingPosition,
        )
    }

    override fun getItemCount(): Int = items.size

    inner class Holder(
        private val binding: ItemTrackBinding,
    ) : RecyclerView.ViewHolder(binding.root) {

        fun bind(track: Track, isDownloading: Boolean, isStreaming: Boolean) {
            binding.trackTitle.text = track.title
            binding.trackArtist.text = track.artist.ifEmpty { "—" }
            val src = when (track.source) {
                DownloadSource.MP3Party -> "MP3Party"
                DownloadSource.DriveMusic -> "DriveMusic"
                DownloadSource.YouTube -> "YouTube"
            }
            binding.trackId.text = "[$src] ID ${track.id}"

            val initial = track.artist.firstOrNull()
                ?: track.title.firstOrNull()
                ?: '?'
            binding.trackInitial.text = initial.uppercaseChar().toString()

            val isYoutube = track.source == DownloadSource.YouTube
            binding.streamButton.isVisible = isYoutube

            val busy = isDownloading || isStreaming
            binding.itemProgress.isVisible = busy
            binding.downloadButton.isEnabled = !busy
            binding.streamButton.isEnabled = !busy
            binding.downloadButton.alpha = if (isDownloading) 0.5f else 1f
            binding.streamButton.alpha = if (isStreaming) 0.5f else 1f

            val streamingThis = PlaybackManager.isCurrentStream(track.id)
            binding.streamButton.setIconResource(
                if (streamingThis && PlaybackManager.isPlaying()) {
                    R.drawable.ic_pause
                } else {
                    R.drawable.ic_stream
                },
            )

            binding.downloadButton.setOnClickListener {
                val pos = bindingAdapterPosition
                if (pos != RecyclerView.NO_POSITION) {
                    onDownload(track, pos)
                }
            }

            binding.streamButton.setOnClickListener {
                val pos = bindingAdapterPosition
                if (pos == RecyclerView.NO_POSITION) return@setOnClickListener
                if (PlaybackManager.isCurrentStream(track.id)) {
                    PlaybackManager.togglePlayPause(binding.root.context)
                } else {
                    onStream(track, pos)
                }
            }
        }
    }
}
