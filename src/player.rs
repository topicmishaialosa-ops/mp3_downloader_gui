use std::collections::VecDeque;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use regex::Regex;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::sync::LazyLock;

use rand::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    NoRepeat,
    RepeatAll,
    RepeatOne,
}

impl LoopMode {
    pub fn next(self) -> Self {
        match self {
            LoopMode::NoRepeat => LoopMode::RepeatAll,
            LoopMode::RepeatAll => LoopMode::RepeatOne,
            LoopMode::RepeatOne => LoopMode::NoRepeat,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LoopMode::NoRepeat => "🔁 Нет",
            LoopMode::RepeatAll => "🔁 Все",
            LoopMode::RepeatOne => "🔂 Один",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlaylistItem {
    pub path_or_url: String,
    pub title: String,
    pub subtitle: String,
    pub is_video: bool,
    pub is_url: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub title: String,
    pub subtitle: String,
    pub is_playing: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub has_media: bool,
    pub is_video: bool,
}

pub struct AudioPlayer {
    _stream: Option<OutputStream>,
    sink: Option<Sink>,
    mpv_child: Arc<Mutex<Option<Child>>>,
    pub state: PlayerState,
    mode: PlayMode,
    pub loop_mode: LoopMode,
    pub shuffle: bool,
    pub volume: f32,
    pub playlist: VecDeque<PlaylistItem>,
    pub playlist_index: usize,
    shuffle_history: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayMode {
    None,
    Rodio,
    External,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self {
            _stream: None,
            sink: None,
            mpv_child: Arc::new(Mutex::new(None)),
            state: PlayerState::default(),
            mode: PlayMode::None,
            loop_mode: LoopMode::NoRepeat,
            shuffle: false,
            volume: 0.8,
            playlist: VecDeque::new(),
            playlist_index: 0,
            shuffle_history: Vec::new(),
        }
    }
}

impl AudioPlayer {
    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self._stream = None;
        if let Some(mut child) = self.mpv_child.lock().unwrap().take() {
            let _ = child.kill();
        }
        self.mode = PlayMode::None;
        self.state = PlayerState::default();
        self.playlist.clear();
        self.playlist_index = 0;
        self.shuffle_history.clear();
    }

    pub fn toggle_pause(&mut self) {
        match self.mode {
            PlayMode::Rodio => {
                if let Some(sink) = &self.sink {
                    if sink.is_paused() {
                        sink.play();
                        self.state.is_playing = true;
                    } else {
                        sink.pause();
                        self.state.is_playing = false;
                    }
                }
            }
            PlayMode::External => {
                self.send_mpv(&["cycle", "pause"]);
                self.state.is_playing = !self.state.is_playing;
            }
            PlayMode::None => {}
        }
    }

    pub fn seek_to(&mut self, secs: f64) {
        let s = secs.max(0.0);
        match self.mode {
            PlayMode::External => {
                self.send_mpv(&["seek", &format!("{:.2}", s), "absolute"]);
                self.state.position_secs = s;
            }
            PlayMode::Rodio => {
                self.state.position_secs = s;
            }
            PlayMode::None => {}
        }
    }

    pub fn tick(&mut self) {
        match self.mode {
            PlayMode::Rodio => {
                if let Some(sink) = &self.sink {
                    self.state.position_secs = sink.get_pos().as_secs_f64();
                    if sink.empty() && self.state.is_playing {
                        self.state.is_playing = false;
                        self.play_next();
                    }
                }
            }
            PlayMode::External => {
                let finished = {
                    let mut guard = self.mpv_child.lock().unwrap();
                    guard.as_mut().and_then(|c| c.try_wait().ok()).flatten().is_some()
                };
                if finished {
                    self.state.is_playing = false;
                    self.play_next();
                }
            }
            PlayMode::None => {}
        }
    }

    pub fn play_file(&mut self, path: &Path, title: &str, is_video: bool) -> Result<(), String> {
        self.stop();
        if is_video {
            return self.play_external(path.to_str().unwrap_or(""), title, is_video);
        }
        let (_stream, handle) =
            OutputStream::try_default().map_err(|e| format!("Аудиоустройство: {}", e))?;
        let sink = Sink::try_new(&handle).map_err(|e| format!("Sink: {}", e))?;
        let file = std::fs::File::open(path).map_err(|e| format!("Файл: {}", e))?;
        let source = Decoder::new(BufReader::new(file)).map_err(|e| format!("Декодер: {}", e))?;
        let duration = source
            .total_duration()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        sink.append(source);
        sink.set_volume(self.volume);
        sink.play();
        self._stream = Some(_stream);
        self.sink = Some(sink);
        self.mode = PlayMode::Rodio;
        self.state = PlayerState {
            title: title.to_string(),
            subtitle: "Локальный файл".into(),
            is_playing: true,
            position_secs: 0.0,
            duration_secs: duration,
            has_media: true,
            is_video: false,
        };
        Ok(())
    }

    pub fn play_url(
        &mut self,
        url: &str,
        title: &str,
        subtitle: &str,
        is_video: bool,
    ) -> Result<(), String> {
        if let Some(mpv) = Self::resolve_mpv() {
            return self
                .play_external_with(url, title, is_video, &mpv)
                .map(|_| {
                    self.state.subtitle = subtitle.to_string();
                });
        }
        let tmp = std::env::temp_dir().join(format!(
            "mp3party_stream_{}.mp3",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        ));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        let bytes = client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
            )
            .send()
            .map_err(|e| e.to_string())?
            .bytes()
            .map_err(|e| e.to_string())?;
        if bytes.len() < 50 * 1024 {
            return Err("Поток слишком короткий или недоступен".into());
        }
        std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        self.play_file(&tmp, title, false)?;
        self.state.subtitle = subtitle.to_string();
        Ok(())
    }

    pub fn add_to_playlist(&mut self, item: PlaylistItem) {
        self.playlist.push_back(item);
    }

    pub fn remove_from_playlist(&mut self, index: usize) {
        if index < self.playlist.len() {
            self.playlist.remove(index);
            if self.playlist.is_empty() {
                self.playlist_index = 0;
                self.shuffle_history.clear();
            } else if index < self.playlist_index {
                self.playlist_index -= 1;
            } else if index == self.playlist_index && self.playlist_index >= self.playlist.len() {
                self.playlist_index = self.playlist.len().saturating_sub(1);
            }
        }
    }

    pub fn clear_playlist(&mut self) {
        self.playlist.clear();
        self.playlist_index = 0;
        self.shuffle_history.clear();
    }

    pub fn set_loop_mode(&mut self, mode: LoopMode) {
        self.loop_mode = mode;
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        if self.shuffle {
            self.shuffle_history.clear();
            if !self.playlist.is_empty() {
                self.shuffle_history.push(self.playlist_index);
            }
        } else {
            self.shuffle_history.clear();
        }
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
        }
        if self.mode == PlayMode::External {
            self.send_mpv(&[
                "set",
                "volume",
                &format!("{}", (self.volume * 100.0) as u32),
            ]);
        }
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn play_next(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        if self.shuffle {
            let next = if self.playlist.len() == 1 {
                0
            } else {
                let mut rng = rand::thread_rng();
                loop {
                    let idx = rng.gen_range(0..self.playlist.len());
                    if idx != self.playlist_index || self.playlist.len() <= 1 {
                        break idx;
                    }
                }
            };
            self.playlist_index = next;
            self.shuffle_history.push(next);
        } else {
            match self.loop_mode {
                LoopMode::NoRepeat => {
                    if self.playlist_index + 1 >= self.playlist.len() {
                        self.stop();
                        return;
                    }
                    self.playlist_index += 1;
                }
                LoopMode::RepeatAll => {
                    self.playlist_index = (self.playlist_index + 1) % self.playlist.len();
                }
                LoopMode::RepeatOne => {
                    // Replay current track — do nothing to index
                }
            }
        }
        self.play_current();
    }

    pub fn play_prev(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        if self.shuffle && self.shuffle_history.len() > 1 {
            self.shuffle_history.pop();
            if let Some(&prev) = self.shuffle_history.last() {
                self.playlist_index = prev;
            }
        } else if !self.shuffle {
            if self.loop_mode == LoopMode::RepeatAll {
                if self.playlist_index == 0 {
                    self.playlist_index = self.playlist.len() - 1;
                } else {
                    self.playlist_index -= 1;
                }
            } else {
                if self.playlist_index > 0 {
                    self.playlist_index -= 1;
                }
            }
        }
        self.play_current();
    }

    pub fn play_current(&mut self) {
        let Some(item) = self.playlist.get(self.playlist_index).cloned() else {
            return;
        };
        if item.is_url {
            let _ = self.play_url(&item.path_or_url, &item.title, &item.subtitle, item.is_video);
        } else {
            let _ = self.play_file(Path::new(&item.path_or_url), &item.title, item.is_video);
        }
    }

    fn play_external(&mut self, target: &str, title: &str, is_video: bool) -> Result<(), String> {
        let mpv = Self::resolve_mpv().ok_or_else(|| "mpv не найден".to_string())?;
        self.play_external_with(target, title, is_video, &mpv)
    }

    fn play_external_with(
        &mut self,
        target: &str,
        title: &str,
        is_video: bool,
        mpv: &Path,
    ) -> Result<(), String> {
        self.stop();
        let mut args = vec![
            "--really-quiet".to_string(),
            "--no-terminal".to_string(),
            "--title".to_string(),
            title.to_string(),
        ];
        if !is_video {
            args.push("--no-video".to_string());
            args.push("--force-window=no".to_string());
        }
        args.push(format!(
            "--input-ipc-server={}",
            Self::mpv_socket_path().display()
        ));
        args.push(target.to_string());

        let child = match Command::new(mpv)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => Command::new("ffplay")
                .args(if is_video {
                    vec![target.to_string()]
                } else {
                    vec![
                        "-nodisp".to_string(),
                        "-autoexit".to_string(),
                        target.to_string(),
                    ]
                })
                .spawn()
                .map_err(|e2| format!("Нужен mpv или ffplay: {} / {}", e, e2))?,
        };

        *self.mpv_child.lock().unwrap() = Some(child);
        self.mode = PlayMode::External;

        // Apply volume to mpv
        let vol_pct = (self.volume * 100.0) as u32;
        self.send_mpv(&["set", "volume", &format!("{}", vol_pct)]);

        self.state = PlayerState {
            title: title.to_string(),
            subtitle: if is_video {
                "Видео (mpv)".into()
            } else {
                "Стрим (mpv)".into()
            },
            is_playing: true,
            position_secs: 0.0,
            duration_secs: 0.0,
            has_media: true,
            is_video,
        };
        Ok(())
    }

    pub fn mpv_install_dir() -> PathBuf {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        #[cfg(windows)]
        return home.join("mpv-util").join("windows");
        #[cfg(target_os = "macos")]
        return home.join("mpv-util").join("macos");
        #[cfg(not(any(windows, target_os = "macos")))]
        home.join("mpv-util").join("bin")
    }

    pub fn resolve_mpv() -> Option<PathBuf> {
        #[cfg(windows)]
        let bundled = Self::mpv_install_dir().join("mpv.exe");
        #[cfg(not(windows))]
        let bundled = Self::mpv_install_dir().join("mpv");
        if bundled.exists() {
            return Some(bundled);
        }

        #[cfg(windows)]
        let which_cmd = "where";
        #[cfg(not(windows))]
        let which_cmd = "which";
        if let Ok(out) = Command::new(which_cmd).arg("mpv").output() {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
        None
    }

    pub fn has_mpv() -> bool {
        Self::resolve_mpv().is_some()
    }

    pub fn install_mpv() -> Result<PathBuf, String> {
        #[cfg(target_os = "linux")]
        {
            return Err(
                "На Linux установите mpv через пакетный менеджер:\n  sudo pacman -S mpv\n  sudo apt install mpv\nhttps://mpv.io/installation/"
                    .into(),
            );
        }

        #[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
        {
            return Err(
                "Автоустановка mpv для Intel Mac недоступна. Установите: brew install mpv".into(),
            );
        }

        #[cfg(any(windows, all(target_os = "macos", target_arch = "aarch64")))]
        {
            return install_mpv_portable();
        }

        #[cfg(not(any(
            target_os = "linux",
            windows,
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        Err("Автоустановка mpv не поддерживается на этой платформе".into())
    }

    fn mpv_socket_path() -> PathBuf {
        std::env::temp_dir().join("mp3_downloader_gui_mpv.sock")
    }

    fn send_mpv(&self, args: &[&str]) {
        let Some(mpv) = Self::resolve_mpv() else {
            return;
        };
        let _ = Command::new(mpv)
            .arg(format!(
                "--input-ipc-server={}",
                Self::mpv_socket_path().display()
            ))
            .args(args)
            .status();
    }
}

static RE_SHINCHIRO_ASSET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""name":"([^"]+\.7z)"[^}]*"browser_download_url":"([^"]+)""#).unwrap()
});

#[cfg(any(windows, all(target_os = "macos", target_arch = "aarch64")))]
fn install_mpv_portable() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let prefix = "mpv-x86_64";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let prefix = "mpv-aarch64";

    let url = shinchiro_mpv_asset_url(prefix)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let bytes = client
        .get(&url)
        .send()
        .map_err(|e| format!("Скачивание mpv: {e}"))?
        .bytes()
        .map_err(|e| e.to_string())?;

    let tmp = std::env::temp_dir().join(format!(
        "mp3party_mpv_{}.7z",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    ));
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;

    let extract = std::env::temp_dir().join("mp3party_mpv_extract");
    let _ = fs::remove_dir_all(&extract);
    fs::create_dir_all(&extract).map_err(|e| e.to_string())?;
    extract_mpv_7z(&tmp, &extract)?;

    let found = find_mpv_in_tree(&extract).ok_or_else(|| "mpv не найден в архиве".to_string())?;
    let src_dir = found
        .parent()
        .ok_or_else(|| "Нет каталога mpv".to_string())?;
    let dest = AudioPlayer::mpv_install_dir();
    let _ = fs::remove_dir_all(&dest);
    copy_dir_all(src_dir, &dest).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&tmp);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&found) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&found, perms);
        }
    }

    AudioPlayer::resolve_mpv().ok_or_else(|| "mpv установлен, но не найден".to_string())
}

fn shinchiro_mpv_asset_url(prefix: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let body = client
        .get("https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())?;

    let mut fallback = None;
    for caps in RE_SHINCHIRO_ASSET.captures_iter(&body) {
        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let url = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
        if !name.starts_with(prefix) {
            continue;
        }
        if name.contains("v3") {
            return Ok(url);
        }
        if fallback.is_none() {
            fallback = Some(url);
        }
    }
    fallback.ok_or_else(|| "Сборка mpv не найдена на GitHub".to_string())
}

fn extract_mpv_7z(archive: &Path, dest: &Path) -> Result<(), String> {
    let seven_z = ["7z", "7za"]
        .iter()
        .find_map(|cmd| {
            Command::new("which")
                .arg(cmd)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .or_else(|| {
            #[cfg(windows)]
            {
                let p = PathBuf::from(r"C:\Program Files\7-Zip\7z.exe");
                if p.exists() {
                    return Some(p.to_string_lossy().into_owned());
                }
            }
            None
        })
        .ok_or_else(|| {
            "Нужен 7-Zip (7z) для распаковки mpv. Установите 7-Zip или mpv в PATH.".to_string()
        })?;

    let status = Command::new(&seven_z)
        .args([
            "x",
            &archive.to_string_lossy(),
            &format!("-o{}", dest.display()),
            "-y",
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Распаковка mpv (7z) не удалась".into());
    }
    Ok(())
}

fn find_mpv_in_tree(root: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    const NAME: &str = "mpv.exe";
    #[cfg(not(windows))]
    const NAME: &str = "mpv";

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(NAME) {
                return Some(path);
            }
        }
    }
    None
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

pub fn open_folder_in_file_manager(path: &Path) {
    let s = path.to_string_lossy();
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(&*s).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(&*s).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer").arg(&*s).spawn();
    }
}
