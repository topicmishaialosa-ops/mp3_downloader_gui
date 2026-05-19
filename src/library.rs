use std::path::{Path, PathBuf};

const MEDIA_EXT: &[&str] = &["mp3", "mp4", "m4a", "opus", "webm", "mkv", "wav", "flac"];
const VIDEO_EXT: &[&str] = &["mp4", "webm", "mkv"];

#[derive(Clone, Debug)]
pub struct LocalMedia {
    pub path: PathBuf,
    pub display_name: String,
    pub is_video: bool,
    pub size_bytes: u64,
}

pub fn list_downloads(dir: &Path) -> Vec<LocalMedia> {
    if !dir.exists() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut out: Vec<LocalMedia> = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let path = e.path();
            let ext = path.extension()?.to_str()?.to_lowercase();
            if !MEDIA_EXT.contains(&ext.as_str()) {
                return None;
            }
            let is_video = VIDEO_EXT.contains(&ext.as_str());
            let display_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file")
                .to_string();
            let size_bytes = e.metadata().ok()?.len();
            Some(LocalMedia {
                path,
                display_name,
                is_video,
                size_bytes,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        let tb = b
            .path
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let ta = a
            .path
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        tb.cmp(&ta)
    });
    out
}
