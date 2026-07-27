use std::process::Command;
use std::time::Duration;

use scraper::{Html, Selector};

use crate::types::*;
use crate::LinkParserApp;

impl LinkParserApp {
    pub fn extract_id(url: &str) -> Option<String> {
        if let Some(caps) = RE_ID_EXTRACT.captures(url) {
            return caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str().to_string());
        }
        None
    }

    pub fn fetch_track_info(id: &str) -> Result<TrackInfo, String> {
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

    pub fn parse_track_page(body: &str, id: &str) -> Result<TrackInfo, String> {
        let document = Html::parse_document(body);
        let mut artist = String::new();
        let mut title = String::new();

        let panel_sel = Selector::parse("div.track__user-panel").unwrap();
        if let Some(el) = document.select(&panel_sel).next() {
            if let Some(a) = el.value().attr("data-js-artist-name") {
                artist = a.to_string();
            }
            if let Some(t) = el.value().attr("data-js-song-title") {
                title = t.to_string();
            }
        }

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

    pub fn search_tracks(query: &str) -> Result<Vec<TrackInfo>, String> {
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

        let item_sel = Selector::parse("div.track.song-item").unwrap();
        let panel_sel = Selector::parse("div.track__user-panel").unwrap();

        for item in document.select(&item_sel) {
            if let Some(panel) = item.select(&panel_sel).next() {
                Self::extract_from_panel(&panel, &mut results);
            }
        }

        if results.is_empty() {
            for panel in document.select(&panel_sel) {
                Self::extract_from_panel(&panel, &mut results);
            }
        }

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

    pub fn split_ytdlp_title(full: &str, channel: &str) -> (String, String) {
        if let Some((a, t)) = full.split_once(" - ") {
            return (a.trim().to_string(), t.trim().to_string());
        }
        if !channel.is_empty() && channel != full {
            return (channel.trim().to_string(), full.trim().to_string());
        }
        ("YouTube".to_string(), full.trim().to_string())
    }

    pub fn search_tracks_ytdlp(query: &str) -> Result<Vec<TrackInfo>, String> {
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

    pub fn extract_from_panel(panel: &scraper::ElementRef, results: &mut Vec<TrackInfo>) {
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

    pub fn mp3party_stream_url(id: &str) -> String {
        format!("https://dl2.mp3party.net/online/{}.mp3", id)
    }

    pub fn mp3party_download_url(id: &str) -> String {
        format!("https://dl2.mp3party.net/download/{}", id)
    }

    pub fn mp3party_client() -> Result<reqwest::blocking::Client, String> {
        reqwest::blocking::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(8))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("Ошибка клиента: {}", e))
    }

    pub fn mp3party_request<'a>(
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

    pub fn mp3party_download_candidates(page_body: &str, track: &TrackInfo) -> Vec<String> {
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

    pub fn is_mp3party_error_body(bytes: &[u8]) -> bool {
        if bytes.len() > 512 {
            return false;
        }
        let preview = String::from_utf8_lossy(bytes);
        preview.contains("failed to get file") || preview.contains("nil")
    }

    pub fn drivemusic_client() -> Result<reqwest::blocking::Client, String> {
        reqwest::blocking::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(8))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("Ошибка клиента: {}", e))
    }

    pub fn drivemusic_page_url(track: &TrackInfo) -> Result<String, String> {
        let u = track.url.trim();
        if u.contains("drivemusic.me") && u.ends_with(".html") {
            return Ok(u.to_string());
        }
        if u.starts_with('/') && u.ends_with(".html") {
            return Ok(format!("{}{}", DRIVEMUSIC_BASE, u));
        }
        Err("DriveMusic: нет ссылки на страницу трека — найдите трек через поиск.".into())
    }

    pub fn drivemusic_extract_mp3_urls(html: &str) -> Vec<String> {
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

    pub fn drivemusic_download_candidates(page_body: &str, _track: &TrackInfo) -> Vec<String> {
        Self::drivemusic_extract_mp3_urls(page_body)
    }

    pub fn search_tracks_drivemusic(query: &str) -> Result<Vec<TrackInfo>, String> {
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

    pub fn pesnime_search_url(query: &str) -> String {
        format!("{}/search/{}?type=tracks", PESNIME_BASE, urlencoding::encode(query))
    }

    pub fn pesnime_track_url(id: &str) -> String {
        format!("{}/track/{}", PESNIME_BASE, id)
    }

    pub fn pesnime_client() -> Result<reqwest::blocking::Client, String> {
        reqwest::blocking::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(8))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("Ошибка клиента: {}", e))
    }

    pub fn pesnime_extract_tracks(body: &str) -> Vec<TrackInfo> {
        let mut results: Vec<TrackInfo> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for caps in RE_PESNIME_TRACK.captures_iter(body) {
            let id = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let artist = caps.get(2).map(|m| Self::unescape_json(m.as_str())).unwrap_or_default();
            let title = caps.get(3).map(|m| Self::unescape_json(m.as_str())).unwrap_or_default();
            let play_url = caps.get(7).map(|m| m.as_str()).unwrap_or("");
            let download_url = caps.get(8).map(|m| m.as_str()).unwrap_or("");
            if id.is_empty() || title.is_empty() || !seen.insert(id.to_string()) {
                continue;
            }
            let url = if !download_url.is_empty() {
                download_url.to_string()
            } else if !play_url.is_empty() {
                play_url.to_string()
            } else {
                Self::pesnime_track_url(id)
            };
            results.push(TrackInfo {
                id: id.to_string(),
                artist,
                title,
                url,
            });
        }
        results
    }

    pub fn search_tracks_pesnime(query: &str) -> Result<Vec<TrackInfo>, String> {
        let url = Self::pesnime_search_url(query);

        let client = Self::pesnime_client()?;
        let resp = client
            .get(&url)
            .header("User-Agent", BROWSER_USER_AGENT)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .send()
            .map_err(|e| format!("Ошибка запроса: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {} при поиске Pesni.me", resp.status()));
        }

        let body = resp.text().map_err(|e| format!("Ошибка чтения: {}", e))?;
        let results = Self::pesnime_extract_tracks(&body);

        let words: Vec<String> = query.split_whitespace()
            .map(|w| w.to_lowercase()).collect();
        let filter = |t: &TrackInfo| -> bool {
            let al = t.artist.to_lowercase();
            let tl = t.title.to_lowercase();
            words.iter().any(|w| al.starts_with(w) || tl.starts_with(w))
        };

        let matched: Vec<TrackInfo> = results.iter().filter(|t| filter(t)).cloned().take(30).collect();
        if !matched.is_empty() {
            return Ok(matched);
        }

        if results.is_empty() {
            let url2 = format!("https://pesni.me/search/{}", urlencoding::encode(query));
            if let Ok(resp2) = client
                .get(&url2)
                .header("User-Agent", BROWSER_USER_AGENT)
                .send()
            {
                if resp2.status().is_success() {
                    if let Ok(body2) = resp2.text() {
                        let results2 = Self::pesnime_extract_tracks(&body2);
                        let matched2: Vec<TrackInfo> = results2.into_iter().filter(|t| filter(t)).take(30).collect();
                        if !matched2.is_empty() {
                            return Ok(matched2);
                        }
                    }
                }
            }
            Err(format!("Ничего не найдено на Pesni.me по запросу «{}».", query))
        } else {
            Ok(results.into_iter().take(30).collect())
        }
    }

    pub fn fetch_track_info_pesnime(id: &str) -> Result<TrackInfo, String> {
        let url = Self::pesnime_track_url(id);

        let client = Self::pesnime_client()?;
        let resp = client
            .get(&url)
            .header("User-Agent", BROWSER_USER_AGENT)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .send()
            .map_err(|e| format!("Ошибка запроса: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {} для {}", resp.status(), url));
        }

        let body = resp.text().map_err(|e| format!("Ошибка чтения: {}", e))?;
        let results = Self::pesnime_extract_tracks(&body);

        results.into_iter().next().ok_or_else(|| {
            format!("Не удалось распознать трек Pesni.me ID {}", id)
        })
    }

    pub fn unescape_json(s: &str) -> String {
        s.replace("\\\"", "\"").replace("\\n", "\n").replace("\\t", "\t")
    }
}
