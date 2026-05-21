package net.mp3party.downloader

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.DefaultItemAnimator
import androidx.recyclerview.widget.LinearLayoutManager
import com.google.android.material.snackbar.Snackbar
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.mp3party.downloader.databinding.FragmentSearchBinding

class SearchFragment : Fragment() {

    private var _binding: FragmentSearchBinding? = null
    private val binding get() = _binding!!
    private lateinit var adapter: TrackAdapter

    private var source = DownloadSource.MP3Party
    private var ytFormat = YtFormat.MP3

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = FragmentSearchBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        adapter = TrackAdapter(
            onDownload = { track, position ->
                (activity as? MainActivity)?.startDownload(track, ytFormat, adapter, position)
            },
            onStream = { track, position ->
                (activity as? MainActivity)?.startStream(track, ytFormat, adapter, position)
            },
        )
        binding.resultsList.layoutManager = LinearLayoutManager(requireContext())
        binding.resultsList.itemAnimator = DefaultItemAnimator()
        binding.resultsList.adapter = adapter

        binding.chipMp3party.setOnClickListener { setSource(DownloadSource.MP3Party) }
        binding.chipDrivemusic.setOnClickListener { setSource(DownloadSource.DriveMusic) }
        binding.chipYoutube.setOnClickListener { setSource(DownloadSource.YouTube) }
        binding.chipMp3.setOnClickListener { ytFormat = YtFormat.MP3 }
        binding.chipMp4.setOnClickListener { ytFormat = YtFormat.MP4 }

        binding.searchButton.setOnClickListener { runSearch() }
        binding.searchInput.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_SEARCH) {
                runSearch()
                true
            } else false
        }

        updateYtdlpStatus()
        updateEmptyState(show = true, hasResults = false)
    }

    private fun setSource(newSource: DownloadSource) {
        source = newSource
        binding.formatChips.isVisible = source == DownloadSource.YouTube
        binding.statusText.text = when (source) {
            DownloadSource.MP3Party -> "Поиск на mp3party.net"
            DownloadSource.DriveMusic -> "Поиск на drivemusic.me"
            DownloadSource.YouTube -> "Поиск на YouTube (yt-dlp)"
        }
        updateYtdlpStatus()
    }

    private fun updateYtdlpStatus() {
        if (source != DownloadSource.YouTube) {
            binding.ytdlpStatus.isVisible = false
            return
        }
        binding.ytdlpStatus.isVisible = true
        binding.ytdlpStatus.text = when {
            YtDlpHelper.isReady -> getString(R.string.ytdlp_ready)
            YtDlpHelper.initError != null ->
                getString(R.string.ytdlp_error, YtDlpHelper.initError)
            else -> getString(R.string.ytdlp_init)
        }
    }

    private fun runSearch() {
        val query = binding.searchInput.text?.toString()?.trim().orEmpty()
        if (query.isEmpty()) {
            Snackbar.make(binding.root, "Введите запрос", Snackbar.LENGTH_SHORT).show()
            return
        }

        hideKeyboard()
        (activity as? MainActivity)?.showLoading(
            true,
            when (source) {
                DownloadSource.YouTube -> "YouTube: «$query»…"
                DownloadSource.DriveMusic -> "DriveMusic: «$query»…"
                DownloadSource.MP3Party -> "MP3Party: «$query»…"
            },
        )
        binding.searchButton.isEnabled = false
        binding.statusText.text = when (source) {
            DownloadSource.YouTube -> if (YtDlpHelper.isReady) {
                "⏳ Поиск YouTube…"
            } else {
                "⏳ Загрузка yt-dlp (первый раз ~1–2 мин)…"
            }
            else -> "⏳ Поиск…"
        }

        lifecycleScope.launch {
            try {
                if (source == DownloadSource.YouTube) {
                    val activity = activity as? MainActivity
                    if (activity == null || !activity.ensureYtDlp()) {
                        binding.statusText.text = "❌ ${getString(R.string.ytdlp_not_installed)}"
                        return@launch
                    }
                }
                val results = withContext(Dispatchers.IO) {
                    when (source) {
                        DownloadSource.MP3Party -> Mp3PartyApi.search(query)
                        DownloadSource.DriveMusic -> DriveMusicApi.search(query)
                        DownloadSource.YouTube -> {
                            val app = requireContext().applicationContext
                            YtDlpHelper.search(app, query)
                        }
                    }
                }
                adapter.submit(results)
                val count = results.size
                updateEmptyState(show = count == 0, hasResults = count > 0)
                binding.statusText.text = if (count > 0) {
                    "✅ Найдено $count ${tracksWord(count)}"
                } else {
                    "Ничего не найдено"
                }
            } catch (e: Exception) {
                updateEmptyState(show = true, hasResults = false)
                binding.statusText.text = "❌ ${e.message}"
                Snackbar.make(binding.root, e.message ?: "Ошибка", Snackbar.LENGTH_LONG).show()
            } finally {
                (activity as? MainActivity)?.showLoading(false, "")
                binding.searchButton.isEnabled = true
                updateYtdlpStatus()
            }
        }
    }

    private fun hideKeyboard() {
        val imm = requireContext().getSystemService(InputMethodManager::class.java) ?: return
        imm.hideSoftInputFromWindow(binding.searchInput.windowToken, 0)
    }

    private fun updateEmptyState(show: Boolean, hasResults: Boolean) {
        binding.emptyState.root.isVisible = show && !hasResults
        binding.resultsList.isVisible = hasResults
    }

    private fun tracksWord(n: Int): String = when {
        n % 100 in 11..14 -> "треков"
        n % 10 == 1 -> "трек"
        n % 10 in 2..4 -> "трека"
        else -> "треков"
    }

    fun refreshPlaybackButtons() {
        if (_binding == null || !::adapter.isInitialized) return
        adapter.notifyDataSetChanged()
    }

    fun refreshYtdlpStatus() {
        if (_binding == null) return
        updateYtdlpStatus()
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}
