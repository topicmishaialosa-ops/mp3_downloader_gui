package net.mp3party.downloader

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.isVisible
import androidx.fragment.app.Fragment
import androidx.recyclerview.widget.LinearLayoutManager
import com.google.android.material.snackbar.Snackbar
import net.mp3party.downloader.databinding.FragmentLibraryBinding

class LibraryFragment : Fragment() {

    private var _binding: FragmentLibraryBinding? = null
    private val binding get() = _binding!!
    private lateinit var adapter: LibraryAdapter

    private val playbackListener: (PlaybackManager.PlayerState) -> Unit = { state ->
        activity?.runOnUiThread {
            adapter.setPlaybackState(state.file, state.isPlaying)
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = FragmentLibraryBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        adapter = LibraryAdapter(
            onPlay = { item ->
                (activity as? MainActivity)?.playMedia(item.file, item.displayName, item.isVideo)
            },
            onToggle = { item ->
                if (PlaybackManager.isCurrentFile(item.file)) {
                    PlaybackManager.togglePlayPause(requireContext())
                } else {
                    (activity as? MainActivity)?.playMedia(item.file, item.displayName, item.isVideo)
                }
            },
            onAddToPlaylist = { item ->
                val title = item.displayName
                val playlistItem = PlaylistItem(
                    pathOrUrl = item.file.absolutePath,
                    title = title,
                    subtitle = if (item.isVideo) "Видео" else "Аудио",
                    isVideo = item.isVideo,
                    isUrl = false,
                )
                PlaybackManager.addToPlaylist(playlistItem)
                Snackbar.make(binding.root, "➕ $title", Snackbar.LENGTH_SHORT).show()
            },
        )
        binding.libraryList.layoutManager = LinearLayoutManager(requireContext())
        binding.libraryList.adapter = adapter

        binding.openFolderButton.setOnClickListener {
            val fallback = MusicLibrary.openFolder(requireContext())
            if (fallback != null) {
                Snackbar.make(
                    binding.root,
                    getString(R.string.folder_copy_hint, fallback),
                    Snackbar.LENGTH_LONG,
                ).show()
            }
        }
    }

    override fun onResume() {
        super.onResume()
        PlaybackManager.libraryListener = playbackListener
        refresh()
        val f = PlaybackManager.currentFile
        adapter.setPlaybackState(f, PlaybackManager.isPlaying())
    }

    override fun onPause() {
        if (PlaybackManager.libraryListener === playbackListener) {
            PlaybackManager.libraryListener = null
        }
        super.onPause()
    }

    fun refresh() {
        if (_binding == null) return
        val dir = MusicLibrary.musicDir(requireContext())
        binding.libraryPath.text = dir.absolutePath
        val files = MusicLibrary.listDownloads(requireContext())
        adapter.submit(files)
        binding.libraryStatus.text = if (files.isEmpty()) {
            getString(R.string.library_empty)
        } else {
            "${files.size} ${filesWord(files.size)}"
        }
        binding.libraryEmpty.root.isVisible = files.isEmpty()
        binding.libraryList.isVisible = files.isNotEmpty()
        adapter.setPlaybackState(PlaybackManager.currentFile, PlaybackManager.isPlaying())
    }

    private fun filesWord(n: Int): String = when {
        n % 100 in 11..14 -> "файлов"
        n % 10 == 1 -> "файл"
        n % 10 in 2..4 -> "файла"
        else -> "файлов"
    }

    override fun onDestroyView() {
        if (PlaybackManager.libraryListener === playbackListener) {
            PlaybackManager.libraryListener = null
        }
        super.onDestroyView()
        _binding = null
    }
}
