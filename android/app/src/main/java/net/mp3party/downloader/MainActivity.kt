package net.mp3party.downloader

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.core.view.updatePadding
import androidx.fragment.app.commit
import androidx.lifecycle.lifecycleScope
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.snackbar.Snackbar
import com.google.android.material.slider.Slider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlin.coroutines.resume
import net.mp3party.downloader.databinding.ActivityMainBinding
import net.mp3party.downloader.databinding.PlayerBarBinding
import java.io.File

class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding
    private lateinit var playerBarBinding: PlayerBarBinding
    private var searchFragment: SearchFragment? = null
    private var libraryFragment: LibraryFragment? = null
    private var downloading = false
    private var streaming = false
    private var seekUserDragging = false
    private var progressJob: Job? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)

        playerBarBinding = PlayerBarBinding.bind(binding.playerBar.root)
        setupInsets()
        setupNavigation()
        setupPlayerBar()

        if (savedInstanceState == null) {
            searchFragment = SearchFragment()
            supportFragmentManager.commit {
                replace(R.id.fragmentContainer, searchFragment!!)
            }
        } else {
            searchFragment = supportFragmentManager.fragments
                .filterIsInstance<SearchFragment>()
                .firstOrNull()
        }

        PlaybackManager.listener = { state ->
            runOnUiThread {
                updatePlayerBar(state)
                searchFragment?.refreshPlaybackButtons()
            }
        }
        PlaybackManager.errorListener = { msg ->
            runOnUiThread {
                Snackbar.make(binding.root, msg, Snackbar.LENGTH_LONG).show()
            }
        }
    }

    override fun onStart() {
        super.onStart()
        startProgressUpdates()
    }

    override fun onStop() {
        progressJob?.cancel()
        progressJob = null
        super.onStop()
    }

    private fun setupInsets() {
        ViewCompat.setOnApplyWindowInsetsListener(binding.root) { _, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            binding.bottomNav.updatePadding(bottom = bars.bottom)
            insets
        }
    }

    private fun setupNavigation() {
        binding.bottomNav.setOnItemSelectedListener { item ->
            when (item.itemId) {
                R.id.nav_search -> {
                    if (searchFragment == null) searchFragment = SearchFragment()
                    supportFragmentManager.commit {
                        replace(R.id.fragmentContainer, searchFragment!!)
                    }
                    true
                }
                R.id.nav_library -> {
                    if (libraryFragment == null) libraryFragment = LibraryFragment()
                    supportFragmentManager.commit {
                        replace(R.id.fragmentContainer, libraryFragment!!)
                    }
                    libraryFragment?.refresh()
                    true
                }
                else -> false
            }
        }
        binding.bottomNav.selectedItemId = R.id.nav_search
    }

    private fun setupPlayerBar() {
        playerBarBinding.playPauseButton.setOnClickListener {
            PlaybackManager.togglePlayPause(this)
        }
        playerBarBinding.closePlayerButton.setOnClickListener {
            PlaybackManager.stop()
            binding.playerBar.root.isVisible = false
        }
        playerBarBinding.expandButton.setOnClickListener {
            openFullscreenPlayer()
        }
        playerBarBinding.seekSlider.addOnChangeListener { _, value, fromUser ->
            if (fromUser) {
                seekUserDragging = true
                playerBarBinding.positionText.text = formatMs(value.toLong())
            }
        }
        playerBarBinding.seekSlider.addOnSliderTouchListener(
            object : Slider.OnSliderTouchListener {
                override fun onStartTrackingTouch(slider: Slider) {
                    seekUserDragging = true
                }

                override fun onStopTrackingTouch(slider: Slider) {
                    PlaybackManager.seekTo(slider.value.toLong())
                    seekUserDragging = false
                }
            },
        )
    }

    private fun startProgressUpdates() {
        progressJob?.cancel()
        progressJob = lifecycleScope.launch {
            while (isActive) {
                if (binding.playerBar.root.isVisible && !PlaybackManager.isVideo) {
                    val pos = PlaybackManager.getPositionMs()
                    val dur = PlaybackManager.getDurationMs()
                    if (!seekUserDragging) {
                        updateSeekUi(pos, dur)
                    }
                }
                delay(400)
            }
        }
    }

    private fun updateSeekUi(positionMs: Long, durationMs: Long) {
        playerBarBinding.positionText.text = formatMs(positionMs)
        playerBarBinding.durationText.text =
            if (durationMs > 0) formatMs(durationMs) else "--:--"
        if (durationMs <= 0 || seekUserDragging) return

        val slider = playerBarBinding.seekSlider
        val durF = durationMs.toFloat().coerceAtLeast(1f)
        val posF = positionMs.toFloat().coerceIn(0f, durF)
        try {
            if (slider.valueFrom != 0f) slider.valueFrom = 0f
            if (slider.valueTo != durF) {
                if (slider.value > durF) slider.value = durF
                slider.valueTo = durF
            }
            if (kotlin.math.abs(slider.value - posF) > 500f) {
                slider.value = posF
            }
        } catch (_: IllegalStateException) {
            // длительность ещё неизвестна или слайдер в переходном состоянии
        }
    }

    private fun formatMs(ms: Long): String {
        val totalSec = (ms / 1000).coerceAtLeast(0)
        val m = totalSec / 60
        val s = totalSec % 60
        return "%d:%02d".format(m, s)
    }

    private var playerBarShowsPause: Boolean? = null

    private fun updatePlayerBar(state: PlaybackManager.PlayerState) {
        val visible = state.hasActiveMedia
        binding.playerBar.root.isVisible = visible
        if (!visible) {
            playerBarShowsPause = null
            return
        }
        playerBarBinding.playerTitle.text = state.title
        playerBarBinding.playerSubtitle.text = when {
            state.isStream && state.isVideo -> "Стрим YouTube · ⛶ полный экран"
            state.isStream -> "Стрим YouTube"
            state.isVideo -> "Видео · нажмите ⛶ для полного экрана"
            else -> "Аудио"
        }
        val showPause = state.isPlaying
        if (playerBarShowsPause != showPause) {
            playerBarShowsPause = showPause
            animatePlayPauseIcon(playerBarBinding.playPauseButton, showPause)
        }
        playerBarBinding.expandButton.isVisible = state.isVideo
        playerBarBinding.seekBlock.isVisible = !state.isVideo
    }

    private fun animatePlayPauseIcon(button: android.widget.ImageButton, toPause: Boolean) {
        button.animate()
            .scaleX(0.85f)
            .scaleY(0.85f)
            .setDuration(70)
            .withEndAction {
                button.setImageResource(if (toPause) R.drawable.ic_pause else R.drawable.ic_play)
                button.animate()
                    .scaleX(1f)
                    .scaleY(1f)
                    .setDuration(120)
                    .start()
            }
            .start()
    }

    fun showLoading(show: Boolean, text: String) {
        binding.loadingOverlay.isVisible = show
        if (text.isNotEmpty()) binding.loadingText.text = text
    }

    /** Запросить загрузку yt-dlp (Android-библиотека), если ещё не готов. */
    suspend fun ensureYtDlp(): Boolean {
        if (YtDlpHelper.isReady) return true
        return suspendCancellableCoroutine { cont ->
            MaterialAlertDialogBuilder(this)
                .setTitle(R.string.ytdlp_dialog_title)
                .setMessage(R.string.ytdlp_dialog_message)
                .setPositiveButton(R.string.ytdlp_download) { _, _ ->
                    lifecycleScope.launch {
                        try {
                            withContext(Dispatchers.IO) {
                                YtDlpHelper.init(applicationContext)
                            }
                            searchFragment?.refreshYtdlpStatus()
                            cont.resume(true)
                        } catch (e: Exception) {
                            Snackbar.make(
                                binding.root,
                                e.message ?: getString(R.string.ytdlp_not_installed),
                                Snackbar.LENGTH_LONG,
                            ).show()
                            cont.resume(false)
                        }
                    }
                }
                .setNegativeButton(android.R.string.cancel) { _, _ -> cont.resume(false) }
                .setOnCancelListener { cont.resume(false) }
                .show()
        }
    }

    private fun openFullscreenPlayer() {
        if (!PlaybackManager.isVideo) return
        val intent = Intent(this, PlayerActivity::class.java).apply {
            putExtra(PlayerActivity.EXTRA_TITLE, PlaybackManager.currentTitle)
            PlaybackManager.currentFile?.absolutePath?.let {
                putExtra(PlayerActivity.EXTRA_PATH, it)
            }
            if (PlaybackManager.isStream) {
                putExtra(PlayerActivity.EXTRA_STREAM, true)
            }
        }
        startActivity(intent)
    }

    fun startStream(
        track: Track,
        format: YtFormat,
        adapter: TrackAdapter,
        position: Int,
    ) {
        if (track.source != DownloadSource.YouTube) return
        if (downloading || streaming) {
            Snackbar.make(binding.root, "Дождитесь завершения операции", Snackbar.LENGTH_SHORT).show()
            return
        }
        streaming = true
        adapter.setStreamingPosition(position)
        showLoading(true, "🎧 ${track.artist} — ${track.title}")

        lifecycleScope.launch {
            try {
                if (!ensureYtDlp()) {
                    return@launch
                }
                val url = withContext(Dispatchers.IO) {
                    YtDlpHelper.getStreamUrl(applicationContext, track, format)
                }
                val title = listOf(track.artist, track.title)
                    .filter { it.isNotBlank() }
                    .joinToString(" — ")
                val isVideo = format == YtFormat.MP4
                PlaybackManager.playStream(this@MainActivity, url, title, track.id, isVideo)
                binding.playerBar.root.isVisible = true
                if (isVideo) {
                    openFullscreenPlayer()
                }
            } catch (e: Exception) {
                Snackbar.make(binding.root, e.message ?: "Ошибка стрима", Snackbar.LENGTH_LONG).show()
            } finally {
                streaming = false
                adapter.clearStreaming()
                showLoading(false, "")
            }
        }
    }

    fun startDownload(
        track: Track,
        format: YtFormat,
        adapter: TrackAdapter,
        position: Int,
    ) {
        if (downloading || streaming) {
            Snackbar.make(binding.root, "Дождитесь завершения операции", Snackbar.LENGTH_SHORT).show()
            return
        }
        downloading = true
        adapter.setDownloadingPosition(position)
        showLoading(true, "📥 ${track.artist} — ${track.title}")

        lifecycleScope.launch {
            try {
                if (track.source == DownloadSource.YouTube && !ensureYtDlp()) {
                    return@launch
                }
                val file = DownloadHelper.download(
                    this@MainActivity,
                    track,
                    format,
                ) { pct, line ->
                    runOnUiThread {
                        binding.loadingText.text = if (line.isNotEmpty()) {
                            "📥 ${(pct * 100).toInt()}% — $line"
                        } else {
                            "📥 ${(pct * 100).toInt()}%"
                        }
                    }
                }
                Snackbar.make(
                    binding.root,
                    "Скачано: ${file.name}",
                    Snackbar.LENGTH_LONG,
                ).show()
                libraryFragment?.refresh()
            } catch (e: Exception) {
                Snackbar.make(binding.root, e.message ?: "Ошибка", Snackbar.LENGTH_LONG).show()
            } finally {
                downloading = false
                adapter.clearDownloading()
                showLoading(false, "")
            }
        }
    }

    fun startDownloadAll(
        tracks: List<Track>,
        format: YtFormat,
    ) {
        if (downloading || streaming) {
            Snackbar.make(binding.root, "Дождитесь завершения операции", Snackbar.LENGTH_SHORT).show()
            return
        }
        downloading = true
        showLoading(true, "📥 Подготовка к скачиванию ${tracks.size} треков…")

        lifecycleScope.launch {
            var downloadedCount = 0
            var errorCount = 0
            for ((index, track) in tracks.withIndex()) {
                val trackNum = index + 1
                try {
                    if (track.source == DownloadSource.YouTube && !ensureYtDlp()) {
                        errorCount++
                        continue
                    }
                    showLoading(true, "📥 $trackNum/${tracks.size}: ${track.artist} — ${track.title}")
                    DownloadHelper.download(
                        this@MainActivity,
                        track,
                        format,
                    ) { pct, line ->
                        runOnUiThread {
                            binding.loadingText.text = if (line.isNotEmpty()) {
                                "📥 $trackNum/${tracks.size} (${(pct * 100).toInt()}%): $line"
                            } else {
                                "📥 $trackNum/${tracks.size} (${(pct * 100).toInt()}%)"
                            }
                        }
                    }
                    downloadedCount++
                } catch (e: Exception) {
                    errorCount++
                }
            }
            Snackbar.make(
                binding.root,
                "Успешно скачано: $downloadedCount/${tracks.size}" + (if (errorCount > 0) " (ошибок: $errorCount)" else ""),
                Snackbar.LENGTH_LONG,
            ).show()
            libraryFragment?.refresh()
            downloading = false
            showLoading(false, "")
        }
    }

    fun playMedia(file: File, title: String, isVideo: Boolean) {
        PlaybackManager.play(this, file, title, isVideo)
        binding.playerBar.root.isVisible = true
        if (isVideo) {
            startActivity(
                Intent(this, PlayerActivity::class.java).apply {
                    putExtra(PlayerActivity.EXTRA_PATH, file.absolutePath)
                    putExtra(PlayerActivity.EXTRA_TITLE, title)
                },
            )
        }
    }

    override fun onDestroy() {
        PlaybackManager.listener = null
        PlaybackManager.errorListener = null
        super.onDestroy()
    }
}
