use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{Decoder, OutputStream, Sink, Source};

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
                self.send_mpv(&[
                    "seek",
                    &format!("{:.2}", s),
                    "absolute",
                ]);
                self.state.position_secs = s;
            }
            PlayMode::Rodio => {
                // rodio не поддерживает seek — перезапуск через mpv если есть
                self.state.position_secs = s;
            }
            PlayMode::None => {}
        }
    }

    pub fn tick(&mut self) {
        if self.mode == PlayMode::Rodio {
            if let Some(sink) = &self.sink {
                self.state.position_secs = sink.get_pos().as_secs_f64();
                if sink.empty() && self.state.is_playing {
                    self.state.is_playing = false;
                }
            }
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
        let file =
            std::fs::File::open(path).map_err(|e| format!("Файл: {}", e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Декодер: {}", e))?;
        let duration = source
            .total_duration()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        sink.append(source);
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

    pub fn play_url(&mut self, url: &str, title: &str, subtitle: &str, is_video: bool) -> Result<(), String> {
        if Self::has_mpv() {
            return self.play_external(url, title, is_video).map(|_| {
                self.state.subtitle = subtitle.to_string();
            });
        }
        // fallback: скачать во временный файл и rodio
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

    fn play_external(&mut self, target: &str, title: &str, is_video: bool) -> Result<(), String> {
        self.stop();
        let mut args = vec![
            "--really-quiet".to_string(),
            "--no-terminal".to_string(),
            "--title".to_string(),
            title.to_string(),
        ];
        if !is_video {
            args.push("--no-video".to_string());
        }
        args.push(format!(
            "--input-ipc-server={}",
            Self::mpv_socket_path().display()
        ));
        args.push(target.to_string());

        let child = match Command::new("mpv")
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

    fn has_mpv() -> bool {
        Command::new("which")
            .arg("mpv")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn mpv_socket_path() -> PathBuf {
        std::env::temp_dir().join("mp3_downloader_gui_mpv.sock")
    }

    fn send_mpv(&self, args: &[&str]) {
        let _ = Command::new("mpv")
            .arg(format!(
                "--input-ipc-server={}",
                Self::mpv_socket_path().display()
            ))
            .args(args)
            .status();
    }
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
