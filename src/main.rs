//! GUI: на Windows без отдельного консольного окна (подсистема windows).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod library;
mod player;

use eframe::egui::{self, Color32, Frame, Margin, Rounding, Stroke, Vec2};
use library::{list_downloads, LocalMedia};
use player::{open_folder_in_file_manager, AudioPlayer};
use regex::Regex;
use scraper::{Html, Selector};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════
//  Статические Regex (компилируются один раз)
// ═══════════════════════════════════════════

static RE_ID_EXTRACT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:/download/|/music/)(\d+)|(?:^|/)(\d+)/?$").unwrap());
static RE_DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());
static RE_YTDLP_PERCENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{1,3})(?:\.\d+)?%").unwrap());
static RE_DRIVEMUSIC_MP3: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https://[a-z0-9.-]*drivemusic\.me/dl/[^"\s<>]+\.mp3"#).unwrap());
static RE_DRIVEMUSIC_SEARCH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)href="(/[a-z0-9_]+/(\d+)-[^"]+\.html)"[^>]*class="popular-play-author"[^>]*>([^<]*)</a>.*?popular-play-composition.*?>(?:<a[^>]*>)?([^<]*)"#,
    )
    .unwrap()
});

const DRIVEMUSIC_BASE: &str = "https://ru.drivemusic.me";

/// Минимальный размер MP3; меньше — считаем ошибкой (HTML/редирект) и удаляем
const MIN_DOWNLOAD_BYTES: u64 = 50 * 1024;

const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const MAX_LOG_LINES: usize = 2000;
const YTDLP_SEARCH_TIMEOUT_SECS: u64 = 45;
const LOADING_WATCHDOG_SECS: u64 = 90;

fn default_downloads_folder() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    home.join("mp3_downloader_gui").join("downloads")
}

// ═══════════════════════════════════════════
//  Типы данных
// ═══════════════════════════════════════════

#[derive(Clone, Debug)]
struct TrackInfo {
    id: String,
    artist: String,
    title: String,
    url: String,
}

#[derive(Clone, Debug)]
enum DownloadStatus {
    Pending,
    Downloading {
        progress: f32,
        bytes: u64,
        total: u64,
    },
    Completed(String),
    Failed(String),
    Cancelled,
}

struct DownloadTask {
    _id: usize,
    track: TrackInfo,
    source: DownloadSource,
    ytdlp_format: Option<YtDlpFormat>,
    status: Arc<Mutex<DownloadStatus>>,
    cancel: Arc<AtomicBool>,
    child_pid: Arc<Mutex<Option<u32>>>,
}

enum ParseResult {
    Success(TrackInfo),
    SearchResults(Vec<TrackInfo>),
    Error(String, String),
}

#[derive(Clone, Debug, PartialEq)]
enum OutputMode {
    UrlParsing,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainTab {
    Search,
    Library,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DownloadSource {
    Mp3Party,
    DriveMusic,
    YtDlp,
}

impl DownloadSource {
    fn label(self) -> &'static str {
        match self {
            DownloadSource::Mp3Party => "MP3Party",
            DownloadSource::DriveMusic => "DriveMusic",
            DownloadSource::YtDlp => "YouTube (yt-dlp)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum YtDlpFormat {
    Mp3,
    Mp4,
}

impl YtDlpFormat {
    fn label(self) -> &'static str {
        match self {
            YtDlpFormat::Mp3 => "MP3 (аудио)",
            YtDlpFormat::Mp4 => "MP4 (видео)",
        }
    }

    fn archive_ext(self) -> &'static str {
        match self {
            YtDlpFormat::Mp3 => "mp3",
            YtDlpFormat::Mp4 => "mp4",
        }
    }
}

// ═══════════════════════════════════════════
//  Тема оформления
// ═══════════════════════════════════════════

#[derive(Clone, Copy)]
struct AppTheme {
    window_bg: Color32,
    card_bg: Color32,
    card_border: Color32,
    header_bg: Color32,
    header_btn_bg: Color32,
    status_bg: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    text_muted: Color32,
    text_on_header: Color32,
    accent: Color32,
    btn_primary: Color32,
    btn_success: Color32,
    btn_neutral: Color32,
    success: Color32,
    warning: Color32,
    error: Color32,
    link: Color32,
    stripe: Color32,
    progress: Color32,
    separator: Color32,
}

impl AppTheme {
    fn dark() -> Self {
        Self {
            window_bg: Color32::from_rgb(22, 24, 32),
            card_bg: Color32::from_rgb(34, 37, 50),
            card_border: Color32::from_rgb(58, 62, 82),
            header_bg: Color32::from_rgb(36, 52, 88),
            header_btn_bg: Color32::from_rgb(52, 72, 118),
            status_bg: Color32::from_rgb(28, 30, 42),
            text_primary: Color32::from_rgb(232, 235, 248),
            text_secondary: Color32::from_rgb(175, 182, 210),
            text_muted: Color32::from_rgb(120, 128, 155),
            text_on_header: Color32::WHITE,
            accent: Color32::from_rgb(110, 165, 255),
            btn_primary: Color32::from_rgb(58, 118, 210),
            btn_success: Color32::from_rgb(42, 145, 95),
            btn_neutral: Color32::from_rgb(55, 60, 78),
            success: Color32::from_rgb(80, 210, 130),
            warning: Color32::from_rgb(240, 190, 70),
            error: Color32::from_rgb(240, 95, 95),
            link: Color32::from_rgb(130, 185, 255),
            stripe: Color32::from_rgb(40, 43, 58),
            progress: Color32::from_rgb(70, 140, 230),
            separator: Color32::from_rgb(50, 54, 72),
        }
    }

    fn light() -> Self {
        Self {
            window_bg: Color32::from_rgb(242, 244, 249),
            card_bg: Color32::WHITE,
            card_border: Color32::from_rgb(210, 216, 228),
            header_bg: Color32::from_rgb(32, 68, 128),
            header_btn_bg: Color32::from_rgb(48, 88, 155),
            status_bg: Color32::from_rgb(232, 236, 244),
            text_primary: Color32::from_rgb(28, 32, 48),
            text_secondary: Color32::from_rgb(70, 78, 100),
            text_muted: Color32::from_rgb(130, 138, 158),
            text_on_header: Color32::WHITE,
            accent: Color32::from_rgb(42, 88, 168),
            btn_primary: Color32::from_rgb(48, 108, 195),
            btn_success: Color32::from_rgb(38, 140, 88),
            btn_neutral: Color32::from_rgb(220, 224, 234),
            success: Color32::from_rgb(28, 150, 75),
            warning: Color32::from_rgb(190, 130, 20),
            error: Color32::from_rgb(200, 55, 55),
            link: Color32::from_rgb(35, 95, 185),
            stripe: Color32::from_rgb(248, 249, 252),
            progress: Color32::from_rgb(50, 120, 210),
            separator: Color32::from_rgb(220, 224, 232),
        }
    }

    fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.window_bg.r() < 128 {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        visuals.window_fill = self.window_bg;
        visuals.panel_fill = self.window_bg;
        visuals.extreme_bg_color = self.card_bg;
        visuals.faint_bg_color = self.stripe;
        visuals.widgets.noninteractive.bg_fill = self.card_bg;
        visuals.widgets.inactive.bg_fill = self.card_bg;
        visuals.widgets.hovered.bg_fill = self.header_btn_bg;
        visuals.widgets.active.bg_fill = self.btn_primary;
        visuals.widgets.open.bg_fill = self.btn_primary;
        visuals.widgets.noninteractive.fg_stroke.color = self.text_primary;
        visuals.widgets.inactive.fg_stroke.color = self.text_primary;
        visuals.widgets.hovered.fg_stroke.color = self.text_on_header;
        visuals.widgets.active.fg_stroke.color = self.text_on_header;
        visuals.override_text_color = Some(self.text_primary);
        visuals.hyperlink_color = self.link;
        visuals.selection.bg_fill = self.accent.gamma_multiply(0.35);
        visuals.selection.stroke.color = self.accent;
        visuals.widgets.noninteractive.weak_bg_fill = self.status_bg;
        visuals.widgets.inactive.weak_bg_fill = self.status_bg;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.separator);
        visuals.window_stroke.color = self.card_border;
        visuals.window_shadow = egui::epaint::Shadow::NONE;

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(14.0, 7.0);
        style.spacing.indent = 18.0;
        style.visuals.widgets.inactive.rounding = Rounding::same(6.0);
        style.visuals.widgets.active.rounding = Rounding::same(6.0);
        style.visuals.widgets.hovered.rounding = Rounding::same(6.0);
        ctx.set_style(style);
    }

    fn card(&self) -> Frame {
        Frame {
            fill: self.card_bg,
            rounding: Rounding::same(10.0),
            stroke: Stroke::new(1.0, self.card_border),
            inner_margin: Margin::symmetric(14.0, 12.0),
            ..Default::default()
        }
    }

    fn status_bar(&self) -> Frame {
        Frame {
            fill: self.status_bg,
            rounding: Rounding::same(8.0),
            stroke: Stroke::new(1.0, self.card_border),
            inner_margin: Margin::symmetric(12.0, 8.0),
            ..Default::default()
        }
    }

    fn header_button<'a>(&self, label: &'a str) -> egui::Button<'a> {
        egui::Button::new(egui::RichText::new(label).color(self.text_on_header))
            .fill(self.header_btn_bg)
            .rounding(Rounding::same(6.0))
    }

    fn primary_button<'a>(&self, label: &'a str) -> egui::Button<'a> {
        egui::Button::new(egui::RichText::new(label).color(Color32::WHITE))
            .fill(self.btn_primary)
            .rounding(Rounding::same(6.0))
    }

    fn success_button<'a>(&self, label: &'a str) -> egui::Button<'a> {
        egui::Button::new(egui::RichText::new(label).color(Color32::WHITE))
            .fill(self.btn_success)
            .rounding(Rounding::same(6.0))
    }

    fn neutral_button<'a>(&self, label: &'a str) -> egui::Button<'a> {
        let text = if self.window_bg.r() < 128 {
            self.text_primary
        } else {
            self.text_secondary
        };
        egui::Button::new(egui::RichText::new(label).color(text))
            .fill(self.btn_neutral)
            .rounding(Rounding::same(6.0))
    }

    fn section_title(&self, text: &str) -> egui::RichText {
        egui::RichText::new(text)
            .size(15.0)
            .strong()
            .color(self.text_primary)
    }

    fn status_color(&self, status: &str, loading: bool) -> Color32 {
        if loading {
            self.warning
        } else if status.starts_with('✅') || status.starts_with("📋") {
            self.success
        } else if status.starts_with('❌') || status.starts_with('⚠') {
            self.error
        } else {
            self.text_secondary
        }
    }
}

struct CommandOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

// ═══════════════════════════════════════════
//  Приложение
// ═══════════════════════════════════════════

struct LinkParserApp {
    input_text: String,
    search_query: String,
    result_filter: String,
    tracks: Vec<TrackInfo>,
    status: String,
    loading: bool,
    loading_started: Option<Instant>,
    rx: Option<mpsc::Receiver<ParseResult>>,
    total_urls: usize,
    processed: usize,
    error_count: usize,
    last_error: Option<String>,
    // Скачивание
    download_tasks: Vec<DownloadTask>,
    downloads_folder: PathBuf,
    next_download_id: usize,
    show_downloads: bool,
    show_logs: bool,
    log_lines: Vec<String>,
    log_tx: mpsc::Sender<String>,
    log_rx: mpsc::Receiver<String>,
    output_mode: OutputMode,
    download_source: DownloadSource,
    ytdlp_format: YtDlpFormat,
    is_dark_mode: bool,
    main_tab: MainTab,
    library_files: Vec<LocalMedia>,
    player: AudioPlayer,
    player_seek_request: Option<f64>,
    stream_rx: Option<mpsc::Receiver<Result<(String, String, String, bool), String>>>,
}

impl Default for LinkParserApp {
    fn default() -> Self {
        let (log_tx, log_rx) = mpsc::channel();
        Self {
            input_text: String::new(),
            search_query: String::new(),
            result_filter: String::new(),
            tracks: Vec::new(),
            status: "✅ Готов к работе.".into(),
            loading: false,
            loading_started: None,
            rx: None,
            total_urls: 0,
            processed: 0,
            error_count: 0,
            last_error: None,
            download_tasks: Vec::new(),
            downloads_folder: default_downloads_folder(),
            next_download_id: 0,
            show_downloads: false,
            show_logs: false,
            log_lines: vec!["✅ Приложение запущено.".to_string()],
            log_tx,
            log_rx,
            output_mode: OutputMode::UrlParsing,
            download_source: DownloadSource::Mp3Party,
            ytdlp_format: YtDlpFormat::Mp3,
            is_dark_mode: true,
            main_tab: MainTab::Search,
            library_files: Vec::new(),
            player: AudioPlayer::default(),
            player_seek_request: None,
            stream_rx: None,
        }
    }
}

// ═══════════════════════════════════════════
//  Методы
// ═══════════════════════════════════════════

impl LinkParserApp {
    fn log_timestamp() -> String {
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

    fn log_send(tx: &mpsc::Sender<String>, msg: impl Into<String>) {
        let line = format!("[{}] {}", Self::log_timestamp(), msg.into());
        let _ = tx.send(line);
    }

    fn push_log_line(&mut self, msg: impl Into<String>) {
        self.log_lines.push(msg.into());
        if self.log_lines.len() > MAX_LOG_LINES {
            let drop = self.log_lines.len() - MAX_LOG_LINES;
            self.log_lines.drain(0..drop);
        }
    }

    fn drain_log_messages(&mut self) {
        while let Ok(line) = self.log_rx.try_recv() {
            self.push_log_line(line);
        }
    }

    fn theme(&self) -> AppTheme {
        if self.is_dark_mode {
            AppTheme::dark()
        } else {
            AppTheme::light()
        }
    }

    /// Извлекает ID из URL
    fn extract_id(url: &str) -> Option<String> {
        if let Some(caps) = RE_ID_EXTRACT.captures(url) {
            // В объединённой регулярке: группа 1 = /download/или/music/ID, группа 2 = ID в конце
            return caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str().to_string());
        }
        None
    }

    /// Парсит страницу трека по ID
    fn fetch_track_info(id: &str) -> Result<TrackInfo, String> {
        let track_url = format!("https://mp3party.net/music/{}", id);

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Ошибка создания клиента: {}", e))?;

        let resp = client
            .get(&track_url)
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send()
            .map_err(|e| format!("Ошибка запроса: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {} для {}", resp.status(), track_url));
        }

        let body = resp.text().map_err(|e| format!("Ошибка чтения: {}", e))?;
        Self::parse_track_page(&body, id)
    }

    /// Парсит HTML страницы трека
    fn parse_track_page(body: &str, id: &str) -> Result<TrackInfo, String> {
        let document = Html::parse_document(body);
        let mut artist = String::new();
        let mut title = String::new();

        // 1. Ищем data-атрибуты
        let panel_sel = Selector::parse("div.track__user-panel").unwrap();
        if let Some(el) = document.select(&panel_sel).next() {
            if let Some(a) = el.value().attr("data-js-artist-name") {
                artist = a.to_string();
            }
            if let Some(t) = el.value().attr("data-js-song-title") {
                title = t.to_string();
            }
        }

        // 2. OG-теги
        if artist.is_empty() || title.is_empty() {
            let og_sel = Selector::parse(r#"meta[property="og:title"]"#).unwrap();
            if let Some(el) = document.select(&og_sel).next() {
                if let Some(content) = el.value().attr("content") {
                    let parts: Vec<&str> = content.split(" - ").collect();
                    if parts.len() >= 2 && artist.is_empty() {
                        artist = parts[0].trim().to_string();
                        title = parts[1..].join(" - ").trim().to_string();
                    } else if title.is_empty() {
                        title = content.to_string();
                    }
                }
            }
        }

        // 3. <title>
        if artist.is_empty() || title.is_empty() {
            let t_sel = Selector::parse("title").unwrap();
            if let Some(el) = document.select(&t_sel).next() {
                let text = el.text().collect::<String>();
                let clean = text
                    .replace("скачать mp3", "")
                    .replace("Скачать", "")
                    .replace("mp3party.net", "")
                    .trim()
                    .to_string();
                let parts: Vec<&str> = clean
                    .split(|c| c == '-' || c == '—' || c == '–')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                if parts.len() >= 2 {
                    if artist.is_empty() {
                        artist = parts[0].to_string();
                    }
                    if title.is_empty() {
                        title = parts[1..].join(" - ");
                    }
                } else if artist.is_empty() && title.is_empty() {
                    title = clean;
                }
            }
        }

        // 4. h1
        if title.is_empty() {
            let h1_sel = Selector::parse("h1").unwrap();
            if let Some(el) = document.select(&h1_sel).next() {
                title = el.text().collect::<String>().trim().to_string();
            }
        }

        if title.is_empty() {
            title = format!("Трек #{}", id);
        }

        let mut stream_url = Self::mp3party_stream_url(id);
        let panel_sel = Selector::parse("div.track__user-panel").unwrap();
        if let Some(panel) = document.select(&panel_sel).next() {
            if panel.value().attr("data-js-id") == Some(id) {
                if let Some(u) = panel.value().attr("data-js-url") {
                    if u.starts_with("http") {
                        stream_url = u.to_string();
                    }
                }
            }
        }

        Ok(TrackInfo {
            id: id.to_string(),
            artist: artist.trim().to_string(),
            title: title.trim().to_string(),
            url: stream_url,
        })
    }

    /// Поиск треков по названию — несколько стратегий
    fn search_tracks(query: &str) -> Result<Vec<TrackInfo>, String> {
        let encoded = urlencoding::encode(query);
        let url = format!("https://mp3party.net/search?q={}", encoded);

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| format!("Ошибка клиента: {}", e))?;

        let resp = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8")
            .send()
            .map_err(|e| format!("Ошибка запроса: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {} при поиске", resp.status()));
        }

        let body = resp.text().map_err(|e| format!("Ошибка чтения: {}", e))?;
        let document = Html::parse_document(&body);

        let mut results: Vec<TrackInfo> = Vec::new();

        // Стратегия 1: div.track.song-item > div.track__user-panel
        let item_sel = Selector::parse("div.track.song-item").unwrap();
        let panel_sel = Selector::parse("div.track__user-panel").unwrap();

        for item in document.select(&item_sel) {
            if let Some(panel) = item.select(&panel_sel).next() {
                Self::extract_from_panel(&panel, &mut results);
            }
        }

        // Стратегия 2: просто все div.track__user-panel
        if results.is_empty() {
            for panel in document.select(&panel_sel) {
                Self::extract_from_panel(&panel, &mut results);
            }
        }

        // Стратегия 3: любой элемент с data-js-id
        if results.is_empty() {
            let any_sel = Selector::parse("[data-js-id]").unwrap();
            for el in document.select(&any_sel) {
                let id = el.value().attr("data-js-id").unwrap_or("");
                let artist = el.value().attr("data-js-artist-name").unwrap_or("");
                let title = el.value().attr("data-js-song-title").unwrap_or("");
                if !id.is_empty() && !title.is_empty() {
                    let url = el
                        .value()
                        .attr("data-js-url")
                        .filter(|u| u.starts_with("http"))
                        .map(|u| u.to_string())
                        .unwrap_or_else(|| Self::mp3party_stream_url(id));
                    results.push(TrackInfo {
                        id: id.to_string(),
                        artist: artist.to_string(),
                        title: title.to_string(),
                        url,
                    });
                }
            }
        }

        // Убираем дубликаты по ID
        let mut unique: Vec<TrackInfo> = Vec::new();
        for track in results {
            if !unique.iter().any(|t| t.id == track.id) {
                unique.push(track);
            }
        }

        if unique.is_empty() {
            Err(format!("Ничего не найдено по запросу «{}».", query))
        } else {
            Ok(unique)
        }
    }

    fn split_ytdlp_title(full: &str, channel: &str) -> (String, String) {
        if let Some((a, t)) = full.split_once(" - ") {
            return (a.trim().to_string(), t.trim().to_string());
        }
        if !channel.is_empty() && channel != full {
            return (channel.trim().to_string(), full.trim().to_string());
        }
        ("YouTube".to_string(), full.trim().to_string())
    }

    /// Поиск через YouTube (yt-dlp ytsearch)
    fn search_tracks_ytdlp(query: &str) -> Result<Vec<TrackInfo>, String> {
        let ytdlp = Self::resolve_yt_dlp()?;
        let target = format!("ytsearch20:{}", query);

        let mut cmd = Command::new(&ytdlp);
        Self::clear_proxy_env(&mut cmd);
        Self::append_ytdlp_network_args(&mut cmd);
        cmd.args([
            "--flat-playlist",
            "--playlist-end",
            "20",
            "--print",
            "%(id)s|||%(title)s|||%(channel)s",
            &target,
        ]);

        let output =
            Self::run_command_with_timeout(cmd, Duration::from_secs(YTDLP_SEARCH_TIMEOUT_SECS))?;

        if !output.status.success() && output.stdout.is_empty() {
            let err = String::from_utf8_lossy(&output.stderr);
            let tail = err.trim();
            let msg = if tail.is_empty() {
                format!("yt-dlp завершился с кодом {:?}", output.status.code())
            } else {
                let lines: Vec<&str> = tail.lines().collect();
                let snippet = if lines.len() > 4 {
                    lines[lines.len() - 4..].join("\n")
                } else {
                    tail.to_string()
                };
                format!("YouTube/yt-dlp: {}", snippet)
            };
            return Err(msg);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results: Vec<TrackInfo> = Vec::new();

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split("|||").collect();
            if parts.len() < 2 {
                continue;
            }
            let id = parts[0].trim();
            if id.is_empty() || id == "NA" {
                continue;
            }
            let full_title = parts[1].trim();
            let channel = parts.get(2).copied().unwrap_or("").trim();
            if full_title.is_empty() || full_title == "NA" {
                continue;
            }

            let (artist, title) = Self::split_ytdlp_title(full_title, channel);
            let url = format!("https://www.youtube.com/watch?v={}", id);

            if !results.iter().any(|t| t.id == id) {
                results.push(TrackInfo {
                    id: id.to_string(),
                    artist,
                    title,
                    url,
                });
            }
        }

        if results.is_empty() {
            Err(format!(
                "Ничего не найдено на YouTube по запросу «{}».",
                query
            ))
        } else {
            Ok(results)
        }
    }

    fn filtered_track_indices(&self) -> Vec<usize> {
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

    fn begin_loading(&mut self) {
        self.loading = true;
        self.loading_started = Some(Instant::now());
    }

    fn cancel_loading(&mut self) {
        self.loading = false;
        self.loading_started = None;
        self.rx = None;
        self.status = "⏹ Операция отменена.".into();
        self.push_log_line(format!(
            "[{}] ⏹ Операция отменена пользователем.",
            Self::log_timestamp()
        ));
    }

    fn check_loading_watchdog(&mut self) {
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

    fn finish_loading(&mut self) {
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

    fn extract_from_panel(panel: &scraper::ElementRef, results: &mut Vec<TrackInfo>) {
        let id = panel.value().attr("data-js-id").unwrap_or("");
        let artist = panel.value().attr("data-js-artist-name").unwrap_or("");
        let title = panel.value().attr("data-js-song-title").unwrap_or("");
        if id.is_empty() || title.is_empty() {
            return;
        }
        let url = panel
            .value()
            .attr("data-js-url")
            .filter(|u| u.starts_with("http"))
            .map(|u| u.to_string())
            .unwrap_or_else(|| Self::mp3party_stream_url(id));
        results.push(TrackInfo {
            id: id.to_string(),
            artist: artist.to_string(),
            title: title.to_string(),
            url,
        });
    }

    fn ytdlp_install_path() -> PathBuf {
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

    fn ytdlp_download_url() -> &'static str {
        #[cfg(windows)]
        return "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
        #[cfg(target_os = "macos")]
        return "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos";
        #[cfg(not(any(windows, target_os = "macos")))]
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    }

    fn resolve_yt_dlp() -> Result<PathBuf, String> {
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

    fn install_yt_dlp() -> Result<PathBuf, String> {
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

    /// Диалог и установка yt-dlp (только с UI-потока, до фоновых задач).
    fn prompt_and_install_yt_dlp() -> Result<PathBuf, String> {
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

    fn require_yt_dlp_ui() -> Result<PathBuf, String> {
        Self::resolve_yt_dlp().or_else(|_| Self::prompt_and_install_yt_dlp())
    }

    /// Предложить установить mpv перед стримом/видео. `true` — можно воспроизводить.
    fn offer_mpv_ui(allow_skip: bool) -> bool {
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

    fn ytdlp_stream_url(track: &TrackInfo, format: YtDlpFormat) -> Result<String, String> {
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

    fn drivemusic_stream_url(track: &TrackInfo) -> Result<String, String> {
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

    fn refresh_library(&mut self) {
        self.library_files = list_downloads(&self.downloads_folder);
    }

    fn start_stream(&mut self, idx: usize) {
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

    fn show_player_bar(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        if !self.player.state.has_media {
            return;
        }
        self.player.tick();
        if let Some(seek) = self.player_seek_request.take() {
            self.player.seek_to(seek);
        }

        theme.card().show(ui, |ui| {
            ui.horizontal(|ui| {
                let icon = if self.player.state.is_playing {
                    "⏸"
                } else {
                    "▶"
                };
                if ui.add(theme.primary_button(icon)).clicked() {
                    self.player.toggle_pause();
                }
                if ui.add(theme.neutral_button("⏹")).clicked() {
                    self.player.stop();
                }
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&self.player.state.title)
                            .strong()
                            .color(theme.text_primary),
                    );
                    ui.label(
                        egui::RichText::new(&self.player.state.subtitle)
                            .size(11.0)
                            .color(theme.text_muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() {
                        self.player.stop();
                    }
                });
            });
            if self.player.state.duration_secs > 0.0 || self.player.state.position_secs > 0.0 {
                let dur = self.player.state.duration_secs.max(1.0);
                let mut pos = self.player.state.position_secs.min(dur);
                let pos_label = Self::format_duration(pos);
                if ui
                    .add(
                        egui::Slider::new(&mut pos, 0.0..=dur)
                            .text(pos_label)
                            .trailing_fill(true),
                    )
                    .changed()
                {
                    self.player_seek_request = Some(pos);
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        Self::format_duration(self.player.state.position_secs),
                        Self::format_duration(dur)
                    ))
                    .size(11.0)
                    .color(theme.text_muted),
                );
            }
        });
    }

    fn format_duration(secs: f64) -> String {
        let s = secs.max(0.0) as u64;
        format!("{}:{:02}", s / 60, s % 60)
    }

    fn show_library_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        theme.card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme.section_title("📂 Мои файлы"));
                if ui.add(theme.neutral_button("🔄 Обновить")).clicked() {
                    self.refresh_library();
                }
                if ui.add(theme.neutral_button("📂 Открыть папку")).clicked() {
                    open_folder_in_file_manager(&self.downloads_folder);
                }
            });
            ui.label(
                egui::RichText::new(self.downloads_folder.display().to_string())
                    .size(11.0)
                    .color(theme.text_muted),
            );
            ui.add_space(6.0);

            if self.library_files.is_empty() {
                ui.label(egui::RichText::new("Скачанных файлов пока нет").color(theme.text_muted));
                return;
            }

            egui::ScrollArea::vertical()
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    let mut play_path: Option<PathBuf> = None;
                    let mut play_title = String::new();
                    let mut play_video = false;
                    for f in &self.library_files {
                        ui.horizontal(|ui| {
                            let icon = if f.is_video { "🎬" } else { "🎵" };
                            ui.label(icon);
                            ui.vertical(|ui| {
                                ui.label(&f.display_name);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · {}",
                                        Self::format_bytes(f.size_bytes),
                                        f.path.file_name().unwrap_or_default().to_string_lossy()
                                    ))
                                    .size(10.0)
                                    .color(theme.text_muted),
                                );
                            });
                            if ui.add(theme.primary_button("▶")).clicked() {
                                play_path = Some(f.path.clone());
                                play_title = f.display_name.clone();
                                play_video = f.is_video;
                            }
                        });
                        ui.separator();
                    }
                    if let Some(path) = play_path {
                        let _ = if play_video {
                            if Self::offer_mpv_ui(true) {
                                self.player.play_url(
                                    path.to_str().unwrap_or(""),
                                    &play_title,
                                    "Видео",
                                    true,
                                )
                            } else {
                                Ok(())
                            }
                        } else {
                            self.player.play_file(&path, &play_title, false)
                        };
                    }
                });
        });
    }

    fn append_ytdlp_format_args(cmd: &mut Command, format: YtDlpFormat) {
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
                // как в yt-dlp-util/download.sh
                cmd.args(["-f", "bv*+ba/b", "--merge-output-format", "mp4"]);
            }
        }
    }

    fn ytdlp_speed_args(cmd: &mut Command) {
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

    fn clear_proxy_env(cmd: &mut Command) {
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

    fn append_ytdlp_network_args(cmd: &mut Command) {
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

    /// Запуск внешней команды с жёстким таймаутом (yt-dlp иначе висит минутами).
    fn run_command_with_timeout(
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

    fn mp3party_stream_url(id: &str) -> String {
        format!("https://dl2.mp3party.net/online/{}.mp3", id)
    }

    fn mp3party_download_url(id: &str) -> String {
        format!("https://dl2.mp3party.net/download/{}", id)
    }

    fn mp3party_client() -> Result<reqwest::blocking::Client, String> {
        reqwest::blocking::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(8))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("Ошибка клиента: {}", e))
    }

    fn mp3party_request<'a>(
        client: &'a reqwest::blocking::Client,
        url: &str,
        track_page: &str,
    ) -> reqwest::blocking::RequestBuilder {
        client
            .get(url)
            .header("User-Agent", BROWSER_USER_AGENT)
            .header("Referer", track_page)
            .header("Origin", "https://mp3party.net")
            .header("Accept", "audio/mpeg,application/octet-stream,*/*;q=0.8")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
    }

    fn open_external_url(url: &str) {
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

    fn mp3party_download_candidates(page_body: &str, track: &TrackInfo) -> Vec<String> {
        let mut candidates: Vec<String> = Vec::new();
        let push_unique = |list: &mut Vec<String>, url: String| {
            if url.starts_with("http") && !list.iter().any(|u| u == &url) {
                list.push(url);
            }
        };

        let document = Html::parse_document(page_body);

        if let Ok(btn_sel) = Selector::parse("a.js-dw-btn[data-track-id]") {
            for el in document.select(&btn_sel) {
                if el.value().attr("data-track-id") == Some(track.id.as_str()) {
                    if let Some(href) = el.value().attr("href") {
                        push_unique(&mut candidates, href.to_string());
                    }
                }
            }
        }

        push_unique(&mut candidates, Self::mp3party_download_url(&track.id));

        if let Ok(panel_sel) = Selector::parse("div.track__user-panel") {
            for panel in document.select(&panel_sel) {
                if panel.value().attr("data-js-id") == Some(track.id.as_str()) {
                    if let Some(u) = panel.value().attr("data-js-url") {
                        push_unique(&mut candidates, u.to_string());
                    }
                }
            }
        }

        if track.url.starts_with("http") {
            push_unique(&mut candidates, track.url.clone());
        }
        push_unique(&mut candidates, Self::mp3party_stream_url(&track.id));

        candidates
    }

    fn is_mp3party_error_body(bytes: &[u8]) -> bool {
        if bytes.len() > 512 {
            return false;
        }
        let preview = String::from_utf8_lossy(bytes);
        preview.contains("failed to get file") || preview.contains("nil")
    }

    /// Скачивание через MP3Party (/download/)
    fn kill_child_process(pid: u32) {
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

    fn is_download_cancelled(cancel: &AtomicBool) -> bool {
        cancel.load(Ordering::Relaxed)
    }

    fn set_download_stopped(
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

    fn cancel_download_task(&mut self, idx: usize) {
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

    fn cancel_all_downloads(&mut self) {
        let count = self.download_tasks.len();
        for i in 0..count {
            self.cancel_download_task(i);
        }
    }

    fn download_track_mp3party(
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

    fn drivemusic_client() -> Result<reqwest::blocking::Client, String> {
        reqwest::blocking::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(8))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("Ошибка клиента: {}", e))
    }

    fn drivemusic_page_url(track: &TrackInfo) -> Result<String, String> {
        let u = track.url.trim();
        if u.contains("drivemusic.me") && u.ends_with(".html") {
            return Ok(u.to_string());
        }
        if u.starts_with('/') && u.ends_with(".html") {
            return Ok(format!("{}{}", DRIVEMUSIC_BASE, u));
        }
        Err("DriveMusic: нет ссылки на страницу трека — найдите трек через поиск.".into())
    }

    fn drivemusic_extract_mp3_urls(html: &str) -> Vec<String> {
        let mut urls: Vec<String> = RE_DRIVEMUSIC_MP3
            .find_iter(html)
            .map(|m| m.as_str().to_string())
            .collect();
        urls.sort_by_key(|u| {
            let online = u.contains("/dl/online/");
            (online, u.len())
        });
        urls.dedup();
        urls
    }

    fn drivemusic_download_candidates(page_body: &str, _track: &TrackInfo) -> Vec<String> {
        Self::drivemusic_extract_mp3_urls(page_body)
    }

    /// Поиск на drivemusic.me
    fn search_tracks_drivemusic(query: &str) -> Result<Vec<TrackInfo>, String> {
        let encoded = urlencoding::encode(query.trim());
        let url = format!(
            "{}/?do=search&subaction=search&story={}",
            DRIVEMUSIC_BASE, encoded
        );

        let client = Self::drivemusic_client()?;
        let resp = client
            .get(&url)
            .header("User-Agent", BROWSER_USER_AGENT)
            .header("Referer", DRIVEMUSIC_BASE)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .send()
            .map_err(|e| format!("Ошибка запроса: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {} при поиске DriveMusic", resp.status()));
        }

        let body = resp.text().map_err(|e| format!("Ошибка чтения: {}", e))?;
        let mut results: Vec<TrackInfo> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for caps in RE_DRIVEMUSIC_SEARCH.captures_iter(&body) {
            let path = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let id = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let title = caps.get(3).map(|m| m.as_str().trim()).unwrap_or("");
            let artist = caps.get(4).map(|m| m.as_str().trim()).unwrap_or("");
            if id.is_empty() || title.is_empty() || !seen.insert(id.to_string()) {
                continue;
            }
            let page = if path.starts_with("http") {
                path.to_string()
            } else {
                format!("{}{}", DRIVEMUSIC_BASE, path)
            };
            results.push(TrackInfo {
                id: id.to_string(),
                artist: artist.to_string(),
                title: title.to_string(),
                url: page,
            });
        }

        if results.is_empty() {
            Err(format!(
                "Ничего не найдено на DriveMusic по запросу «{}».",
                query
            ))
        } else {
            Ok(results.into_iter().take(30).collect())
        }
    }

    fn download_track_drivemusic(
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

    /// Скачивание через yt-dlp (MP3 или MP4, как в yt-dlp-util)
    fn download_track_ytdlp(
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

    // ── Запуск парсинга ссылок ──

    fn start_parsing(&mut self) {
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

        let mut ids: Vec<(String, String)> = Vec::new();
        for line in &lines {
            if let Some(id) = Self::extract_id(line) {
                ids.push((line.to_string(), id));
            } else {
                if RE_DIGITS.is_match(line.trim()) {
                    ids.push((line.to_string(), line.trim().to_string()));
                } else {
                    self.status = format!("⚠️ Не удалось извлечь ID из: {}", line);
                    return;
                }
            }
        }

        if ids.is_empty() {
            self.status = "⚠️ Не найдено ID в ссылках.".into();
            return;
        }

        self.tracks.clear();
        self.result_filter.clear();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.total_urls = ids.len();
        self.processed = 0;
        self.error_count = 0;
        self.last_error = None;
        self.begin_loading();
        self.output_mode = OutputMode::UrlParsing;
        self.status = format!("⏳ Парсинг {} треков...", ids.len());
        self.push_log_line(format!(
            "[{}] ⏳ Парсинг {} треков...",
            Self::log_timestamp(),
            ids.len()
        ));

        let log_tx = self.log_tx.clone();
        thread::spawn(move || {
            for (orig_url, id) in ids {
                match Self::fetch_track_info(&id) {
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

    fn start_search(&mut self) {
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

    fn start_download(&mut self, track_idx: usize) {
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

    fn task_source_label(
        source: DownloadSource,
        ytdlp_format: Option<YtDlpFormat>,
    ) -> &'static str {
        match (source, ytdlp_format) {
            (DownloadSource::YtDlp, Some(YtDlpFormat::Mp3)) => "YouTube MP3",
            (DownloadSource::YtDlp, Some(YtDlpFormat::Mp4)) => "YouTube MP4",
            (DownloadSource::YtDlp, None) => "YouTube",
            (DownloadSource::Mp3Party, _) => "MP3Party",
            (DownloadSource::DriveMusic, _) => "DriveMusic",
        }
    }

    fn format_bytes(b: u64) -> String {
        if b > 1024 * 1024 {
            format!("{:.1} MB", b as f64 / 1024.0 / 1024.0)
        } else if b > 1024 {
            format!("{:.1} KB", b as f64 / 1024.0)
        } else {
            format!("{} B", b)
        }
    }

    fn open_downloads_folder(&mut self) {
        let folder = self.downloads_folder.clone();
        if let Err(e) = std::fs::create_dir_all(&folder) {
            self.status = format!("❌ Не удалось создать папку: {}", e);
            return;
        }
        match std::process::Command::new("xdg-open").arg(&folder).spawn() {
            Ok(_) => self.status = format!("📂 Открыта папка: {}", folder.display()),
            Err(e) => self.status = format!("❌ Не удалось открыть папку: {}", e),
        }
    }

    fn has_active_downloads(&self) -> bool {
        self.download_tasks.iter().any(|t| {
            matches!(
                *t.status.lock().unwrap(),
                DownloadStatus::Pending | DownloadStatus::Downloading { .. }
            )
        })
    }

    fn show_download_progress(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        let active: Vec<_> = self
            .download_tasks
            .iter()
            .filter_map(|t| {
                let s = t.status.lock().unwrap().clone();
                match s {
                    DownloadStatus::Pending => {
                        Some((t.track.clone(), t.source, t.ytdlp_format, None))
                    }
                    DownloadStatus::Downloading {
                        progress,
                        bytes,
                        total,
                    } => Some((
                        t.track.clone(),
                        t.source,
                        t.ytdlp_format,
                        Some((progress, bytes, total)),
                    )),
                    _ => None,
                }
            })
            .collect();

        if active.is_empty() {
            return;
        }

        theme.card().show(ui, |ui| {
            let folder_hint = self.downloads_folder.display().to_string();
            ui.horizontal(|ui| {
                ui.label(theme.section_title("📥 Скачивание"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(theme.neutral_button("📂 Открыть папку"))
                        .on_hover_text(&folder_hint)
                        .clicked()
                    {
                        self.open_downloads_folder();
                    }
                });
            });

            ui.add_space(6.0);

            let mut total_bytes: u64 = 0;
            let mut total_size: u64 = 0;

            for (track, source, ytdlp_fmt, prog) in &active {
                ui.label(
                    egui::RichText::new(format!(
                        "[{}] {} — {}",
                        Self::task_source_label(*source, *ytdlp_fmt),
                        track.artist,
                        track.title
                    ))
                    .size(12.0)
                    .strong()
                    .color(theme.text_primary),
                );

                match prog {
                    None => {
                        ui.add(
                            egui::ProgressBar::new(0.0)
                                .animate(true)
                                .text("⏳ Подготовка…")
                                .desired_width(ui.available_width()),
                        );
                    }
                    Some((progress, bytes, total)) => {
                        if *total > 0 {
                            total_bytes += bytes;
                            total_size += total;
                            ui.add(
                                egui::ProgressBar::new(*progress)
                                    .show_percentage()
                                    .fill(theme.progress)
                                    .desired_width(ui.available_width())
                                    .text(format!(
                                        "{} / {}",
                                        Self::format_bytes(*bytes),
                                        Self::format_bytes(*total)
                                    )),
                            );
                        } else {
                            total_bytes += bytes;
                            ui.add(
                                egui::ProgressBar::new(0.0)
                                    .animate(true)
                                    .fill(theme.progress)
                                    .desired_width(ui.available_width())
                                    .text(format!("{} скачано", Self::format_bytes(*bytes))),
                            );
                        }
                    }
                }
                ui.add_space(6.0);
            }

            if total_size > 0 {
                let overall = total_bytes as f32 / total_size as f32;
                ui.separator();
                ui.label(
                    egui::RichText::new("Общий прогресс")
                        .size(11.0)
                        .color(theme.text_muted),
                );
                ui.add(
                    egui::ProgressBar::new(overall.min(1.0))
                        .show_percentage()
                        .fill(theme.accent)
                        .desired_width(ui.available_width())
                        .text(format!(
                            "{} / {}",
                            Self::format_bytes(total_bytes),
                            Self::format_bytes(total_size)
                        )),
                );
            }
        });
    }
}

// ═══════════════════════════════════════════
//  EFAME APP — красивый GUI
// ═══════════════════════════════════════════

impl eframe::App for LinkParserApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = self.theme();
        theme.apply(ctx);
        self.drain_log_messages();
        self.check_loading_watchdog();

        if let Some(rx) = &self.rx {
            if let Ok(result) = rx.try_recv() {
                self.processed += 1;
                match result {
                    ParseResult::Success(track) => {
                        self.tracks.push(track);
                    }
                    ParseResult::SearchResults(results) => {
                        self.tracks.extend(results);
                    }
                    ParseResult::Error(_url, err) => {
                        self.error_count += 1;
                        self.last_error = Some(err);
                    }
                }
                if self.processed >= self.total_urls {
                    self.finish_loading();
                } else {
                    self.status = format!("⏳ {}/{}", self.processed, self.total_urls);
                }
                ctx.request_repaint();
            }
        }

        if self.has_active_downloads() {
            ctx.request_repaint();
        }

        if let Some(rx) = self.stream_rx.take() {
            if let Ok(result) = rx.try_recv() {
                self.loading = false;
                match result {
                    Ok((url, title, sub, is_video)) => {
                        if let Err(e) = self.player.play_url(&url, &title, &sub, is_video) {
                            self.status = format!("❌ Плеер: {}", e);
                        } else {
                            self.status = format!("🎧 {}", title);
                        }
                    }
                    Err(e) => self.status = format!("❌ Стрим: {}", e),
                }
                ctx.request_repaint();
            } else {
                self.stream_rx = Some(rx);
            }
        }

        if self.player.state.has_media {
            self.player.tick();
            ctx.request_repaint_after(Duration::from_millis(400));
        }

        egui::TopBottomPanel::bottom("player_bar")
            .frame(Frame {
                fill: theme.card_bg,
                inner_margin: Margin::symmetric(12.0, 8.0),
                ..Default::default()
            })
            .show(ctx, |ui| {
                self.show_player_bar(ui, theme);
            });

        egui::TopBottomPanel::top("header")
            .frame(Frame {
                fill: theme.header_bg,
                inner_margin: Margin::symmetric(16.0, 12.0),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🎵 MP3Party")
                            .size(20.0)
                            .color(theme.text_on_header)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("загрузка треков")
                            .size(13.0)
                            .color(theme.text_on_header.gamma_multiply(0.75)),
                    );
                    if self.loading {
                        ui.add_space(8.0);
                        ui.add(egui::Spinner::new().color(theme.text_on_header));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(theme.header_button("✕"))
                            .on_hover_text("Закрыть")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }

                        let theme_label = if self.is_dark_mode {
                            "☀️ Светлая"
                        } else {
                            "🌙 Тёмная"
                        };
                        if ui.add(theme.header_button(theme_label)).clicked() {
                            self.is_dark_mode = !self.is_dark_mode;
                        }

                        let active_count = self
                            .download_tasks
                            .iter()
                            .filter(|t| {
                                matches!(
                                    *t.status.lock().unwrap(),
                                    DownloadStatus::Downloading { .. }
                                )
                            })
                            .count();
                        let dl_text = if active_count > 0 {
                            format!("📥 Загрузки ({})", active_count)
                        } else {
                            "📥 Загрузки".to_string()
                        };
                        if ui.add(theme.header_button(&dl_text)).clicked() {
                            self.show_downloads = !self.show_downloads;
                        }

                        let logs_text = if self.log_lines.is_empty() {
                            "📋 Логи".to_string()
                        } else {
                            format!("📋 Логи ({})", self.log_lines.len())
                        };
                        if ui.add(theme.header_button(&logs_text)).clicked() {
                            self.show_logs = !self.show_logs;
                        }
                    });
                });
            });

        if self.show_downloads {
            egui::Window::new("📥 Загрузки")
                .id("download_window".into())
                .resizable(true)
                .default_size([520.0, 320.0])
                .collapsible(true)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(theme.card_bg)
                        .stroke(Stroke::new(1.0, theme.card_border))
                        .rounding(Rounding::same(10.0)),
                )
                .show(ctx, |ui| {
                    self.show_downloads_panel(ui, theme);
                });
        }

        if self.show_logs {
            egui::Window::new("📋 Логи")
                .id("logs_window".into())
                .resizable(true)
                .default_size([560.0, 360.0])
                .collapsible(true)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(theme.card_bg)
                        .stroke(Stroke::new(1.0, theme.card_border))
                        .rounding(Rounding::same(10.0)),
                )
                .show(ctx, |ui| {
                    self.show_logs_panel(ui, theme);
                });
        }

        egui::CentralPanel::default()
            .frame(Frame {
                fill: theme.window_bg,
                inner_margin: Margin::same(16.0),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.main_tab == MainTab::Search, "🔎 Поиск")
                        .clicked()
                    {
                        self.main_tab = MainTab::Search;
                    }
                    if ui
                        .selectable_label(self.main_tab == MainTab::Library, "📂 Мои файлы")
                        .clicked()
                    {
                        self.main_tab = MainTab::Library;
                        self.refresh_library();
                    }
                });
                ui.add_space(8.0);
                match self.main_tab {
                    MainTab::Search => self.show_main_panel(ui, theme),
                    MainTab::Library => self.show_library_panel(ui, theme),
                }
            });
    }
}

// ═══════════════════════════════════════════
//  Отрисовка панелей
// ═══════════════════════════════════════════

impl LinkParserApp {
    fn show_main_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        theme.card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Источник (поиск и скачивание):")
                        .size(13.0)
                        .strong()
                        .color(theme.text_primary),
                );
                egui::ComboBox::from_id_salt("download_source")
                    .selected_text(self.download_source.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.download_source,
                            DownloadSource::Mp3Party,
                            DownloadSource::Mp3Party.label(),
                        );
                        ui.selectable_value(
                            &mut self.download_source,
                            DownloadSource::DriveMusic,
                            DownloadSource::DriveMusic.label(),
                        );
                        ui.selectable_value(
                            &mut self.download_source,
                            DownloadSource::YtDlp,
                            DownloadSource::YtDlp.label(),
                        );
                    });
            });
            if self.download_source == DownloadSource::YtDlp {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Формат yt-dlp:")
                            .size(12.0)
                            .color(theme.text_secondary),
                    );
                    egui::ComboBox::from_id_salt("ytdlp_format")
                        .selected_text(self.ytdlp_format.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.ytdlp_format,
                                YtDlpFormat::Mp3,
                                YtDlpFormat::Mp3.label(),
                            );
                            ui.selectable_value(
                                &mut self.ytdlp_format,
                                YtDlpFormat::Mp4,
                                YtDlpFormat::Mp4.label(),
                            );
                        });
                });
            }

            ui.add_space(4.0);
            let hint = match self.download_source {
                DownloadSource::Mp3Party => "Поиск на mp3party.net, скачивание online/download URL",
                DownloadSource::DriveMusic => {
                    "Поиск на drivemusic.me; ссылки на MP3 временные — скачивание со страницы трека"
                }
                DownloadSource::YtDlp => match self.ytdlp_format {
                    YtDlpFormat::Mp3 => {
                        "YouTube: поиск ytsearch, скачивание MP3 (-x --audio-format mp3)"
                    }
                    YtDlpFormat::Mp4 => "YouTube: поиск ytsearch, скачивание MP4 (видео+аудио)",
                },
            };
            ui.label(egui::RichText::new(hint).size(11.0).color(theme.text_muted));
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.columns(2, |cols| {
                // ── Парсинг ссылок ──
                theme.card().show(&mut cols[0], |ui| {
                    ui.label(theme.section_title("📎 Парсинг ссылок"));
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Вставьте ссылки или ID треков (по одному на строку)")
                            .size(12.0)
                            .color(theme.text_muted),
                    );
                    ui.add_space(4.0);

                    let input_h = 72.0;
                    egui::ScrollArea::vertical()
                        .id_salt("input_scroll")
                        .max_height(input_h)
                        .show(ui, |ui| {
                            ui.add_sized(
                                [ui.available_width(), input_h],
                                egui::TextEdit::multiline(&mut self.input_text)
                                    .desired_width(f32::INFINITY)
                                    .font(egui::TextStyle::Monospace)
                                    .hint_text("https://dl2.mp3party.net/download/8787500"),
                            );
                        });
                });

                // ── Поиск ──
                theme.card().show(&mut cols[1], |ui| {
                    let search_title = match self.download_source {
                        DownloadSource::Mp3Party => "🔎 Поиск (MP3Party)",
                        DownloadSource::DriveMusic => "🔎 Поиск (DriveMusic)",
                        DownloadSource::YtDlp => "🔎 Поиск (YouTube)",
                    };
                    ui.label(theme.section_title(search_title));
                    ui.add_space(6.0);
                    let search_hint = match self.download_source {
                        DownloadSource::Mp3Party => "Исполнитель или название на mp3party",
                        DownloadSource::DriveMusic => "Исполнитель или название на drivemusic",
                        DownloadSource::YtDlp => "Запрос для поиска на YouTube",
                    };
                    ui.label(
                        egui::RichText::new(search_hint)
                            .size(12.0)
                            .color(theme.text_muted),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        let placeholder = match self.download_source {
                            DownloadSource::Mp3Party => "Queen, Кино…",
                            DownloadSource::DriveMusic => "Исполнитель или название…",
                            DownloadSource::YtDlp => "Queen Killer Queen…",
                        };
                        let search_resp = ui.add_sized(
                            [ui.available_width() - 100.0, 32.0],
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text(placeholder),
                        );

                        let search_clicked = ui
                            .add_enabled(!self.loading, theme.primary_button("🔍 Найти"))
                            .clicked();

                        if self.loading
                            && ui
                                .add_enabled(true, theme.neutral_button("⏹ Отмена"))
                                .clicked()
                        {
                            self.cancel_loading();
                        }

                        let enter_in_search = ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && search_resp.has_focus();

                        if search_clicked || enter_in_search {
                            self.start_search();
                        }
                    });
                });
            });
        });

        ui.add_space(10.0);

        // ── Панель действий ──
        theme.card().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.loading, theme.success_button("📥 Парсить ссылки"))
                    .clicked()
                {
                    self.tracks.clear();
                    self.output_mode = OutputMode::UrlParsing;
                    self.start_parsing();
                }

                if ui.add(theme.neutral_button("🗑 Очистить")).clicked() {
                    self.tracks.clear();
                    self.input_text.clear();
                    self.search_query.clear();
                    self.result_filter.clear();
                    self.output_mode = OutputMode::UrlParsing;
                    self.status = "✅ Очищено".into();
                }

                if !self.tracks.is_empty()
                    && ui.add(theme.neutral_button("📋 Копировать")).clicked()
                {
                    let mut text = String::from("ID\tИсполнитель\tНазвание\tСсылка\n");
                    for t in &self.tracks {
                        text.push_str(&format!("{}\t{}\t{}\t{}\n", t.id, t.artist, t.title, t.url));
                    }
                    ui.output_mut(|o| o.copied_text = text);
                    self.status = "📋 Скопировано в буфер".into();
                }

                if ui.add(theme.neutral_button("📂 Выбрать папку")).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Выберите папку для загрузок")
                        .pick_folder()
                    {
                        self.downloads_folder = path;
                        self.status = format!("📂 Папка: {}", self.downloads_folder.display());
                    }
                }

                if ui
                    .add(theme.neutral_button("📂 Открыть папку"))
                    .on_hover_text("Открыть папку загрузок в файловом менеджере")
                    .clicked()
                {
                    self.open_downloads_folder();
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Сохранение в:")
                        .size(11.0)
                        .color(theme.text_muted),
                );
                if ui
                    .link(
                        egui::RichText::new(self.downloads_folder.display().to_string()).size(11.0),
                    )
                    .on_hover_text("Открыть папку")
                    .clicked()
                {
                    self.open_downloads_folder();
                }
            });
        });

        ui.add_space(8.0);

        // ── Статус ──
        theme.status_bar().show(ui, |ui| {
            ui.horizontal(|ui| {
                let color = theme.status_color(&self.status, self.loading);
                ui.colored_label(color, egui::RichText::new(&self.status).size(13.0));
            });
        });

        if self.has_active_downloads() {
            ui.add_space(8.0);
            self.show_download_progress(ui, theme);
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // ─── Таблица результатов ───
        if !self.tracks.is_empty() {
            let mode_label = match self.output_mode {
                OutputMode::UrlParsing => "по ссылкам".to_string(),
                OutputMode::Search => format!("поиск — {}", self.download_source.label()),
            };

            theme.card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        theme
                            .section_title(&format!(
                                "📊 Результаты ({}) — {} треков",
                                mode_label,
                                self.tracks.len()
                            ))
                            .size(14.0),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_sized(
                            [200.0, 26.0],
                            egui::TextEdit::singleline(&mut self.result_filter)
                                .hint_text("🔍 фильтр…"),
                        );
                        ui.label(
                            egui::RichText::new("Фильтр")
                                .size(12.0)
                                .color(theme.text_muted),
                        );

                        if ui
                            .add(theme.success_button("📥 Скачать все"))
                            .on_hover_text("Скачать все отфильтрованные треки")
                            .clicked()
                        {
                            let filtered = self.filtered_track_indices();
                            for idx in filtered {
                                self.start_download(idx);
                            }
                        }
                    });
                });

                let filtered_indices = self.filtered_track_indices();
                if !self.result_filter.trim().is_empty() {
                    ui.label(
                        egui::RichText::new(format!("Показано: {}", filtered_indices.len()))
                            .size(11.0)
                            .color(theme.text_muted),
                    );
                }

                ui.add_space(6.0);

                let avail = ui.available_height();
                let table_h = if self.show_downloads {
                    avail * 0.55
                } else {
                    avail
                };

                egui::ScrollArea::vertical()
                    .id_salt("results_scroll")
                    .auto_shrink([false; 2])
                    .max_height(table_h.max(120.0))
                    .show(ui, |ui| {
                        let mut to_remove: Option<usize> = None;
                        let mut to_download: Option<usize> = None;
                        let mut to_stream: Option<usize> = None;

                        egui::Grid::new(format!("results_grid_{:?}", self.output_mode))
                            .striped(true)
                            .spacing([12.0, 8.0])
                            .min_col_width(60.0)
                            .show(ui, |ui| {
                                ui.strong(
                                    egui::RichText::new("ID").size(12.0).color(theme.text_muted),
                                );
                                ui.strong(
                                    egui::RichText::new("Исполнитель")
                                        .size(12.0)
                                        .color(theme.text_muted),
                                );
                                ui.strong(
                                    egui::RichText::new("Название")
                                        .size(12.0)
                                        .color(theme.text_muted),
                                );
                                ui.strong(
                                    egui::RichText::new(" ").size(12.0).color(theme.text_muted),
                                );
                                ui.end_row();

                                for i in &filtered_indices {
                                    let track = &self.tracks[*i];

                                    if ui
                                        .link(
                                            egui::RichText::new(&track.id)
                                                .color(theme.link)
                                                .size(12.0),
                                        )
                                        .clicked()
                                    {
                                        ui.output_mut(|o| o.copied_text = track.id.clone());
                                        self.status = format!("📋 ID {} скопирован", track.id);
                                    }

                                    ui.label(
                                        egui::RichText::new(&track.artist)
                                            .color(theme.text_primary)
                                            .size(13.0),
                                    );

                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(&track.title)
                                                .color(theme.text_secondary)
                                                .size(13.0),
                                        );
                                        let src = self.download_source.label();
                                        if ui
                                            .add(
                                                theme
                                                    .success_button("⬇")
                                                    .min_size(Vec2::new(36.0, 24.0)),
                                            )
                                            .on_hover_text(format!("Скачать через {}", src))
                                            .clicked()
                                        {
                                            to_download = Some(*i);
                                        }
                                        if ui
                                            .add(
                                                theme
                                                    .neutral_button("▶")
                                                    .min_size(Vec2::new(36.0, 24.0)),
                                            )
                                            .on_hover_text("Слушать онлайн")
                                            .clicked()
                                        {
                                            to_stream = Some(*i);
                                        }
                                    });

                                    if ui.small_button("✕").on_hover_text("Убрать").clicked()
                                    {
                                        to_remove = Some(*i);
                                    }
                                    ui.end_row();
                                }
                            });

                        if let Some(idx) = to_download {
                            self.start_download(idx);
                        }
                        if let Some(idx) = to_stream {
                            self.start_stream(idx);
                        }
                        if let Some(idx) = to_remove {
                            self.tracks.remove(idx);
                        }
                    });
            });
        } else if !self.loading {
            ui.add_space(32.0);
            ui.vertical_centered(|ui| {
                theme.card().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("🎧").size(36.0).color(theme.text_muted));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Вставьте ссылки или найдите треки")
                                .size(15.0)
                                .color(theme.text_secondary),
                        );
                        ui.label(
                            egui::RichText::new("Результаты появятся здесь")
                                .size(12.0)
                                .color(theme.text_muted),
                        );
                    });
                });
            });
        }
    }

    fn show_logs_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.horizontal(|ui| {
            ui.label(
                theme
                    .section_title(&format!("📋 {} строк", self.log_lines.len()))
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme.neutral_button("📋 Копировать")).clicked() {
                    let text = self.log_lines.join("\n");
                    ui.output_mut(|o| o.copied_text = text);
                }
                if ui.add(theme.neutral_button("🗑 Очистить")).clicked() {
                    self.log_lines.clear();
                    self.push_log_line(format!("[{}] Лог очищен.", Self::log_timestamp()));
                }
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        if self.log_lines.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Лог пуст")
                        .size(14.0)
                        .color(theme.text_muted),
                );
            });
            return;
        }

        let scroll_height = ui.available_height().max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("logs_scroll")
            .auto_shrink([false; 2])
            .max_height(scroll_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log_lines {
                    ui.label(
                        egui::RichText::new(line)
                            .size(12.0)
                            .family(egui::FontFamily::Monospace)
                            .color(theme.text_secondary),
                    );
                }
            });
    }

    fn show_downloads_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.horizontal(|ui| {
            ui.label(
                theme
                    .section_title(&format!("📥 {} задач", self.download_tasks.len()))
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme.neutral_button("📂 Открыть папку")).clicked() {
                    self.open_downloads_folder();
                }
                if ui.add(theme.neutral_button("🗑 Очистить готовые")).clicked() {
                    self.download_tasks.retain(|t| {
                        let s = t.status.lock().unwrap();
                        matches!(
                            *s,
                            DownloadStatus::Downloading { .. } | DownloadStatus::Pending
                        )
                    });
                }
                if self.has_active_downloads()
                    && ui
                        .add(theme.neutral_button("⏹ Остановить всё"))
                        .on_hover_text("Принудительно остановить все загрузки")
                        .clicked()
                {
                    self.cancel_all_downloads();
                }
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        if self.download_tasks.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Нет активных загрузок")
                        .size(14.0)
                        .color(theme.text_muted),
                );
            });
            return;
        }

        let mut to_remove: Vec<usize> = Vec::new();
        let mut to_cancel: Vec<usize> = Vec::new();
        let mut open_folder = false;

        let scroll_height = ui.available_height().max(100.0);
        egui::ScrollArea::vertical()
            .id_salt("download_scroll")
            .auto_shrink([false; 2])
            .max_height(scroll_height)
            .show(ui, |ui| {
                for (i, task) in self.download_tasks.iter().enumerate() {
                    let status = task.status.lock().unwrap().clone();
                    let track = &task.track;

                    theme.card().show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "[{}] {} — {}",
                                Self::task_source_label(task.source, task.ytdlp_format),
                                track.artist,
                                track.title
                            ))
                            .size(13.0)
                            .strong()
                            .color(theme.text_primary),
                        );

                        ui.add_space(6.0);

                        match &status {
                            DownloadStatus::Pending => {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::ProgressBar::new(0.0)
                                            .animate(true)
                                            .text("⏳ Ожидание…")
                                            .desired_width(ui.available_width() - 90.0),
                                    );
                                    if ui
                                        .add(theme.neutral_button("⏹ Стоп"))
                                        .on_hover_text("Принудительно остановить")
                                        .clicked()
                                    {
                                        to_cancel.push(i);
                                    }
                                });
                            }
                            DownloadStatus::Downloading {
                                progress,
                                bytes,
                                total,
                            } => {
                                ui.horizontal(|ui| {
                                    if *total > 0 {
                                        let pct = (progress * 100.0) as u32;
                                        ui.add(
                                            egui::ProgressBar::new(*progress)
                                                .show_percentage()
                                                .desired_width(ui.available_width() - 90.0)
                                                .fill(theme.progress)
                                                .text(format!(
                                                    "{} / {} ({}%)",
                                                    Self::format_bytes(*bytes),
                                                    Self::format_bytes(*total),
                                                    pct
                                                )),
                                        );
                                    } else {
                                        ui.add(
                                            egui::ProgressBar::new(*progress)
                                                .animate(*progress < 0.01)
                                                .fill(theme.progress)
                                                .desired_width(ui.available_width() - 90.0)
                                                .text(format!(
                                                    "{} скачано",
                                                    Self::format_bytes(*bytes)
                                                )),
                                        );
                                    }
                                    if ui
                                        .add(theme.neutral_button("⏹ Стоп"))
                                        .on_hover_text("Принудительно остановить")
                                        .clicked()
                                    {
                                        to_cancel.push(i);
                                    }
                                });
                            }
                            DownloadStatus::Cancelled => {
                                ui.horizontal(|ui| {
                                    ui.colored_label(theme.text_muted, "⏹ Остановлено");
                                    if ui.small_button("✕").clicked() {
                                        to_remove.push(i);
                                    }
                                });
                            }
                            DownloadStatus::Completed(_path) => {
                                ui.horizontal(|ui| {
                                    ui.colored_label(theme.success, "✅ Завершено");
                                    if ui
                                        .add(theme.neutral_button("📂 Открыть"))
                                        .on_hover_text("Открыть папку загрузок")
                                        .clicked()
                                    {
                                        open_folder = true;
                                    }
                                    if ui.small_button("✕").clicked() {
                                        to_remove.push(i);
                                    }
                                });
                            }
                            DownloadStatus::Failed(err) => {
                                ui.colored_label(theme.error, format!("❌ {}", err));
                                ui.horizontal(|ui| {
                                    if task.source == DownloadSource::Mp3Party {
                                        let page =
                                            format!("https://mp3party.net/music/{}", track.id);
                                        if ui
                                            .add(theme.neutral_button("🌐 Браузер"))
                                            .on_hover_text("Открыть страницу трека")
                                            .clicked()
                                        {
                                            Self::open_external_url(&page);
                                        }
                                    }
                                    if task.source == DownloadSource::DriveMusic {
                                        if let Ok(page) = Self::drivemusic_page_url(track) {
                                            if ui
                                                .add(theme.neutral_button("🌐 Браузер"))
                                                .on_hover_text("Открыть страницу трека")
                                                .clicked()
                                            {
                                                Self::open_external_url(&page);
                                            }
                                        }
                                    }
                                    if ui.small_button("✕").clicked() {
                                        to_remove.push(i);
                                    }
                                });
                            }
                        }
                    });

                    ui.add_space(6.0);
                }
            });

        if open_folder {
            self.open_downloads_folder();
        }

        for idx in to_cancel {
            self.cancel_download_task(idx);
        }

        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            self.download_tasks.remove(idx);
        }
    }
}

// ═══════════════════════════════════════════
//  Точка входа
// ═══════════════════════════════════════════

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 680.0])
            .with_min_inner_size([600.0, 450.0])
            .with_title("🎵 MP3Party Parser — загрузка треков"),
        ..Default::default()
    };

    eframe::run_native(
        "mp3party_link_parser",
        options,
        Box::new(|_cc| Ok(Box::new(LinkParserApp::default()))),
    )
}
