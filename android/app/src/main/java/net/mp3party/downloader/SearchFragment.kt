package net.mp3party.downloader

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.FileProvider
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.DefaultItemAnimator
import androidx.recyclerview.widget.LinearLayoutManager
import com.google.android.material.dialog.MaterialAlertDialogBuilder
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
            onAddToPlaylist = { track ->
                val title = listOf(track.artist, track.title)
                    .filter { it.isNotBlank() }
                    .joinToString(" — ")
                val subtitle = track.source.name
                val url = track.streamUrl
                val item = PlaylistItem(
                    pathOrUrl = url,
                    title = title,
                    subtitle = subtitle,
                    isVideo = false,
                    isUrl = url.isNotEmpty(),
                )
                PlaybackManager.addToPlaylist(item)
                Snackbar.make(binding.root, "➕ $title", Snackbar.LENGTH_SHORT).show()
            },
            onShare = { track ->
                shareTrack(track)
            },
        )
        binding.resultsList.layoutManager = LinearLayoutManager(requireContext())
        binding.resultsList.itemAnimator = DefaultItemAnimator()
        binding.resultsList.adapter = adapter

        binding.chipMp3party.setOnClickListener { setSource(DownloadSource.MP3Party) }
        binding.chipDrivemusic.setOnClickListener { setSource(DownloadSource.DriveMusic) }
        binding.chipPesnime.setOnClickListener { setSource(DownloadSource.PesniMe) }
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

        binding.downloadAllButton.setOnClickListener {
            val tracks = adapter.getItems()
            if (tracks.isNotEmpty()) {
                (activity as? MainActivity)?.startDownloadAll(tracks, ytFormat)
            }
        }

        binding.batchButton.setOnClickListener { openBatchDialog() }

        val impePicker = registerForActivityResult(ActivityResultContracts.GetContent()) { uri ->
            uri ?: return@registerForActivityResult
            (activity as? MainActivity)?.handleImpeUri(uri)
        }
        binding.impeButton.setOnClickListener {
            val options = arrayOf("📁 Из файла", "🌐 По ссылке")
            MaterialAlertDialogBuilder(requireContext())
                .setTitle("Импорт")
                .setItems(options) { _, which ->
                    when (which) {
                        0 -> impePicker.launch("*/*")
                        1 -> showImpeUrlDialog()
                    }
                }
                .setNegativeButton("Отмена", null)
                .show()
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
            DownloadSource.PesniMe -> "Поиск на pesni.me"
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
                DownloadSource.PesniMe -> "Pesni.me: «$query»…"
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
                        DownloadSource.PesniMe -> PesniMeApi.search(query)
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

    private fun openBatchDialog() {
        val ctx = requireContext()
        val edit = EditText(ctx).apply {
            hint = "Кино - Группа крови\nАгата Кристи - Опиум для никого\nСектор Газа - Лирика"
            setSingleLine(false)
            setLines(8)
            minLines = 4
            maxLines = 16
            setHorizontallyScrolling(true)
            setTextAppearance(android.R.style.TextAppearance_Material_Body1)
        }
        val autodlCb = CheckBox(ctx).apply {
            text = "⬇ Автоскачивать первый трек"
            setPadding(48, 0, 48, 0)
        }
        val scroll = android.widget.ScrollView(ctx).apply {
            setPadding(48, 24, 48, 0)
            addView(edit)
        }
        val layout = LinearLayout(ctx).apply {
            orientation = LinearLayout.VERTICAL
            addView(scroll)
            addView(autodlCb)
        }
        MaterialAlertDialogBuilder(ctx)
            .setTitle("📋 Пакетный поиск")
            .setMessage(
                "По одному треку на строку.\n" +
                    "Формат: «Исполнитель - Название», «Название» (без разделителя), или URL.\n" +
                    "Нумерация («1. », «12) ») и комментарии после «#» игнорируются.",
            )
            .setView(layout)
            .setPositiveButton("▶ Найти по списку") { _, _ ->
                val text = edit.text?.toString().orEmpty()
                runBatchSearch(text, autodlCb.isChecked)
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun runBatchSearch(input: String, autodownload: Boolean = false) {
        val queries = BatchQueryParser.parse(input)
        if (queries.isEmpty()) {
            Snackbar.make(binding.root, "Список пуст", Snackbar.LENGTH_SHORT).show()
            return
        }
        (activity as? MainActivity)?.showLoading(
            true,
            "Пакетный поиск: ${queries.size} запрос(ов)…",
        )
        binding.statusText.text = "⏳ Пакетный поиск (${queries.size})…"
        binding.searchButton.isEnabled = false
        binding.batchButton.isEnabled = false

        lifecycleScope.launch {
            try {
                if (source == DownloadSource.YouTube) {
                    val activity = activity as? MainActivity
                    if (activity == null || !activity.ensureYtDlp()) {
                        binding.statusText.text = "❌ ${getString(R.string.ytdlp_not_installed)}"
                        return@launch
                    }
                }
                val collected = mutableListOf<Track>()
                val autodlTracks = mutableListOf<Track>()
                var errorCount = 0
                val app = requireContext().applicationContext
                queries.forEachIndexed { idx, q ->
                    val num = idx + 1
                    if (q.isUrl) {
                        binding.statusText.text = "[$num/${queries.size}] ⚠️ URL в списке не поддерживается"
                        return@forEachIndexed
                    }
                    binding.statusText.text = "[$num/${queries.size}] 🔎 ${q.searchText()}"
                    try {
                        val results = withContext(Dispatchers.IO) {
                            when (source) {
                                DownloadSource.MP3Party -> Mp3PartyApi.search(q.searchText())
                                DownloadSource.DriveMusic -> DriveMusicApi.search(q.searchText())
                                DownloadSource.PesniMe -> PesniMeApi.search(q.searchText())
                                DownloadSource.YouTube -> YtDlpHelper.search(app, q.searchText())
                            }
                        }
                        if (results.isNotEmpty()) {
                            if (autodownload) autodlTracks.add(results[0])
                            collected.addAll(results)
                        } else {
                            errorCount++
                        }
                    } catch (e: Exception) {
                        errorCount++
                    }
                }
                // Дедупликация по id в пределах источника.
                val unique = collected.distinctBy { "${it.source}:${it.id}" }
                adapter.submit(unique)
                updateEmptyState(show = unique.isEmpty(), hasResults = unique.isNotEmpty())
                if (autodlTracks.isNotEmpty()) {
                    (activity as? MainActivity)?.startDownloadAll(autodlTracks, ytFormat)
                }
                binding.statusText.text = if (unique.isNotEmpty()) {
                    "✅ Найдено ${unique.size} (из ${queries.size} запрос(ов)" +
                        (if (errorCount > 0) ", ошибок: $errorCount" else "") + ")"
                } else {
                    "Ничего не найдено"
                }
            } catch (e: Exception) {
                binding.statusText.text = "❌ ${e.message}"
                Snackbar.make(binding.root, e.message ?: "Ошибка", Snackbar.LENGTH_LONG).show()
            } finally {
                (activity as? MainActivity)?.showLoading(false, "")
                binding.searchButton.isEnabled = true
                binding.batchButton.isEnabled = true
                updateYtdlpStatus()
            }
        }
    }

    private fun updateEmptyState(show: Boolean, hasResults: Boolean) {
        binding.emptyState.root.isVisible = show && !hasResults
        binding.resultsList.isVisible = hasResults
        binding.downloadAllButton.isVisible = hasResults
    }

    private fun tracksWord(n: Int): String = when {
        n % 100 in 11..14 -> "треков"
        n % 10 == 1 -> "трек"
        n % 10 in 2..4 -> "трека"
        else -> "треков"
    }

    private fun impeString(track: Track): String = buildString {
        appendLine("source=${track.source.name}")
        appendLine("id=${track.id}")
        appendLine("artist=${track.artist}")
        appendLine("title=${track.title}")
        appendLine("url=${track.streamUrl}")
    }

    private fun shareTrackAsFile(track: Track) {
        val impe = impeString(track)
        try {
            val dir = java.io.File(requireContext().cacheDir, "impe")
            dir.mkdirs()
            val file = java.io.File(dir, "${track.id}.impe")
            file.writeText(impe)
            val uri = FileProvider.getUriForFile(
                requireContext(),
                "${requireContext().packageName}.fileprovider",
                file,
            )
            val shareIntent = Intent(Intent.ACTION_SEND).apply {
                type = "application/octet-stream"
                putExtra(Intent.EXTRA_STREAM, uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            startActivity(Intent.createChooser(shareIntent, getString(R.string.share_track)))
        } catch (_: Exception) {
            Snackbar.make(binding.root, "Не удалось поделиться", Snackbar.LENGTH_SHORT).show()
        }
    }

    private fun shareTrack(track: Track) {
        val options = arrayOf("🔗 Копировать прямую ссылку", "📁 Сохранить как .impe")
        MaterialAlertDialogBuilder(requireContext())
            .setTitle("${track.artist} — ${track.title}")
            .setItems(options) { _, which ->
                when (which) {
                    0 -> {
                        val directUrl = when (track.source) {
                            DownloadSource.MP3Party -> "https://dl2.mp3party.net/download/${track.id}"
                            DownloadSource.YouTube -> "https://www.youtube.com/watch?v=${track.id}"
                            else -> track.streamUrl
                        }
                        val clipboard = requireContext().getSystemService(android.content.Context.CLIPBOARD_SERVICE) as ClipboardManager
                        clipboard.setPrimaryClip(ClipData.newPlainText("direct_url", directUrl))
                        Snackbar.make(binding.root, "🔗 Прямая ссылка скопирована", Snackbar.LENGTH_SHORT).show()
                    }
                    1 -> shareTrackAsFile(track)
                }
            }
            .setNegativeButton("Отмена", null)
            .show()
    }

    private fun showImpeUrlDialog() {
        val editText = EditText(requireContext()).apply {
            hint = "Ссылка: .impe, YouTube, mp3party.net…"
            inputType = android.text.InputType.TYPE_CLASS_TEXT or android.text.InputType.TYPE_TEXT_VARIATION_URI
        }
        MaterialAlertDialogBuilder(requireContext())
            .setTitle("🌐 Импорт по ссылке")
            .setView(editText)
            .setPositiveButton("Загрузить") { _, _ ->
                val url = editText.text.toString().trim()
                if (url.isNotEmpty()) {
                    loadUrlToTrack(url)
                }
            }
            .setNegativeButton("Отмена", null)
            .show()
    }

    private fun loadUrlToTrack(url: String) {
        lifecycleScope.launch {
            try {
                val track = withContext(Dispatchers.IO) { detectUrlTrack(requireContext(), url) }
                if (track != null) {
                    (activity as? MainActivity)?.showImpeDialog(track)
                    return@launch
                }

                val text = withContext(Dispatchers.IO) {
                    val client = okhttp3.OkHttpClient()
                    val request = okhttp3.Request.Builder().url(url).get().build()
                    client.newCall(request).execute().body?.string()
                }
                if (text.isNullOrEmpty()) {
                    Snackbar.make(binding.root, "❌ Пустой ответ", Snackbar.LENGTH_SHORT).show()
                    return@launch
                }
                (activity as? MainActivity)?.handleImpeText(text)
            } catch (_: Exception) {
                Snackbar.make(binding.root, "❌ Ошибка загрузки", Snackbar.LENGTH_SHORT).show()
            }
        }
    }

    private fun detectUrlTrack(context: android.content.Context, url: String): Track? {
        val ytRe = Regex("(?:youtube\\.com/watch\\?v=|youtu\\.be/)([a-zA-Z0-9_-]{11})")
        val ytM = ytRe.find(url)
        if (ytM != null) {
            val id = ytM.groupValues[1]
            return Track(id = id, artist = "", title = "YouTube #${id.take(8)}",
                streamUrl = "https://www.youtube.com/watch?v=$id", source = DownloadSource.YouTube)
        }

        val idRe = Regex("(?:/download/|/music/|/track/)(\\d+)|(?:^|/)(\\d+)/?\$")
        val idM = idRe.find(url)
        if (idM != null) {
            val id = idM.groupValues[1].ifEmpty { idM.groupValues[2] }
            if (url.contains("pesni")) {
                val client = okhttp3.OkHttpClient()
                val pageUrl = "https://music.pesni.me/track/$id"
                val req = okhttp3.Request.Builder().url(pageUrl)
                    .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
                    .build()
                try {
                    val resp = client.newCall(req).execute()
                    if (resp.isSuccessful) {
                        val body = resp.body?.string() ?: return null
                        val trackRe = Regex("\"id\":(\\d+),\"artist\":\"([^\"]*)\",\"title\":\"([^\"]*)\",")
                        val trM = trackRe.find(body)
                        if (trM != null) {
                            val artist = trM.groupValues[2].trim()
                            val title = trM.groupValues[3].trim()
                            val playRe = Regex("\"play\":\"([^\"]+)\"")
                            val playM = playRe.find(body)
                            val streamUrl = playM?.groupValues?.getOrNull(1) ?: ""
                            return Track(id = id, artist = artist, title = title,
                                streamUrl = streamUrl, source = DownloadSource.PesniMe)
                        }
                    }
                } catch (_: Exception) { }
            }
            if (url.contains("mp3party")) {
                val client = okhttp3.OkHttpClient()
                val pageUrl = "https://mp3party.net/music/$id"
                val req = okhttp3.Request.Builder().url(pageUrl)
                    .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
                    .build()
                try {
                    val resp = client.newCall(req).execute()
                    if (resp.isSuccessful) {
                        val body = resp.body?.string() ?: return null
                        val artistRe = Regex("""data-js-artist-name="([^"]*)"""")
                        val titleRe = Regex("""data-js-song-title="([^"]*)"""")
                        val artist = artistRe.find(body)?.groupValues?.getOrNull(1)?.trim() ?: ""
                        val title = titleRe.find(body)?.groupValues?.getOrNull(1)?.trim() ?: "Track #$id"
                        return Track(id = id, artist = artist, title = title,
                            streamUrl = "https://dl2.mp3party.net/online/$id.mp3", source = DownloadSource.MP3Party)
                    }
                } catch (_: Exception) { }
            }
        }

        if (url.endsWith(".mp3")) {
            val name = url.substringAfterLast('/')
            val clean = name.removeSuffix(".mp3").replace('_', ' ').replace('-', ' ')
            val dd = clean.indexOf("  ")
            val artist = if (dd >= 0) clean.substring(0, dd).trim() else ""
            val title = if (dd >= 0) clean.substring(dd + 2).trim() else clean.trim()
            val source = if (url.contains("pesni")) DownloadSource.PesniMe else DownloadSource.MP3Party
            return Track(id = url, artist = artist,
                title = title.ifEmpty { name },
                streamUrl = url, source = source)
        }

        return null
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
