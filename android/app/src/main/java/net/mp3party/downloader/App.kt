package net.mp3party.downloader

import android.app.Application
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class App : Application() {
    val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    override fun onCreate() {
        super.onCreate()
        appScope.launch(Dispatchers.IO) {
            try {
                YtDlpHelper.init(applicationContext)
            } catch (_: Exception) {
                // initError в YtDlpHelper; повтор при первом поиске YouTube
            }
        }
    }
}
