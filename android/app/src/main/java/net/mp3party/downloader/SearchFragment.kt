package net.mp3party.downloader

import android.content.ClipData
import android.content.ClipboardManager
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
    private var copySource = DownloadSource.MP3Party
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
            onCopyLink = { track ->
                copyDirectLink(track)
            },
            onSaveImpe = { track ->
                saveTrackAsImpe(track)
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
            impePicker.launch("*/*")
        }

        binding.importLinksButton.setOnClickListener { openImportLinksDialog() }

        binding.saveAllImpeButton.setOnClickListener { saveImpeFiles() }

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
                        val url = q.url ?: return@forEachIndexed
                        val lower = url.lowercase()
                        val isDirect = lower.endsWith(".mp3") || lower.endsWith(".mp4") ||
                            lower.endsWith(".m4a") || lower.endsWith(".ogg") ||
                            lower.endsWith(".flac") || lower.endsWith(".wav") ||
                            lower.contains("/download/") || lower.contains("/dl/online/") ||
                            lower.contains("pl.pesni.me")
                        if (isDirect) {
                            val raw = url.substringAfterLast('/').substringBeforeLast('.')
                            val filename = sanitizeFilename(raw)
                            collected.add(Track(
                                id = "",
                                artist = "",
                                title = filename,
                                streamUrl = url,
                                source = source,
                            ))
                            return@forEachIndexed
                        }
                        binding.statusText.text = "[$num/${queries.size}] ⚠️ URL не распознан: $url"
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
                val unique = collected.distinctBy { "${it.source}:${it.id}:${it.streamUrl}" }
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

    private fun openImportLinksDialog() {
        val ctx = requireContext()
        val edit = EditText(ctx).apply {
            hint = "https://dl2.mp3party.net/download/12345\nhttps://cdn.example.com/song.mp3\nhttps://s123.pl.pesni.me/track/abc.mp3"
            setSingleLine(false)
            setLines(6)
            minLines = 3
            maxLines = 12
            setHorizontallyScrolling(true)
            setTextAppearance(android.R.style.TextAppearance_Material_Body1)
        }
        val scroll = android.widget.ScrollView(ctx).apply {
            setPadding(48, 24, 48, 0)
            addView(edit)
        }
        MaterialAlertDialogBuilder(ctx)
            .setTitle("🔗 Импорт прямых ссылок")
            .setMessage("Вставьте прямые ссылки на аудиофайлы (по одной на строку).\nПоддерживаются: .mp3, .mp4, .m4a, .ogg, .flac, .wav, а также ссылки /download/, /dl/online/, pl.pesni.me")
            .setView(scroll)
            .setPositiveButton("📥 Импортировать") { _, _ ->
                val text = edit.text?.toString().orEmpty()
                importDirectLinks(text)
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun importDirectLinks(text: String) {
        val lines = text.lines().map { it.trim() }.filter { it.isNotEmpty() }
        if (lines.isEmpty()) {
            Snackbar.make(binding.root, "Список пуст", Snackbar.LENGTH_SHORT).show()
            return
        }
        val patterns = listOf(".mp3", ".mp4", ".m4a", ".ogg", ".flac", ".wav",
            "/download/", "/dl/online/", "pl.pesni.me")

        lifecycleScope.launch {
            val tracks = withContext(Dispatchers.IO) {
                val result = mutableListOf<Track>()
                for (line in lines) {
                    val lower = line.lowercase()
                    val isDirect = patterns.any { lower.contains(it) }
                    if (!isDirect || !(lower.startsWith("http://") || lower.startsWith("https://"))) continue

                    if (lower.contains("mp3party.net/download/")) {
                        val id = line.substringAfterLast('/').substringBefore('.').trim()
                        if (id.isNotEmpty()) {
                            val fetched = Mp3PartyApi.fetchTrack(id)
                            if (fetched != null) {
                                result.add(fetched.copy(streamUrl = line))
                                continue
                            }
                        }
                    } else if (lower.contains("pl.pesni.me") || lower.contains("dw.pesni.me")) {
                        val id = line.substringAfterLast('/').substringBefore('.').trim()
                        if (id.isNotEmpty()) {
                            val fetched = PesniMeApi.fetchTrack(id)
                            if (fetched != null) {
                                result.add(fetched.copy(streamUrl = line))
                                continue
                            }
                        }
                    }
                    val raw = line.substringAfterLast('/').substringBeforeLast('.')
                    val filename = sanitizeFilename(raw)
                    result.add(Track(id = "", artist = "", title = filename, streamUrl = line, source = source))
                }
                result
            }
            if (tracks.isEmpty()) {
                Snackbar.make(binding.root, "Не удалось распознать ссылки", Snackbar.LENGTH_SHORT).show()
                return@launch
            }
            adapter.submit(tracks)
            updateEmptyState(show = false, hasResults = true)
            binding.statusText.text = "✅ Импортировано ${tracks.size} ссылок"
        }
    }

    private fun updateEmptyState(show: Boolean, hasResults: Boolean) {
        binding.emptyState.root.isVisible = show && !hasResults
        binding.resultsList.isVisible = hasResults
        binding.downloadAllButton.isVisible = hasResults
        binding.saveAllImpeButton.isVisible = hasResults
    }

    private fun saveImpeFiles() {
        val tracks = adapter.getItems()
        if (tracks.isEmpty()) return
        val dir = java.io.File(requireContext().getExternalFilesDir(null), "impe")
        dir.mkdirs()
        var saved = 0
        for (t in tracks) {
            val impe = buildString {
                appendLine("source=${t.source.name}")
                appendLine("id=${t.id}")
                appendLine("artist=${t.artist}")
                appendLine("title=${t.title}")
                appendLine("url=${t.streamUrl}")
            }
            val fname = "${t.artist.replace(' ', '_')}_${t.title.replace(' ', '_')}.impe"
            val file = java.io.File(dir, fname)
            file.writeText(impe)
            saved++
        }
        Snackbar.make(binding.root, "💾 Сохранено .impe: $saved в ${dir.absolutePath}", Snackbar.LENGTH_LONG).show()
    }

    private fun tracksWord(n: Int): String = when {
        n % 100 in 11..14 -> "треков"
        n % 10 == 1 -> "трек"
        n % 10 in 2..4 -> "трека"
        else -> "треков"
    }

    private fun saveTrackAsImpe(track: Track) {
        val dir = java.io.File(requireContext().getExternalFilesDir(null), "impe")
        dir.mkdirs()
        val impe = buildString {
            appendLine("source=${track.source.name}")
            appendLine("id=${track.id}")
            appendLine("artist=${track.artist}")
            appendLine("title=${track.title}")
            appendLine("url=${track.streamUrl}")
        }
        val fname = "${track.artist.replace(' ', '_')}_${track.title.replace(' ', '_')}.impe"
        val file = java.io.File(dir, fname)
        file.writeText(impe)
        Snackbar.make(binding.root, "💾 Сохранено: ${file.absolutePath}", Snackbar.LENGTH_LONG).show()
    }

    private fun copyDirectLink(track: Track) {
        val url = when (copySource) {
            DownloadSource.MP3Party -> {
                if (track.streamUrl.startsWith("http")) track.streamUrl
                else "https://dl2.mp3party.net/online/${track.id}.mp3"
            }
            DownloadSource.DriveMusic -> track.streamUrl
            DownloadSource.PesniMe -> track.streamUrl
            DownloadSource.YouTube -> track.streamUrl
        }
        if (url.startsWith("http")) {
            val clipboard = requireContext().getSystemService(android.content.Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(ClipData.newPlainText("direct_link", url))
            Snackbar.make(binding.root, "📋 Скопировано: $url", Snackbar.LENGTH_SHORT).show()
        } else {
            Snackbar.make(binding.root, "❌ Ссылка недоступна", Snackbar.LENGTH_SHORT).show()
        }
    }

    fun refreshPlaybackButtons() {
        if (_binding == null || !::adapter.isInitialized) return
        adapter.notifyDataSetChanged()
    }

    fun refreshYtdlpStatus() {
        if (_binding == null) return
        updateYtdlpStatus()
    }

    private fun sanitizeFilename(raw: String): String {
        // 1) URL-декодировать (%D0%A1 → К, + → пробел)
        var name = try {
            java.net.URLDecoder.decode(raw.replace('+', ' '), "UTF-8")
        } catch (_: Exception) { raw }

        // 2) Попробовать base64-декодировать
        val b64Clean = name.replace('-', '+').replace('_', '/').trimEnd('=')
        val padded = b64Clean + "====".take(4 - b64Clean.length % 4).let { if (b64Clean.length % 4 == 0) "" else it }
        if (b64Clean.length >= 6) {
            try {
                val decoded = android.util.Base64.decode(padded, android.util.Base64.DEFAULT)
                val text = String(decoded, Charsets.UTF_8)
                if (Regex("\\p{L}").containsMatchIn(text) && !text.contains('\u0000')) {
                    name = text
                }
            } catch (_: Exception) { /* не base64 */ }
        }

        return name
            .replace('_', ' ')
            .replace(Regex("[^\\p{L}\\p{N}\\s\\-+]"), "")
            .replace(Regex("\\s+"), " ")
            .trim()
            .ifEmpty { "track" }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }
}
