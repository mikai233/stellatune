use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::StatusCode;
use serde_json::Value;

use crate::{LyricLine, LyricsDoc, LyricsQuery, LyricsSearchCandidate};

const HTTP_RETRY_BASE_BACKOFF_MS: i64 = 300;
const HTTP_RETRY_MAX_BACKOFF_MS: i64 = 2_000;
pub(super) fn unix_now_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(_) => 0,
    }
}

pub(super) async fn load_local_lrc_doc_async(track_key: String) -> Option<LyricsDoc> {
    match tokio::task::spawn_blocking(move || load_local_lrc_doc_blocking(&track_key)).await {
        Ok(doc) => doc,
        Err(err) => {
            tracing::warn!("load local lrc task failed: {err}");
            None
        },
    }
}

pub(super) fn load_local_lrc_doc_blocking(track_key: &str) -> Option<LyricsDoc> {
    let track_path = Path::new(track_key);
    if !track_path.exists() || !track_path.is_file() {
        return None;
    }

    let primary = track_path.with_extension("lrc");
    if let Some(doc) = read_and_parse_lrc(&primary, track_key) {
        return Some(doc);
    }

    let parent = track_path.parent()?;
    let stem = track_path.file_stem()?.to_string_lossy().to_string();
    let entries = fs::read_dir(parent).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let ext = p.extension().and_then(|v| v.to_str()).unwrap_or_default();
        if !ext.eq_ignore_ascii_case("lrc") {
            continue;
        }
        let file_stem = p
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_string();
        if file_stem.eq_ignore_ascii_case(&stem)
            && let Some(doc) = read_and_parse_lrc(&p, track_key)
        {
            return Some(doc);
        }
    }
    None
}

pub(super) fn read_and_parse_lrc(path: &Path, track_key: &str) -> Option<LyricsDoc> {
    let content = fs::read_to_string(path).ok()?;
    parse_lrc(track_key, "local_lrc", &content)
        .or_else(|| parse_plain(track_key, "local_lrc", content.trim()))
}

pub(super) fn normalize_query(query: LyricsQuery) -> LyricsQuery {
    LyricsQuery {
        track_key: query.track_key.trim().to_string(),
        title: query.title.trim().to_string(),
        artist: trim_to_option(query.artist),
        album: trim_to_option(query.album),
        duration_ms: query.duration_ms,
    }
}

pub(super) fn trim_to_option(input: Option<String>) -> Option<String> {
    input
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(super) fn find_line_index(lines: &[LyricLine], position_ms: i64) -> i64 {
    let mut idx = -1_i64;
    for (i, line) in lines.iter().enumerate() {
        match line.start_ms {
            Some(start_ms) if position_ms >= start_ms => {
                idx = i as i64;
            },
            Some(_) => break,
            None => {},
        }
    }
    idx
}

pub(super) fn doc_from_lrclib_value(track_key: &str, value: &Value) -> Option<LyricsDoc> {
    let synced = value
        .get("syncedLyrics")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if !synced.is_empty()
        && let Some(doc) = parse_lrc(track_key, "lrclib", synced)
    {
        return Some(doc);
    }

    let plain = value
        .get("plainLyrics")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if plain.is_empty() {
        return None;
    }
    parse_plain(track_key, "lrclib", plain)
}

pub(super) fn candidate_from_lrclib_value(
    track_key: &str,
    value: &Value,
    index: usize,
) -> Option<LyricsSearchCandidate> {
    let doc = doc_from_lrclib_value(track_key, value)?;
    let title = json_string(value, &["trackName", "track_name", "name"])
        .unwrap_or_else(|| "Unknown title".to_string());
    let artist = json_opt_string(value, &["artistName", "artist_name", "artist"]);
    let album = json_opt_string(value, &["albumName", "album_name", "album"]);
    let candidate_id = value
        .get("id")
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else {
                v.as_i64().map(|n| n.to_string())
            }
        })
        .unwrap_or_else(|| format!("lrclib-{}-{index}", title));
    let preview = doc
        .lines
        .iter()
        .map(|l| l.text.trim())
        .find(|t| !t.is_empty())
        .map(|s| s.to_string());

    Some(LyricsSearchCandidate {
        candidate_id,
        title,
        artist,
        album,
        source: "lrclib".to_string(),
        is_synced: doc.is_synced,
        preview,
        doc,
    })
}

pub(super) fn push_candidate_unique(
    out: &mut Vec<LyricsSearchCandidate>,
    seen: &mut HashSet<String>,
    item: LyricsSearchCandidate,
) {
    let dedup_key = format!(
        "{}|{}|{}",
        item.source,
        item.title.to_lowercase(),
        item.artist.as_deref().unwrap_or_default().to_lowercase()
    );
    if seen.insert(dedup_key) {
        out.push(item);
    }
}

pub(super) fn is_retriable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
            | StatusCode::INTERNAL_SERVER_ERROR
    )
}

pub(super) fn is_retriable_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request() || err.is_body()
}

pub(super) fn retry_backoff_ms(attempt: usize) -> i64 {
    let exp = (attempt.saturating_sub(1)).min(4);
    let factor = 1_i64 << exp;
    (HTTP_RETRY_BASE_BACKOFF_MS * factor).min(HTTP_RETRY_MAX_BACKOFF_MS)
}

pub(super) fn parse_retry_after_ms(headers: &reqwest::header::HeaderMap) -> Option<i64> {
    let raw = headers.get(reqwest::header::RETRY_AFTER)?;
    let text = raw.to_str().ok()?.trim();
    if text.is_empty() {
        return None;
    }
    let secs: i64 = text.parse().ok()?;
    Some((secs * 1_000).clamp(0, HTTP_RETRY_MAX_BACKOFF_MS))
}

pub(super) fn json_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = value.get(*key).and_then(Value::as_str) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

pub(super) fn json_opt_string(value: &Value, keys: &[&str]) -> Option<String> {
    json_string(value, keys)
}

pub(super) fn parse_plain(track_key: &str, source: &str, plain: &str) -> Option<LyricsDoc> {
    let lines: Vec<LyricLine> = plain
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|text| LyricLine {
            start_ms: None,
            end_ms: None,
            text: text.to_string(),
        })
        .collect();

    if lines.is_empty() {
        return None;
    }

    Some(LyricsDoc {
        track_key: track_key.to_string(),
        source: source.to_string(),
        is_synced: false,
        lines,
    })
}

pub(super) fn parse_lrc(track_key: &str, source: &str, raw: &str) -> Option<LyricsDoc> {
    let mut lines: Vec<LyricLine> = Vec::new();

    for row in raw.lines() {
        let mut s = row.trim();
        if s.is_empty() {
            continue;
        }

        let mut timestamps = Vec::new();
        while s.starts_with('[') {
            let Some(end_idx) = s.find(']') else {
                break;
            };
            let tag = &s[1..end_idx];
            if let Some(ms) = parse_timestamp_tag(tag) {
                timestamps.push(ms);
            }
            s = s[end_idx + 1..].trim_start();
        }

        if timestamps.is_empty() {
            continue;
        }
        if s.is_empty() {
            continue;
        }

        for start_ms in timestamps {
            lines.push(LyricLine {
                start_ms: Some(start_ms),
                end_ms: None,
                text: s.to_string(),
            });
        }
    }

    if lines.is_empty() {
        return None;
    }

    lines.sort_by_key(|l| l.start_ms.unwrap_or(i64::MAX));
    for i in 0..lines.len().saturating_sub(1) {
        let next_start = lines[i + 1].start_ms;
        lines[i].end_ms = next_start;
    }

    Some(LyricsDoc {
        track_key: track_key.to_string(),
        source: source.to_string(),
        is_synced: true,
        lines,
    })
}

pub(super) fn parse_timestamp_tag(tag: &str) -> Option<i64> {
    // Supports mm:ss, mm:ss.xx and mm:ss.xxx.
    let (min_part, sec_part) = tag.split_once(':')?;
    let minutes: i64 = min_part.parse().ok()?;

    let (sec_str, frac_str) = match sec_part.split_once('.') {
        Some((sec, frac)) => (sec, frac),
        None => (sec_part, ""),
    };
    let seconds: i64 = sec_str.parse().ok()?;
    if !(0..60).contains(&seconds) {
        return None;
    }

    let mut frac_digits = frac_str.chars().take(3).collect::<String>();
    while frac_digits.len() < 3 {
        frac_digits.push('0');
    }
    let frac_ms: i64 = if frac_digits.is_empty() {
        0
    } else {
        frac_digits.parse().ok()?
    };

    Some(minutes * 60_000 + seconds * 1_000 + frac_ms)
}
