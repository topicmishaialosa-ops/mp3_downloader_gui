use regex::Regex;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};

use crate::library::LocalMedia;
use crate::player::AudioPlayer;

pub(crate) static RE_ID_EXTRACT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:/download/|/music/)(\d+)|(?:^|/)(\d+)/?$").unwrap());
pub(crate) static RE_DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());
pub(crate) static RE_YTDLP_PERCENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{1,3})(?:\.\d+)?%").unwrap());
pub(crate) static RE_DRIVEMUSIC_MP3: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https://[a-z0-9.-]*drivemusic\.me/dl/[^"\s<>]+\.mp3"#).unwrap());
pub(crate) static RE_DRIVEMUSIC_SEARCH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)href="(/[a-z0-9_]+/(\d+)-[^"]+\.html)"[^>]*class="popular-play-author"[^>]*>([^<]*)</a>.*?popular-play-composition.*?>(?:<a[^>]*>)?([^<]*)"#,
    )
    .unwrap()
});
pub(crate) static RE_PESNIME_TRACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\\"id\\":(\d+),\\"artist\\":\\"([^"\\]*)\\",\\"title\\":\\"([^"\\]*)\\",\\"version\\":\\"[^"\\]*\\",\\"duration\\":(\d+),\\"bitrate\\":([^,]*),\\"size\\":([^,]*),\\"play\\":\\"([^"\\]+)\\",\\"download\\":\\"([^"\\]+)\\""#,
    )
    .unwrap()
});

pub const DRIVEMUSIC_BASE: &str = "https://ru.drivemusic.me";
pub const PESNIME_BASE: &str = "https://mix.pesni.me";
pub const MIN_DOWNLOAD_BYTES: u64 = 50 * 1024;
pub const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
pub const MAX_LOG_LINES: usize = 2000;
pub const YTDLP_SEARCH_TIMEOUT_SECS: u64 = 45;
pub const LOADING_WATCHDOG_SECS: u64 = 90;

pub fn default_downloads_folder() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    home.join("mp3_downloader_gui").join("downloads")
}

#[derive(Clone, Debug)]
pub struct TrackInfo {
    pub id: String,
    pub artist: String,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub enum DownloadStatus {
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

pub struct DownloadTask {
    pub _id: usize,
    pub track: TrackInfo,
    pub source: DownloadSource,
    pub ytdlp_format: Option<YtDlpFormat>,
    pub status: Arc<Mutex<DownloadStatus>>,
    pub cancel: Arc<AtomicBool>,
    pub child_pid: Arc<Mutex<Option<u32>>>,
}

pub enum ParseResult {
    Success(TrackInfo),
    SearchResults(Vec<TrackInfo>),
    Error(String, String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputMode {
    UrlParsing,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainTab {
    Search,
    Library,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadSource {
    Mp3Party,
    DriveMusic,
    YtDlp,
    PesniMe,
}

impl DownloadSource {
    pub fn label(self) -> &'static str {
        match self {
            DownloadSource::Mp3Party => "MP3Party",
            DownloadSource::DriveMusic => "DriveMusic",
            DownloadSource::YtDlp => "YouTube (yt-dlp)",
            DownloadSource::PesniMe => "Pesni.me",
        }
    }

    pub fn impe_name(self) -> &'static str {
        match self {
            DownloadSource::Mp3Party => "MP3Party",
            DownloadSource::DriveMusic => "DriveMusic",
            DownloadSource::YtDlp => "YouTube",
            DownloadSource::PesniMe => "PesniMe",
        }
    }

    pub fn from_impe_name(s: &str) -> Option<Self> {
        match s {
            "MP3Party" => Some(DownloadSource::Mp3Party),
            "DriveMusic" => Some(DownloadSource::DriveMusic),
            "YouTube" => Some(DownloadSource::YtDlp),
            "PesniMe" => Some(DownloadSource::PesniMe),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YtDlpFormat {
    Mp3,
    Mp4,
}

impl YtDlpFormat {
    pub fn label(self) -> &'static str {
        match self {
            YtDlpFormat::Mp3 => "MP3 (аудио)",
            YtDlpFormat::Mp4 => "MP4 (видео)",
        }
    }

    pub fn archive_ext(self) -> &'static str {
        match self {
            YtDlpFormat::Mp3 => "mp3",
            YtDlpFormat::Mp4 => "mp4",
        }
    }
}

pub struct CommandOutput {
    pub status: std::process::ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub struct LinkParserApp {
    pub input_text: String,
    pub search_query: String,
    pub batch_input: String,
    pub result_filter: String,
    pub tracks: Vec<TrackInfo>,
    pub status: String,
    pub loading: bool,
    pub loading_started: Option<std::time::Instant>,
    pub rx: Option<mpsc::Receiver<ParseResult>>,
    pub total_urls: usize,
    pub processed: usize,
    pub error_count: usize,
    pub last_error: Option<String>,
    pub download_tasks: Vec<DownloadTask>,
    pub downloads_folder: PathBuf,
    pub next_download_id: usize,
    pub show_downloads: bool,
    pub show_logs: bool,
    pub show_batch_window: bool,
    pub batch_autodownload: bool,
    pub log_lines: Vec<String>,
    pub log_tx: mpsc::Sender<String>,
    pub log_rx: mpsc::Receiver<String>,
    pub output_mode: OutputMode,
    pub download_source: DownloadSource,
    pub ytdlp_format: YtDlpFormat,
    pub is_dark_mode: bool,
    pub main_tab: MainTab,
    pub library_files: Vec<LocalMedia>,
    pub player: AudioPlayer,
    pub player_seek_request: Option<f64>,
    pub player_volume_request: Option<f32>,
    pub show_playlist_window: bool,
    pub stream_rx: Option<mpsc::Receiver<Result<(String, String, String, bool), String>>>,
    pub impe_to_handle: Option<(TrackInfo, DownloadSource)>,
    pub copy_source: DownloadSource,
    pub copy_rx: Option<mpsc::Receiver<String>>,
}

impl Default for LinkParserApp {
    fn default() -> Self {
        let (log_tx, log_rx) = mpsc::channel();
        Self {
            input_text: String::new(),
            search_query: String::new(),
            batch_input: String::new(),
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
            show_batch_window: false,
            batch_autodownload: false,
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
            player_volume_request: None,
            show_playlist_window: false,
            stream_rx: None,
            impe_to_handle: None,
            copy_source: DownloadSource::Mp3Party,
            copy_rx: None,
        }
    }
}
