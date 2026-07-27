use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::batch;
use crate::downloader::sanitize_filename;
use crate::types::*;
use crate::LinkParserApp;

impl LinkParserApp {
    pub fn log_timestamp() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!(
            "{:02}:{:02}:{:02}",
            (secs / 3600) % 24,
            (secs / 60) % 60,
            secs % 60
        )
    }

    pub fn log_send(tx: &mpsc::Sender<String>, msg: impl Into<String>) {
        let line = format!("[{}] {}", Self::log_timestamp(), msg.into());
        let _ = tx.send(line);
    }

    pub fn push_log_line(&mut self, msg: impl Into<String>) {
        self.log_lines.push(msg.into());
        if self.log_lines.len() > MAX_LOG_LINES {
            let drop = self.log_lines.len() - MAX_LOG_LINES;
            self.log_lines.drain(0..drop);
        }
    }

    pub fn drain_log_messages(&mut self) {
        while let Ok(line) = self.log_rx.try_recv() {
            self.push_log_line(line);
        }
    }

    pub fn theme(&self) -> crate::theme::AppTheme {
        if self.is_dark_mode {
            crate::theme::AppTheme::dark()
        } else {
            crate::theme::AppTheme::light()
        }
    }

    pub fn filtered_track_indices(&self) -> Vec<usize> {
        let q = self.result_filter.trim().to_lowercase();
        self.tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                if q.is_empty() {
                    return true;
                }
                t.title.to_lowercase().contains(&q) || t.artist.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn begin_loading(&mut self) {
        self.loading = true;
        self.loading_started = Some(Instant::now());
    }

    pub fn cancel_loading(&mut self) {
        self.loading = false;
        self.loading_started = None;
        self.rx = None;
        self.status = "⏹ Операция отменена.".into();
        self.push_log_line(format!(
            "[{}] ⏹ Операция отменена пользователем.",
            Self::log_timestamp()
        ));
    }

    pub fn check_loading_watchdog(&mut self) {
        if !self.loading {
            return;
        }
        let Some(started) = self.loading_started else {
            return;
        };
        if started.elapsed() < Duration::from_secs(LOADING_WATCHDOG_SECS) {
            return;
        }
        self.error_count += 1;
        self.last_error =
            Some("Слишком долгое ожидание — операция прервана. Проверьте интернет.".into());
        self.processed = self.total_urls;
        self.finish_loading();
    }

    pub fn finish_loading(&mut self) {
        self.loading = false;
        self.loading_started = None;
        self.rx = None;

        if self.tracks.is_empty() && self.error_count > 0 {
            let msg = format!("❌ {}", self.last_error.as_deref().unwrap_or("Ошибка"));
            self.push_log_line(format!("[{}] {}", Self::log_timestamp(), msg));
            self.status = msg;
            return;
        }

        let word = match self.tracks.len() {
            1 => "трек",
            2..=4 => "трека",
            _ => "треков",
        };

        if self.error_count > 0 {
            let msg = format!(
                "✅ {} {} (ошибок: {})",
                self.tracks.len(),
                word,
                self.error_count
            );
            self.push_log_line(format!("[{}] {}", Self::log_timestamp(), msg));
            self.status = msg;
        } else {
            let msg = format!("✅ Готово — {} {}", self.tracks.len(), word);
            self.push_log_line(format!("[{}] {}", Self::log_timestamp(), msg));
            self.status = msg;
        }
    }

    pub fn format_duration(secs: f64) -> String {
        let s = secs.max(0.0) as u64;
        format!("{}:{:02}", s / 60, s % 60)
    }

    pub fn format_bytes(b: u64) -> String {
        if b > 1024 * 1024 {
            format!("{:.1} MB", b as f64 / 1024.0 / 1024.0)
        } else if b > 1024 {
            format!("{:.1} KB", b as f64 / 1024.0)
        } else {
            format!("{} B", b)
        }
    }

    pub fn task_source_label(
        source: DownloadSource,
        ytdlp_format: Option<YtDlpFormat>,
    ) -> &'static str {
        match (source, ytdlp_format) {
            (DownloadSource::YtDlp, Some(YtDlpFormat::Mp3)) => "YouTube MP3",
            (DownloadSource::YtDlp, Some(YtDlpFormat::Mp4)) => "YouTube MP4",
            (DownloadSource::YtDlp, None) => "YouTube",
            (DownloadSource::Mp3Party, _) => "MP3Party",
            (DownloadSource::DriveMusic, _) => "DriveMusic",
            (DownloadSource::PesniMe, _) => "Pesni.me",
        }
    }

    pub fn refresh_library(&mut self) {
        self.library_files = crate::library::list_downloads(&self.downloads_folder);
    }

    pub fn start_stream(&mut self, idx: usize) {
        use std::sync::mpsc;
        use std::thread;

        if idx >= self.tracks.len() {
            return;
        }
        let track = self.tracks[idx].clone();
        let source = self.download_source;
        let fmt = self.ytdlp_format;
        if source == DownloadSource::YtDlp {
            if let Err(err) = Self::require_yt_dlp_ui() {
                self.status = format!("❌ {}", err);
                return;
            }
        }
        if !Self::offer_mpv_ui(true) {
            return;
        }
        self.status = format!("🎧 Поток: {} — {}", track.artist, track.title);
        self.loading = true;

        let (tx, rx) = mpsc::channel();
        self.stream_rx = Some(rx);
        thread::spawn(move || {
            let result = (|| {
                let url = match source {
                    DownloadSource::Mp3Party => {
                        if track.url.starts_with("http") {
                            track.url.clone()
                        } else {
                            format!("https://dl2.mp3party.net/online/{}.mp3", track.id)
                        }
                    }
                    DownloadSource::DriveMusic => LinkParserApp::drivemusic_stream_url(&track)?,
                    DownloadSource::PesniMe => LinkParserApp::pesnime_stream_url(&track)?,
                    DownloadSource::YtDlp => LinkParserApp::ytdlp_stream_url(&track, fmt)?,
                };
                let title = format!("{} — {}", track.artist, track.title);
                let sub = format!("Стрим {}", source.label());
                let is_video = source == DownloadSource::YtDlp && fmt == YtDlpFormat::Mp4;
                Ok((url, title, sub, is_video))
            })();
            let _ = tx.send(result);
        });
    }

    pub fn start_parsing(&mut self) {
        let text = self.input_text.trim().to_string();
        if text.is_empty() {
            self.status = "⚠️ Введите ссылки для парсинга.".into();
            return;
        }

        let lines: Vec<&str> = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        if lines.is_empty() {
            self.status = "⚠️ Нет валидных ссылок.".into();
            return;
        }

        let mut direct_links: Vec<String> = Vec::new();
        let mut ids: Vec<(String, String)> = Vec::new();

        for line in &lines {
            let lower = line.to_lowercase();
            if lower.starts_with("http://") || lower.starts_with("https://") {
                if lower.ends_with(".mp3")
                    || lower.ends_with(".mp4")
                    || lower.ends_with(".m4a")
                    || lower.ends_with(".ogg")
                    || lower.ends_with(".flac")
                    || lower.ends_with(".wav")
                    || lower.contains("/download/")
                    || lower.contains("/dl/online/")
                    || lower.contains("/dl/download/")
                    || lower.contains("pl.pesni.me")
                {
                    direct_links.push(line.to_string());
                    continue;
                }
            }
            if let Some(id) = Self::extract_id(line) {
                ids.push((line.to_string(), id));
            } else if RE_DIGITS.is_match(line.trim()) {
                ids.push((line.to_string(), line.trim().to_string()));
            } else {
                self.status = format!("⚠️ Не удалось распознать: {}", line);
                return;
            }
        }

        let total = direct_links.len() + ids.len();
        if total == 0 {
            self.status = "⚠️ Не найдено ссылок.".into();
            return;
        }

        self.tracks.clear();
        self.result_filter.clear();
        self.output_mode = OutputMode::UrlParsing;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.total_urls = total;
        self.processed = 0;
        self.error_count = 0;
        self.last_error = None;
        self.begin_loading();
        self.status = format!("⏳ Импорт {} ссылок...", total);

        let log_tx = self.log_tx.clone();
        thread::spawn(move || {
            for url in direct_links {
                let lower = url.to_lowercase();
                let resolved = if lower.contains("mp3party.net/download/") {
                    let id = url
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .trim_end_matches(".mp3")
                        .trim_end_matches(".mp4");
                    if !id.is_empty() {
                        Self::fetch_track_info(id).ok().map(|mut t| {
                            t.url = url.clone();
                            t
                        })
                    } else {
                        None
                    }
                } else if lower.contains("pl.pesni.me") || lower.contains("dw.pesni.me") {
                    let id = url
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .split('.').next()
                        .unwrap_or("");
                    if !id.is_empty() {
                        Self::fetch_track_info_pesnime(id).ok().map(|mut t| {
                            t.url = url.clone();
                            t
                        })
                    } else {
                        None
                    }
                } else if lower.contains("drivemusic.me/dl/") {
                    let raw = url
                        .rsplit('/')
                        .next()
                        .unwrap_or("track")
                        .split('.').next()
                        .unwrap_or("track");
                    let filename = sanitize_filename(raw);
                    Some(TrackInfo {
                        id: String::new(),
                        artist: String::new(),
                        title: filename,
                        url: url.clone(),
                    })
                } else {
                    None
                };

                match resolved {
                    Some(track) => {
                        let _ = tx.send(ParseResult::Success(track));
                    }
                    None => {
                        let raw = url
                            .rsplit('/')
                            .next()
                            .unwrap_or("track")
                            .split('.').next()
                            .unwrap_or("track");
                        let filename = sanitize_filename(raw);
                        let _ = tx.send(ParseResult::Success(TrackInfo {
                            id: String::new(),
                            artist: String::new(),
                            title: filename,
                            url,
                        }));
                    }
                }
            }

            for (orig_url, id) in ids {
                let result = if orig_url.contains("pesni.me") {
                    Self::fetch_track_info_pesnime(&id)
                } else {
                    Self::fetch_track_info(&id)
                };
                match result {
                    Ok(track) => {
                        let _ = tx.send(ParseResult::Success(track));
                    }
                    Err(err) => {
                        Self::log_send(&log_tx, format!("❌ Парсинг {}: {}", orig_url, err));
                        let _ = tx.send(ParseResult::Error(orig_url, err));
                    }
                }
            }
        });
    }

    pub fn start_search(&mut self) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            self.status = "⚠️ Введите поисковый запрос.".into();
            return;
        }

        let source = self.download_source;
        if source == DownloadSource::YtDlp {
            if let Err(err) = Self::require_yt_dlp_ui() {
                self.status = format!("❌ {}", err);
                self.push_log_line(format!("[{}] ❌ yt-dlp: {}", Self::log_timestamp(), err));
                return;
            }
            self.push_log_line(format!("[{}] ✅ yt-dlp готов", Self::log_timestamp()));
        }

        self.tracks.clear();
        self.result_filter.clear();
        self.output_mode = OutputMode::Search;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.total_urls = 1;
        self.processed = 0;
        self.error_count = 0;
        self.last_error = None;
        self.begin_loading();
        self.status = format!("⏳ Поиск [{}] «{}»...", source.label(), query);
        self.push_log_line(format!(
            "[{}] ⏳ Поиск [{}] «{}»...",
            Self::log_timestamp(),
            source.label(),
            query
        ));

        let log_tx = self.log_tx.clone();
        thread::spawn(move || {
            let result = match source {
                DownloadSource::Mp3Party => Self::search_tracks(&query),
                DownloadSource::DriveMusic => Self::search_tracks_drivemusic(&query),
                DownloadSource::PesniMe => Self::search_tracks_pesnime(&query),
                DownloadSource::YtDlp => Self::search_tracks_ytdlp(&query),
            };
            match result {
                Ok(results) => {
                    let n = results.len();
                    Self::log_send(&log_tx, format!("✅ Поиск: найдено {} треков", n));
                    let _ = tx.send(ParseResult::SearchResults(results));
                }
                Err(err) => {
                    Self::log_send(&log_tx, format!("❌ Поиск: {}", err));
                    let _ = tx.send(ParseResult::Error(query, err));
                }
            }
        });
    }

    pub fn start_batch_search(&mut self) {
        let queries = batch::parse_batch_queries(&self.batch_input);
        if queries.is_empty() {
            self.status = "⚠️ Введите хотя бы один запрос в список.".into();
            return;
        }
        let source = self.download_source;
        if source == DownloadSource::YtDlp {
            if let Err(err) = Self::require_yt_dlp_ui() {
                self.status = format!("❌ {}", err);
                return;
            }
        }

        self.tracks.clear();
        self.result_filter.clear();
        self.output_mode = OutputMode::Search;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.total_urls = queries.len();
        self.processed = 0;
        self.error_count = 0;
        self.last_error = None;
        self.begin_loading();
        self.status = format!(
            "⏳ Пакетный поиск [{}]: {} запрос(ов)…",
            source.label(),
            queries.len()
        );
        self.push_log_line(format!(
            "[{}] ⏳ Пакетный поиск [{}]: {} запрос(ов)",
            Self::log_timestamp(),
            source.label(),
            queries.len()
        ));

        let log_tx = self.log_tx.clone();
        thread::spawn(move || {
            let total = queries.len();
            for (i, q) in queries.into_iter().enumerate() {
                let num = i + 1;
                Self::log_send(
                    &log_tx,
                    format!("[{}/{}] 🔎 {}", num, total, q.search_text()),
                );
                if let Some(url) = q.url {
                    Self::log_send(
                        &log_tx,
                        format!(
                            "⚠️ URL '{}' пропущен — вставьте ссылку в основное поле «Парсить ссылки»",
                            url
                        ),
                    );
                    let _ = tx.send(ParseResult::Error(
                        url,
                        "URL в batch-режиме не поддерживается".into(),
                    ));
                    continue;
                }
                let q_text = q.search_text();
                let result = match source {
                    DownloadSource::Mp3Party => Self::search_tracks(&q_text),
                    DownloadSource::DriveMusic => Self::search_tracks_drivemusic(&q_text),
                    DownloadSource::PesniMe => Self::search_tracks_pesnime(&q_text),
                    DownloadSource::YtDlp => Self::search_tracks_ytdlp(&q_text),
                };
                match result {
                    Ok(mut tracks) => {
                        Self::log_send(
                            &log_tx,
                            format!("  ✓ {}/{}: найдено {}", num, total, tracks.len()),
                        );
                        let _ = tx.send(ParseResult::SearchResults(std::mem::take(&mut tracks)));
                    }
                    Err(err) => {
                        Self::log_send(&log_tx, format!("  ✗ {}/{}: {}", num, total, err));
                        let _ = tx.send(ParseResult::Error(q_text, err));
                    }
                }
            }
        });
    }

    pub fn start_download(&mut self, track_idx: usize) {
        let source = self.download_source;
        let ytdlp_format = self.ytdlp_format;
        if source == DownloadSource::YtDlp {
            if let Err(err) = Self::require_yt_dlp_ui() {
                self.status = format!("❌ {}", err);
                return;
            }
        }
        if let Some(track) = self.tracks.get(track_idx) {
            for task in &self.download_tasks {
                let s = task.status.lock().unwrap();
                let same_ytdlp_fmt = task.ytdlp_format == Some(ytdlp_format);
                if matches!(
                    *s,
                    DownloadStatus::Downloading { .. } | DownloadStatus::Pending
                ) && task.track.id == track.id
                    && task.source == source
                    && (source != DownloadSource::YtDlp || same_ytdlp_fmt)
                {
                    self.status =
                        format!("⚠️ «{} — {}» уже скачивается", track.artist, track.title);
                    return;
                }
            }

            let folder = self.downloads_folder.clone();
            let status = Arc::new(Mutex::new(DownloadStatus::Pending));
            let cancel = Arc::new(AtomicBool::new(false));
            let child_pid = Arc::new(Mutex::new(None));
            let task = DownloadTask {
                _id: self.next_download_id,
                track: track.clone(),
                source,
                ytdlp_format: if source == DownloadSource::YtDlp {
                    Some(ytdlp_format)
                } else {
                    None
                },
                status: status.clone(),
                cancel: cancel.clone(),
                child_pid: child_pid.clone(),
            };
            self.next_download_id += 1;
            self.download_tasks.push(task);

            let log_tx = self.log_tx.clone();
            match source {
                DownloadSource::Mp3Party => {
                    Self::download_track_mp3party(track.clone(), folder, status, cancel, log_tx);
                }
                DownloadSource::DriveMusic => {
                    Self::download_track_drivemusic(track.clone(), folder, status, cancel, log_tx);
                }
                DownloadSource::PesniMe => {
                    Self::download_track_pesnime(track.clone(), folder, status, cancel, log_tx);
                }
                DownloadSource::YtDlp => {
                    Self::download_track_ytdlp(
                        track.clone(),
                        folder,
                        ytdlp_format,
                        status,
                        cancel,
                        child_pid,
                        log_tx,
                    );
                }
            }
            self.show_downloads = true;
            let fmt_note = if source == DownloadSource::YtDlp {
                format!(" {}", ytdlp_format.label())
            } else {
                String::new()
            };
            self.status = format!(
                "📥 [{}]{} {} — {}",
                source.label(),
                fmt_note,
                track.artist,
                track.title
            );
        }
    }

    pub fn open_downloads_folder(&mut self) {
        let folder = self.downloads_folder.clone();
        if let Err(e) = std::fs::create_dir_all(&folder) {
            self.status = format!("❌ Не удалось создать папку: {}", e);
            return;
        }
        match Command::new("xdg-open").arg(&folder).spawn() {
            Ok(_) => self.status = format!("📂 Открыта папка: {}", folder.display()),
            Err(e) => self.status = format!("❌ Не удалось открыть папку: {}", e),
        }
    }

    pub fn has_active_downloads(&self) -> bool {
        self.download_tasks.iter().any(|t| {
            matches!(
                *t.status.lock().unwrap(),
                DownloadStatus::Pending | DownloadStatus::Downloading { .. }
            )
        })
    }
}
