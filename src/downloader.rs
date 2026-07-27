use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::player::AudioPlayer;
use crate::types::*;
use crate::LinkParserApp;

impl LinkParserApp {
    pub fn ytdlp_install_path() -> PathBuf {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        #[cfg(windows)]
        return home.join("yt-dlp-util").join("bin").join("yt-dlp.exe");
        #[cfg(target_os = "macos")]
        return home.join("yt-dlp-util").join("bin").join("yt-dlp_macos");
        #[cfg(not(any(windows, target_os = "macos")))]
        home.join("yt-dlp-util").join("bin").join("yt-dlp")
    }

    pub fn ytdlp_download_url() -> &'static str {
        #[cfg(windows)]
        return "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
        #[cfg(target_os = "macos")]
        return "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos";
        #[cfg(not(any(windows, target_os = "macos")))]
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    }

    pub fn resolve_yt_dlp() -> Result<PathBuf, String> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let mut file_candidates = vec![Self::ytdlp_install_path()];
        if !home.is_empty() {
            #[cfg(windows)]
            file_candidates.push(PathBuf::from(format!(
                "{home}/yt-dlp-util/.yt-dlp-venv/Scripts/yt-dlp.exe"
            )));
            #[cfg(not(windows))]
            file_candidates.push(PathBuf::from(format!(
                "{home}/yt-dlp-util/.yt-dlp-venv/bin/yt-dlp"
            )));
        }
        for p in file_candidates {
            if p.exists() {
                return Ok(p);
            }
        }

        #[cfg(windows)]
        let which_cmd = "where";
        #[cfg(not(windows))]
        let which_cmd = "which";
        if let Ok(out) = Command::new(which_cmd).arg("yt-dlp").output() {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !path.is_empty() {
                    return Ok(PathBuf::from(path));
                }
            }
        }

        Err(format!(
            "yt-dlp не найден. Установите в PATH или скачайте в {}",
            Self::ytdlp_install_path().display()
        ))
    }

    pub fn install_yt_dlp() -> Result<PathBuf, String> {
        let path = Self::ytdlp_install_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let client = reqwest::blocking::Client::builder()
            .user_agent(BROWSER_USER_AGENT)
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .get(Self::ytdlp_download_url())
            .send()
            .map_err(|e| format!("Скачивание yt-dlp: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Скачивание yt-dlp: HTTP {}", resp.status()));
        }
        let bytes = resp.bytes().map_err(|e| e.to_string())?;
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)
                .map_err(|e| e.to_string())?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).map_err(|e| e.to_string())?;
        }

        Ok(path)
    }

    pub fn prompt_and_install_yt_dlp() -> Result<PathBuf, String> {
        let install_to = Self::ytdlp_install_path();
        let msg = format!(
            "Для YouTube нужен yt-dlp, но он не найден.\n\n\
             Скачать последнюю версию с GitHub в\n{}?",
            install_to.display()
        );
        let yes = rfd::MessageDialog::new()
            .set_title("yt-dlp")
            .set_description(msg)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if yes != rfd::MessageDialogResult::Yes {
            return Err("yt-dlp не установлен — YouTube недоступен.".into());
        }

        let path = Self::install_yt_dlp()?;
        Self::resolve_yt_dlp().map_err(|_| {
            format!(
                "yt-dlp скачан в {}, но не удалось запустить",
                path.display()
            )
        })
    }

    pub fn require_yt_dlp_ui() -> Result<PathBuf, String> {
        Self::resolve_yt_dlp().or_else(|_| Self::prompt_and_install_yt_dlp())
    }

    pub fn offer_mpv_ui(allow_skip: bool) -> bool {
        if AudioPlayer::has_mpv() {
            return true;
        }

        let install_dir = AudioPlayer::mpv_install_dir();
        let msg = if cfg!(target_os = "linux") {
            "Для стриминга и перемотки рекомендуется mpv.\n\n\
             Установите: sudo pacman -S mpv  или  sudo apt install mpv\n\n\
             «Да» — открыть mpv.io\n\
             «Нет» — продолжить без mpv (без перемотки)"
                .to_string()
        } else {
            format!(
                "Для стриминга и перемотки рекомендуется mpv.\n\n\
                 Скачать portable (~20 MB) в\n{}\n\n\
                 «Нет» — без mpv (ограниченный режим)",
                install_dir.display()
            )
        };

        let ans = rfd::MessageDialog::new()
            .set_title("mpv")
            .set_description(msg)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();

        if ans == rfd::MessageDialogResult::No {
            return allow_skip;
        }
        if ans != rfd::MessageDialogResult::Yes {
            return false;
        }

        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("xdg-open")
                .arg("https://mpv.io/installation/")
                .spawn();
            return false;
        }

        #[cfg(not(target_os = "linux"))]
        match AudioPlayer::install_mpv() {
            Ok(_) => true,
            Err(e) => {
                rfd::MessageDialog::new()
                    .set_title("mpv")
                    .set_description(e)
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
                allow_skip
            }
        }
    }

    pub fn ytdlp_stream_url(track: &TrackInfo, format: YtDlpFormat) -> Result<String, String> {
        let ytdlp = Self::resolve_yt_dlp()?;
        let target = if track.url.starts_with("http://") || track.url.starts_with("https://") {
            track.url.clone()
        } else {
            format!("ytsearch1:{} - {}", track.artist.trim(), track.title.trim())
        };
        let format_arg = match format {
            YtDlpFormat::Mp3 => "bestaudio[ext=m4a]/bestaudio/best",
            YtDlpFormat::Mp4 => "best[height<=720][ext=mp4]/best[ext=mp4]/best",
        };
        let output = Self::run_command_with_timeout(
            {
                let mut cmd = Command::new(&ytdlp);
                Self::clear_proxy_env(&mut cmd);
                Self::append_ytdlp_network_args(&mut cmd);
                cmd.args(["--no-playlist", "-g", "-f", format_arg, &target]);
                cmd
            },
            Duration::from_secs(60),
        )?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let url = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|l| l.starts_with("http://") || l.starts_with("https://"))
            .unwrap_or("")
            .trim()
            .to_string();
        if url.is_empty() {
            Err("yt-dlp не вернул URL потока".into())
        } else {
            Ok(url)
        }
    }

    pub fn drivemusic_stream_url(track: &TrackInfo) -> Result<String, String> {
        let page = Self::drivemusic_page_url(track)?;
        let client = Self::drivemusic_client()?;
        let resp = client
            .get(&page)
            .header("User-Agent", BROWSER_USER_AGENT)
            .header("Referer", DRIVEMUSIC_BASE)
            .send()
            .map_err(|e| e.to_string())?;
        let body = resp.text().map_err(|e| e.to_string())?;
        let urls = Self::drivemusic_extract_mp3_urls(&body);
        urls.iter()
            .find(|u| u.contains("/dl/online/"))
            .cloned()
            .or_else(|| urls.into_iter().next())
            .ok_or_else(|| "На странице нет MP3 URL".to_string())
    }

    pub fn append_ytdlp_format_args(cmd: &mut Command, format: YtDlpFormat) {
        match format {
            YtDlpFormat::Mp3 => {
                cmd.args([
                    "-x",
                    "--audio-format",
                    "mp3",
                    "--audio-quality",
                    "0",
                    "--embed-thumbnail",
                ]);
            }
            YtDlpFormat::Mp4 => {
                cmd.args(["-f", "bv*+ba/b", "--merge-output-format", "mp4"]);
            }
        }
    }

    pub fn ytdlp_speed_args(cmd: &mut Command) {
        let has_aria2c = Command::new("which")
            .arg("aria2c")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if has_aria2c {
            cmd.args([
                "--downloader",
                "aria2c",
                "--downloader-args",
                "aria2c:-x 16 -s 16 -k 1M",
            ]);
        } else {
            cmd.arg("--concurrent-fragments").arg("8");
        }
    }

    pub fn clear_proxy_env(cmd: &mut Command) {
        for key in [
            "http_proxy",
            "https_proxy",
            "ftp_proxy",
            "all_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "FTP_PROXY",
            "ALL_PROXY",
        ] {
            cmd.env_remove(key);
        }
    }

    pub fn append_ytdlp_network_args(cmd: &mut Command) {
        cmd.env("YT_DLP_NO_UPDATE", "1");
        cmd.args([
            "--no-warnings",
            "--ignore-errors",
            "--socket-timeout",
            "12",
            "--retries",
            "1",
            "--extractor-retries",
            "1",
        ]);
    }

    pub fn run_command_with_timeout(
        mut cmd: Command,
        timeout: Duration,
    ) -> Result<CommandOutput, String> {
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Не удалось запустить процесс: {}", e))?;

        let stdout_handle = child.stdout.take().map(|out| {
            thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = BufReader::new(out).read_to_end(&mut buf);
                buf
            })
        });
        let stderr_handle = child.stderr.take().map(|err| {
            thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = BufReader::new(err).read_to_end(&mut buf);
                buf
            })
        });

        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = stdout_handle
                        .map(|h| h.join().unwrap_or_default())
                        .unwrap_or_default();
                    let stderr = stderr_handle
                        .map(|h| h.join().unwrap_or_default())
                        .unwrap_or_default();
                    return Ok(CommandOutput {
                        status,
                        stdout,
                        stderr,
                    });
                }
                Ok(None) => {
                    if started.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(format!(
                            "Превышено время ожидания ({} с). Проверьте интернет и доступ к YouTube.",
                            timeout.as_secs()
                        ));
                    }
                    thread::sleep(Duration::from_millis(80));
                }
                Err(e) => return Err(format!("Ошибка ожидания процесса: {}", e)),
            }
        }
    }

    pub fn kill_child_process(pid: u32) {
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
        }
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .status();
        }
    }

    pub fn is_download_cancelled(cancel: &AtomicBool) -> bool {
        cancel.load(Ordering::Relaxed)
    }

    pub fn set_download_stopped(
        status: &Arc<Mutex<DownloadStatus>>,
        cancel: &AtomicBool,
        failed_msg: Option<String>,
    ) {
        let mut s = status.lock().unwrap();
        *s = if Self::is_download_cancelled(cancel) {
            DownloadStatus::Cancelled
        } else if let Some(msg) = failed_msg {
            DownloadStatus::Failed(msg)
        } else {
            DownloadStatus::Cancelled
        };
    }

    pub fn cancel_download_task(&mut self, idx: usize) {
        let Some(task) = self.download_tasks.get(idx) else {
            return;
        };
        let active = matches!(
            *task.status.lock().unwrap(),
            DownloadStatus::Pending | DownloadStatus::Downloading { .. }
        );
        if !active {
            return;
        }

        task.cancel.store(true, Ordering::Relaxed);
        if let Some(pid) = *task.child_pid.lock().unwrap() {
            Self::kill_child_process(pid);
        }

        {
            let mut s = task.status.lock().unwrap();
            *s = DownloadStatus::Cancelled;
        }

        self.push_log_line(format!(
            "[{}] ⏹ Остановлено: {} — {}",
            Self::log_timestamp(),
            task.track.artist,
            task.track.title
        ));
    }

    pub fn cancel_all_downloads(&mut self) {
        let count = self.download_tasks.len();
        for i in 0..count {
            self.cancel_download_task(i);
        }
    }

    pub fn download_track_mp3party(
        track: TrackInfo,
        folder: PathBuf,
        status: Arc<Mutex<DownloadStatus>>,
        cancel: Arc<AtomicBool>,
        log_tx: mpsc::Sender<String>,
    ) {
        thread::spawn(move || {
            let fail = |status: &Arc<Mutex<DownloadStatus>>, msg: String| {
                if Self::is_download_cancelled(&cancel) {
                    Self::set_download_stopped(&status, &cancel, None);
                    Self::log_send(&log_tx, "⏹ MP3Party: скачивание остановлено");
                    return;
                }
                Self::log_send(&log_tx, format!("❌ MP3Party: {}", msg));
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Failed(msg);
            };

            let stop_if_cancelled =
                |status: &Arc<Mutex<DownloadStatus>>, filepath: &Path| -> bool {
                    if !Self::is_download_cancelled(&cancel) {
                        return false;
                    }
                    let _ = std::fs::remove_file(filepath);
                    Self::set_download_stopped(status, &cancel, None);
                    Self::log_send(&log_tx, "⏹ MP3Party: скачивание остановлено");
                    true
                };

            Self::log_send(
                &log_tx,
                format!("📥 MP3Party: {} — {}", track.artist, track.title),
            );

            let filename = format!("{} - {}.mp3", track.artist.trim(), track.title.trim())
                .replace(|c: char| "/\\:*?\"<>|".contains(c), "_");

            let filepath = folder.join(&filename);
            let track_page = format!("https://mp3party.net/music/{}", track.id);

            let _ = std::fs::create_dir_all(&folder);

            {
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Downloading {
                    progress: 0.0,
                    bytes: 0,
                    total: 0,
                };
            }

            let client = match Self::mp3party_client() {
                Ok(c) => c,
                Err(e) => {
                    fail(&status, e);
                    return;
                }
            };

            if stop_if_cancelled(&status, &filepath) {
                return;
            }

            let page_body = match client
                .get(&track_page)
                .header("User-Agent", BROWSER_USER_AGENT)
                .send()
            {
                Ok(r) if r.status().is_success() => match r.text() {
                    Ok(t) => t,
                    Err(e) => {
                        fail(&status, format!("Страница трека: {}", e));
                        return;
                    }
                },
                Ok(r) => {
                    fail(&status, format!("Страница трека: HTTP {}", r.status()));
                    return;
                }
                Err(e) => {
                    fail(&status, format!("Страница трека: {}", e));
                    return;
                }
            };

            let candidates = Self::mp3party_download_candidates(&page_body, &track);

            let mut last_err = String::from("нет доступных URL");
            let mut server_unavailable = false;
            for download_url in candidates {
                if stop_if_cancelled(&status, &filepath) {
                    return;
                }

                Self::log_send(&log_tx, format!("MP3Party: пробую {}", download_url));

                let resp = match Self::mp3party_request(&client, &download_url, &track_page).send()
                {
                    Ok(r) if r.status().is_success() => r,
                    Ok(r) => {
                        last_err = format!("HTTP {}", r.status());
                        continue;
                    }
                    Err(e) => {
                        last_err = e.to_string();
                        continue;
                    }
                };

                let total = resp.content_length().unwrap_or(0);
                let mut file = match std::fs::File::create(&filepath) {
                    Ok(f) => f,
                    Err(e) => {
                        fail(&status, format!("Файл: {}", e));
                        return;
                    }
                };

                let mut downloaded: u64 = 0;
                let mut buffer = [0u8; 8192];
                let mut reader = resp.take(1024 * 1024 * 100);
                let mut read_err = None;

                loop {
                    if stop_if_cancelled(&status, &filepath) {
                        return;
                    }
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Err(e) = file.write_all(&buffer[..n]) {
                                read_err = Some(format!("Запись: {}", e));
                                break;
                            }
                            downloaded += n as u64;
                            let progress = if total > 0 {
                                downloaded as f32 / total as f32
                            } else {
                                0.0
                            };
                            let mut s = status.lock().unwrap();
                            *s = DownloadStatus::Downloading {
                                progress: progress.min(1.0),
                                bytes: downloaded,
                                total,
                            };
                        }
                        Err(e) => {
                            read_err = Some(format!("Чтение: {}", e));
                            break;
                        }
                    }
                }
                drop(file);

                if let Some(e) = read_err {
                    let _ = std::fs::remove_file(&filepath);
                    last_err = e;
                    continue;
                }

                if downloaded < MIN_DOWNLOAD_BYTES {
                    let bad = std::fs::read(&filepath)
                        .map(|b| Self::is_mp3party_error_body(&b))
                        .unwrap_or(false);
                    let _ = std::fs::remove_file(&filepath);
                    last_err = if bad {
                        server_unavailable = true;
                        "сервер mp3party: failed to get file info".into()
                    } else {
                        format!("файл {} KB", downloaded.max(1) / 1024)
                    };
                    continue;
                }

                Self::log_send(&log_tx, format!("✅ MP3Party: {}", filepath.display()));
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Completed(filepath.to_string_lossy().to_string());
                return;
            }

            let hint = if server_unavailable {
                format!(
                    "Трек ID {} недоступен на CDN MP3Party — выберите другой результат поиска или источник YouTube.",
                    track.id
                )
            } else {
                format!("MP3Party: {last_err}")
            };
            if Self::is_download_cancelled(&cancel) {
                stop_if_cancelled(&status, &filepath);
                return;
            }
            fail(&status, format!("{hint} Страница: {track_page}"));
        });
    }

    pub fn download_track_drivemusic(
        track: TrackInfo,
        folder: PathBuf,
        status: Arc<Mutex<DownloadStatus>>,
        cancel: Arc<AtomicBool>,
        log_tx: mpsc::Sender<String>,
    ) {
        thread::spawn(move || {
            let fail = |status: &Arc<Mutex<DownloadStatus>>, msg: String| {
                if Self::is_download_cancelled(&cancel) {
                    Self::set_download_stopped(&status, &cancel, None);
                    Self::log_send(&log_tx, "⏹ DriveMusic: скачивание остановлено");
                    return;
                }
                Self::log_send(&log_tx, format!("❌ DriveMusic: {}", msg));
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Failed(msg);
            };

            let stop_if_cancelled =
                |status: &Arc<Mutex<DownloadStatus>>, filepath: &Path| -> bool {
                    if !Self::is_download_cancelled(&cancel) {
                        return false;
                    }
                    let _ = std::fs::remove_file(filepath);
                    Self::set_download_stopped(status, &cancel, None);
                    Self::log_send(&log_tx, "⏹ DriveMusic: скачивание остановлено");
                    true
                };

            Self::log_send(
                &log_tx,
                format!("📥 DriveMusic: {} — {}", track.artist, track.title),
            );

            let filename = format!(
                "{} - {}_{}.mp3",
                track.artist.trim(),
                track.title.trim(),
                track.id
            )
            .replace(|c: char| "/\\:*?\"<>|".contains(c), "_");

            let filepath = folder.join(&filename);
            let track_page = match Self::drivemusic_page_url(&track) {
                Ok(p) => p,
                Err(e) => {
                    fail(&status, e);
                    return;
                }
            };

            let _ = std::fs::create_dir_all(&folder);

            {
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Downloading {
                    progress: 0.0,
                    bytes: 0,
                    total: 0,
                };
            }

            let client = match Self::drivemusic_client() {
                Ok(c) => c,
                Err(e) => {
                    fail(&status, e);
                    return;
                }
            };

            if stop_if_cancelled(&status, &filepath) {
                return;
            }

            let page_body = match client
                .get(&track_page)
                .header("User-Agent", BROWSER_USER_AGENT)
                .header("Referer", DRIVEMUSIC_BASE)
                .send()
            {
                Ok(r) if r.status().is_success() => match r.text() {
                    Ok(t) => t,
                    Err(e) => {
                        fail(&status, format!("Страница трека: {}", e));
                        return;
                    }
                },
                Ok(r) => {
                    fail(&status, format!("Страница трека: HTTP {}", r.status()));
                    return;
                }
                Err(e) => {
                    fail(&status, format!("Страница трека: {}", e));
                    return;
                }
            };

            let candidates = Self::drivemusic_download_candidates(&page_body, &track);
            if candidates.is_empty() {
                fail(
                    &status,
                    format!(
                        "Нет ссылок на MP3 (временные URL могли устареть). Страница: {}",
                        track_page
                    ),
                );
                return;
            }

            let mut last_err = String::from("нет доступных URL");
            for download_url in candidates {
                if stop_if_cancelled(&status, &filepath) {
                    return;
                }

                Self::log_send(&log_tx, format!("DriveMusic: пробую {}", download_url));

                let resp = match client
                    .get(&download_url)
                    .header("User-Agent", BROWSER_USER_AGENT)
                    .header("Referer", &track_page)
                    .header("Origin", DRIVEMUSIC_BASE)
                    .header("Accept", "audio/mpeg,application/octet-stream,*/*;q=0.8")
                    .send()
                {
                    Ok(r) if r.status().is_success() => r,
                    Ok(r) => {
                        last_err = format!("HTTP {}", r.status());
                        continue;
                    }
                    Err(e) => {
                        last_err = e.to_string();
                        continue;
                    }
                };

                let total = resp.content_length().unwrap_or(0);
                let mut file = match std::fs::File::create(&filepath) {
                    Ok(f) => f,
                    Err(e) => {
                        fail(&status, format!("Файл: {}", e));
                        return;
                    }
                };

                let mut downloaded: u64 = 0;
                let mut buffer = [0u8; 8192];
                let mut reader = resp;
                let mut read_err = None;

                loop {
                    if stop_if_cancelled(&status, &filepath) {
                        return;
                    }
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Err(e) = file.write_all(&buffer[..n]) {
                                read_err = Some(format!("Запись: {}", e));
                                break;
                            }
                            downloaded += n as u64;
                            let progress = if total > 0 {
                                downloaded as f32 / total as f32
                            } else {
                                0.0
                            };
                            let mut s = status.lock().unwrap();
                            *s = DownloadStatus::Downloading {
                                progress: progress.min(1.0),
                                bytes: downloaded,
                                total,
                            };
                        }
                        Err(e) => {
                            read_err = Some(format!("Чтение: {}", e));
                            break;
                        }
                    }
                }
                drop(file);

                if let Some(e) = read_err {
                    let _ = std::fs::remove_file(&filepath);
                    last_err = e;
                    continue;
                }

                if downloaded < MIN_DOWNLOAD_BYTES {
                    let _ = std::fs::remove_file(&filepath);
                    last_err = format!(
                        "файл {} KB — ссылка могла устареть, откройте страницу заново",
                        downloaded.max(1) / 1024
                    );
                    continue;
                }

                Self::log_send(&log_tx, format!("✅ DriveMusic: {}", filepath.display()));
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Completed(filepath.to_string_lossy().to_string());
                return;
            }

            fail(
                &status,
                format!("DriveMusic: {last_err}. Страница: {track_page} (ссылки временные)"),
            );
        });
    }

    pub fn download_track_ytdlp(
        track: TrackInfo,
        folder: PathBuf,
        format: YtDlpFormat,
        status: Arc<Mutex<DownloadStatus>>,
        cancel: Arc<AtomicBool>,
        child_pid: Arc<Mutex<Option<u32>>>,
        log_tx: mpsc::Sender<String>,
    ) {
        thread::spawn(move || {
            let fail = |status: &Arc<Mutex<DownloadStatus>>, msg: String| {
                if Self::is_download_cancelled(&cancel) {
                    Self::set_download_stopped(&status, &cancel, None);
                    Self::log_send(&log_tx, "⏹ yt-dlp: скачивание остановлено");
                    return;
                }
                Self::log_send(&log_tx, format!("❌ yt-dlp: {}", msg));
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Failed(msg);
            };

            Self::log_send(
                &log_tx,
                format!(
                    "📥 yt-dlp {}: {} — {}",
                    format.label(),
                    track.artist,
                    track.title
                ),
            );

            let ytdlp = match Self::resolve_yt_dlp() {
                Ok(p) => p,
                Err(e) => {
                    fail(&status, e);
                    return;
                }
            };

            let _ = std::fs::create_dir_all(&folder);
            let archive = folder.join(format!(".yt-dlp-archive-{}", format.archive_ext()));
            let target = if track.url.starts_with("http://") || track.url.starts_with("https://") {
                track.url.clone()
            } else {
                let query = format!("{} - {}", track.artist.trim(), track.title.trim());
                format!("ytsearch1:{}", query)
            };
            let output_tpl = format!("{}/%(title)s.%(ext)s", folder.display());

            {
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Downloading {
                    progress: 0.0,
                    bytes: 0,
                    total: 0,
                };
            }

            let mut cmd = Command::new(&ytdlp);
            Self::clear_proxy_env(&mut cmd);
            cmd.args([
                "--force-ipv4",
                "--socket-timeout",
                "20",
                "--continue",
                "--download-archive",
                archive.to_str().unwrap_or(""),
                "--no-playlist",
                "--reject-title",
                "(?i)сборник",
                "--print",
                "after_move:AFTERMOVE:%(filepath)s",
                "--retries",
                "10",
                "--fragment-retries",
                "10",
                "--no-mtime",
                "--newline",
                "-o",
                &output_tpl,
            ]);
            Self::append_ytdlp_format_args(&mut cmd, format);
            cmd.arg(&target);
            Self::ytdlp_speed_args(&mut cmd);
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    fail(&status, format!("Не удалось запустить yt-dlp: {}", e));
                    return;
                }
            };

            {
                let pid = child.id();
                *child_pid.lock().unwrap() = Some(pid);
            }

            let stdout = match child.stdout.take() {
                Some(s) => s,
                None => {
                    fail(&status, "yt-dlp: нет stdout".into());
                    return;
                }
            };

            let stderr = child.stderr.take();
            let log_tx_stderr = log_tx.clone();
            let stderr_handle = stderr.map(|err| {
                thread::spawn(move || {
                    for line in BufReader::new(err).lines() {
                        if let Ok(l) = line {
                            let t = l.trim();
                            if !t.is_empty() {
                                Self::log_send(&log_tx_stderr, format!("yt-dlp> {}", t));
                            }
                        }
                    }
                })
            });

            let mut completed_path: Option<String> = None;
            let mut last_pct: f32 = 0.0;

            for line in BufReader::new(stdout).lines() {
                if Self::is_download_cancelled(&cancel) {
                    let _ = child.kill();
                    Self::kill_child_process(child.id());
                    break;
                }

                let line = match line {
                    Ok(l) => l,
                    Err(_) => continue,
                };

                if let Some(path) = line.split("AFTERMOVE:").nth(1) {
                    let path = path.trim();
                    if !path.is_empty() {
                        completed_path = Some(path.to_string());
                    }
                    continue;
                }

                if let Some(caps) = RE_YTDLP_PERCENT.captures(&line) {
                    if let Ok(pct) = caps[1].parse::<f32>() {
                        let progress = (pct / 100.0).min(0.99);
                        if progress >= last_pct {
                            last_pct = progress;
                            let mut s = status.lock().unwrap();
                            *s = DownloadStatus::Downloading {
                                progress,
                                bytes: 0,
                                total: 0,
                            };
                        }
                    }
                }
            }

            if Self::is_download_cancelled(&cancel) {
                let _ = child.kill();
                Self::kill_child_process(child.id());
                *child_pid.lock().unwrap() = None;
                Self::set_download_stopped(&status, &cancel, None);
                Self::log_send(&log_tx, "⏹ yt-dlp: скачивание остановлено");
                if let Some(h) = stderr_handle {
                    let _ = h.join();
                }
                return;
            }

            let exit_code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
            *child_pid.lock().unwrap() = None;
            if let Some(h) = stderr_handle {
                let _ = h.join();
            }

            if let Some(path) = completed_path {
                Self::log_send(&log_tx, format!("✅ yt-dlp: {}", path));
                let mut s = status.lock().unwrap();
                *s = DownloadStatus::Completed(path);
                return;
            }

            if exit_code == 0 {
                fail(
                    &status,
                    "yt-dlp завершился без файла (возможно, трек уже в архиве)".into(),
                );
            } else {
                fail(&status, format!("yt-dlp завершился с кодом {}", exit_code));
            }
        });
    }

    pub fn open_external_url(url: &str) {
        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("xdg-open").arg(url).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("open").arg(url).spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
        }
    }
}
