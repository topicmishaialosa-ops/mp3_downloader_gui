package net.mp3party.downloader

import android.animation.ObjectAnimator
import android.animation.PropertyValuesHolder
import android.view.LayoutInflater
import android.view.ViewGroup
import android.view.animation.AccelerateDecelerateInterpolator
import androidx.core.view.isVisible
import androidx.recyclerview.widget.RecyclerView
import net.mp3party.downloader.databinding.ItemLibraryFileBinding
import java.io.File

class LibraryAdapter(
    private val onPlay: (LocalMediaFile) -> Unit,
    private val onToggle: (LocalMediaFile) -> Unit,
) : RecyclerView.Adapter<LibraryAdapter.Holder>() {

    private val items = mutableListOf<LocalMediaFile>()
    private var playingPath: String? = null
    private var isPlaying = false

    fun submit(list: List<LocalMediaFile>) {
        items.clear()
        items.addAll(list)
        notifyDataSetChanged()
    }

    fun setPlaybackState(file: File?, playing: Boolean) {
        val path = file?.absolutePath
        if (path == playingPath && playing == isPlaying) return
        val oldPath = playingPath
        playingPath = path
        isPlaying = playing
        items.forEachIndexed { index, item ->
            val p = item.file.absolutePath
            if (p == oldPath || p == path) notifyItemChanged(index)
        }
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): Holder {
        val binding = ItemLibraryFileBinding.inflate(
            LayoutInflater.from(parent.context),
            parent,
            false,
        )
        return Holder(binding)
    }

    override fun onBindViewHolder(holder: Holder, position: Int) {
        holder.bind(items[position])
    }

    override fun onViewRecycled(holder: Holder) {
        holder.stopPulse()
        super.onViewRecycled(holder)
    }

    override fun getItemCount(): Int = items.size

    inner class Holder(
        private val binding: ItemLibraryFileBinding,
    ) : RecyclerView.ViewHolder(binding.root) {

        private var pulseAnimator: ObjectAnimator? = null

        fun bind(item: LocalMediaFile) {
            binding.fileName.text = item.displayName
            val type = if (item.isVideo) "MP4/видео" else "MP3/аудио"
            val mb = item.sizeBytes / (1024 * 1024)
            binding.fileMeta.text = "$type · ${mb} MB"
            binding.fileIcon.text = if (item.isVideo) "🎬" else "🎵"

            val isCurrent = item.file.absolutePath == playingPath
            val playing = isCurrent && isPlaying
            val wasPlaying = binding.playFileButton.tag as? Boolean

            binding.nowPlayingIndicator.isVisible = playing
            val animate = wasPlaying != null && isCurrent && wasPlaying != playing
            binding.playFileButton.tag = if (isCurrent) playing else null
            updatePlayIcon(playing, animate = animate)

            binding.playFileButton.setOnClickListener {
                if (isCurrent) {
                    onToggle(item)
                } else {
                    animatePlayIcon(binding, toPause = true)
                    onPlay(item)
                }
            }
            binding.root.setOnClickListener {
                if (isCurrent) {
                    onToggle(item)
                } else {
                    onPlay(item)
                }
            }
        }

        private fun updatePlayIcon(playing: Boolean, animate: Boolean) {
            val icon = if (playing) R.drawable.ic_pause else R.drawable.ic_play
            if (animate) {
                binding.playFileButton.animate()
                    .scaleX(0.82f)
                    .scaleY(0.82f)
                    .setDuration(70)
                    .withEndAction {
                        binding.playFileButton.setIconResource(icon)
                        binding.playFileButton.animate()
                            .scaleX(1f)
                            .scaleY(1f)
                            .setDuration(120)
                            .setInterpolator(AccelerateDecelerateInterpolator())
                            .start()
                    }
                    .start()
            } else {
                binding.playFileButton.setIconResource(icon)
            }
            if (playing) startPulse() else stopPulse()
        }

        fun stopPulse() {
            pulseAnimator?.cancel()
            pulseAnimator = null
            binding.playFileButton.scaleX = 1f
            binding.playFileButton.scaleY = 1f
        }

        private fun startPulse() {
            if (pulseAnimator != null) return
            pulseAnimator = ObjectAnimator.ofPropertyValuesHolder(
                binding.playFileButton,
                PropertyValuesHolder.ofFloat("scaleX", 1f, 1.06f),
                PropertyValuesHolder.ofFloat("scaleY", 1f, 1.06f),
            ).apply {
                duration = 600
                repeatMode = ObjectAnimator.REVERSE
                repeatCount = ObjectAnimator.INFINITE
                interpolator = AccelerateDecelerateInterpolator()
                start()
            }
        }
    }

    private fun animatePlayIcon(binding: ItemLibraryFileBinding, toPause: Boolean) {
        binding.playFileButton.animate()
            .scaleX(0.82f)
            .scaleY(0.82f)
            .setDuration(70)
            .withEndAction {
                binding.playFileButton.setIconResource(
                    if (toPause) R.drawable.ic_pause else R.drawable.ic_play,
                )
                binding.playFileButton.animate()
                    .scaleX(1f)
                    .scaleY(1f)
                    .setDuration(120)
                    .start()
            }
            .start()
    }
}
