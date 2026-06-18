package net.mp3party.downloader

import android.os.Bundle
import android.widget.ImageButton
import android.widget.SeekBar
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import net.mp3party.downloader.databinding.ActivityPlayerBinding

class PlayerActivity : AppCompatActivity() {

    private lateinit var binding: ActivityPlayerBinding

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        binding = ActivityPlayerBinding.inflate(layoutInflater)
        setContentView(binding.root)

        val title = intent.getStringExtra(EXTRA_TITLE).orEmpty()
        val path = intent.getStringExtra(EXTRA_PATH)
        val isStream = intent.getBooleanExtra(EXTRA_STREAM, false)

        if (!isStream) {
            val file = path?.let { java.io.File(it) }
            if (file == null || !file.exists()) {
                finish()
                return
            }
        } else if (!PlaybackManager.hasActiveMedia()) {
            finish()
            return
        }

        binding.fullTitle.text = title.ifEmpty { PlaybackManager.currentTitle }
        binding.backButton.setOnClickListener { finish() }

        binding.loopButton.setOnClickListener {
            PlaybackManager.advanceLoopMode()
            updateLoopButton(binding.loopButton)
        }
        binding.prevButton.setOnClickListener {
            PlaybackManager.playPrev()
        }
        binding.nextButton.setOnClickListener {
            PlaybackManager.playNext()
        }

        // Shuffle button
        binding.shuffleButton.setOnClickListener {
            PlaybackManager.toggleShuffle()
            updateShuffleButton(binding.shuffleButton)
        }
        updateShuffleButton(binding.shuffleButton)

        // Volume slider
        binding.volumeSlider.max = 100
        binding.volumeSlider.progress = (PlaybackManager.volume * 100).toInt()
        binding.volumeSlider.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(seekBar: SeekBar?, progress: Int, fromUser: Boolean) {
                if (fromUser) {
                    PlaybackManager.setVolume(progress / 100f)
                }
            }
            override fun onStartTrackingTouch(seekBar: SeekBar?) {}
            override fun onStopTrackingTouch(seekBar: SeekBar?) {}
        })

        val player = PlaybackManager.getPlayer(this)
        binding.playerView.player = player

        updateLoopButton(binding.loopButton)
    }

    private fun updateLoopButton(btn: ImageButton) {
        btn.alpha = if (PlaybackManager.loopMode == LoopMode.NoRepeat) 0.4f else 1.0f
    }

    private fun updateShuffleButton(btn: ImageButton) {
        btn.alpha = if (PlaybackManager.shuffle) 1.0f else 0.4f
    }

    override fun onDestroy() {
        binding.playerView.player = null
        super.onDestroy()
    }

    companion object {
        const val EXTRA_PATH = "path"
        const val EXTRA_TITLE = "title"
        const val EXTRA_STREAM = "stream"
    }
}
