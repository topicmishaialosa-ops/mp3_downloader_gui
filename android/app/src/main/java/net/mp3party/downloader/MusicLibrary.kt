package net.mp3party.downloader

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import androidx.core.content.FileProvider
import java.io.File
import java.net.URLEncoder

object MusicLibrary {
    private val mediaExt = setOf("mp3", "mp4", "m4a", "opus", "webm", "mkv", "wav", "flac")
    private val videoExt = setOf("mp4", "webm", "mkv")

    fun musicDir(context: Context): File =
        context.getExternalFilesDir(Environment.DIRECTORY_MUSIC) ?: File(context.filesDir, "music")

    fun listDownloads(context: Context): List<LocalMediaFile> {
        val dir = musicDir(context)
        if (!dir.exists()) dir.mkdirs()
        return dir.listFiles()
            ?.filter { it.isFile && it.extension.lowercase() in mediaExt }
            ?.map { file ->
                val ext = file.extension.lowercase()
                LocalMediaFile(
                    file = file,
                    displayName = file.nameWithoutExtension,
                    isVideo = ext in videoExt,
                    sizeBytes = file.length(),
                )
            }
            ?.sortedByDescending { it.file.lastModified() }
            ?: emptyList()
    }

    /**
     * Открывает папку Music в файловом менеджере.
     * @return null — успех; иначе текст для Snackbar (путь скопирован в буфер).
     */
    fun openFolder(context: Context): String? {
        val dir = musicDir(context)
        if (!dir.exists()) dir.mkdirs()

        val authority = "${context.packageName}.fileprovider"

        // 1) Стандартный Documents UI (primary:Android/data/…/files/Music)
        buildStorageDocumentUri(dir)?.let { docUri ->
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(docUri, DocumentsContract.Document.MIME_TYPE_DIR)
                addCategory(Intent.CATEGORY_DEFAULT)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            if (intent.resolveActivity(context.packageManager) != null) {
                try {
                    context.startActivity(Intent.createChooser(intent, "Открыть папку"))
                    return null
                } catch (_: Exception) {
                    // пробуем следующий способ
                }
            }
        }

        // 2) FileProvider + MIME каталога
        try {
            val uri = FileProvider.getUriForFile(context, authority, dir)
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
                addCategory(Intent.CATEGORY_DEFAULT)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            if (intent.resolveActivity(context.packageManager) != null) {
                context.startActivity(Intent.createChooser(intent, "Открыть папку"))
                return null
            }
        } catch (_: Exception) {
            /* next */
        }

        // 3) Открыть любой файл из папки — пользователь увидит каталог в «Назад»
        dir.listFiles()?.maxByOrNull { it.lastModified() }?.let { newest ->
            try {
                val uri = FileProvider.getUriForFile(context, authority, newest)
                val mime = if (newest.extension.lowercase() in videoExt) "video/*" else "audio/*"
                val intent = Intent(Intent.ACTION_VIEW).apply {
                    setDataAndType(uri, mime)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                }
                if (intent.resolveActivity(context.packageManager) != null) {
                    context.startActivity(Intent.createChooser(intent, "Файлы в папке Music"))
                    return null
                }
            } catch (_: Exception) {
                /* next */
            }
        }

        copyPath(context, dir.absolutePath)
        return dir.absolutePath
    }

    private fun buildStorageDocumentUri(dir: File): Uri? {
        val path = dir.absolutePath
        val prefix = "/storage/emulated/0/"
        if (!path.startsWith(prefix)) return null
        val relative = path.removePrefix(prefix)
        val docId = "primary:$relative"
        val encoded = URLEncoder.encode(docId, Charsets.UTF_8.name())
            .replace("+", "%20")
        return Uri.parse("content://com.android.externalstorage.documents/document/$encoded")
    }

    private fun copyPath(context: Context, path: String) {
        val clipboard = context.getSystemService(ClipboardManager::class.java) ?: return
        clipboard.setPrimaryClip(ClipData.newPlainText("music_folder", path))
    }

    fun shareFile(context: Context, file: File): Boolean {
        val uri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            file,
        )
        val mime = if (file.extension.lowercase() in videoExt) "video/*" else "audio/*"
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, mime)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        return try {
            context.startActivity(Intent.createChooser(intent, "Открыть"))
            true
        } catch (_: Exception) {
            false
        }
    }
}
