//! Пакетный (batch) поиск треков.
//!
//! Формат ввода — обычный многострочный текст, по треку на строку:
//!   Исполнитель - Название       (дефис, en-dash или em-dash)
//!   Просто название                (если разделителя нет)
//!   https://example.com/track/123  (если URL — качаем напрямую)
//!
//! Допускаются нумерованные списки (`1. ...`, `12) ...`) и `#`-комментарии.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchQuery {
    pub raw: String,
    pub artist: String,
    pub title: String,
    pub url: Option<String>,
}

impl BatchQuery {
    /// Полная поисковая строка для API.
    pub fn search_text(&self) -> String {
        match (&self.artist[..], &self.title[..]) {
            ("", "") => self.raw.clone(),
            ("", _) => self.title.clone(),
            (_, "") => self.artist.clone(),
            _ => format!("{} - {}", self.artist, self.title),
        }
    }
}

/// Разобрать многострочный текст на список запросов.
pub fn parse_batch_queries(input: &str) -> Vec<BatchQuery> {
    let mut out = Vec::new();
    for line in input.lines() {
        let stripped = strip_numbering(line);
        let no_comment = strip_trailing_comment(&stripped);
        let trimmed = no_comment.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        out.push(parse_single(trimmed));
    }
    out
}

fn strip_numbering(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let start_digits = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == start_digits {
        return s.to_string();
    }
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= bytes.len() || (bytes[i] != b'.' && bytes[i] != b')') {
        return s.to_string();
    }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    s[i..].to_string()
}

fn strip_trailing_comment(s: &str) -> String {
    if let Some(idx) = s.find(" #") {
        return s[..idx].to_string();
    }
    s.to_string()
}

fn parse_single(line: &str) -> BatchQuery {
    if is_url(line) {
        return BatchQuery {
            raw: line.to_string(),
            artist: String::new(),
            title: String::new(),
            url: Some(line.to_string()),
        };
    }
    let seps = [" - ", " \u{2013} ", " \u{2014} "];
    for sep in seps {
        if let Some(idx) = line.find(sep) {
            return BatchQuery {
                raw: line.to_string(),
                artist: line[..idx].trim().to_string(),
                title: line[idx + sep.len()..].trim().to_string(),
                url: None,
            };
        }
    }
    BatchQuery {
        raw: line.to_string(),
        artist: String::new(),
        title: line.trim().to_string(),
        url: None,
    }
}

fn is_url(s: &str) -> bool {
    let lower = s.trim_start().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dash_separated() {
        let qs = parse_batch_queries("Кино - Группа крови\nКороль и Лев - Circle of Life");
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[0].artist, "Кино");
        assert_eq!(qs[0].title, "Группа крови");
        assert_eq!(qs[1].artist, "Король и Лев");
        assert_eq!(qs[1].title, "Circle of Life");
    }

    #[test]
    fn parses_em_dash_and_en_dash() {
        let qs = parse_batch_queries("A \u{2014} B\nC \u{2013} D");
        assert_eq!(qs[0].artist, "A");
        assert_eq!(qs[0].title, "B");
        assert_eq!(qs[1].artist, "C");
        assert_eq!(qs[1].title, "D");
    }

    #[test]
    fn parses_title_only() {
        let qs = parse_batch_queries("Yesterday");
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].artist, "");
        assert_eq!(qs[0].title, "Yesterday");
    }

    #[test]
    fn parses_url() {
        let qs = parse_batch_queries("https://example.com/track/123");
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].url.as_deref(), Some("https://example.com/track/123"));
    }

    #[test]
    fn skips_empty_and_comments() {
        let qs = parse_batch_queries("\n# комментарий\nКино - Звезда\n   \n# ещё\nA - B");
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[0].title, "Звезда");
        assert_eq!(qs[1].title, "B");
    }

    #[test]
    fn strips_numbering() {
        let qs = parse_batch_queries("1. Кино - Звезда\n12) A - B\n3)A - C");
        assert_eq!(qs.len(), 3);
        assert_eq!(qs[0].title, "Звезда");
        assert_eq!(qs[1].title, "B");
        assert_eq!(qs[2].title, "C");
    }

    #[test]
    fn search_text_builds_query() {
        let q = BatchQuery {
            raw: String::new(),
            artist: "Кино".into(),
            title: "Звезда".into(),
            url: None,
        };
        assert_eq!(q.search_text(), "Кино - Звезда");
    }
}
