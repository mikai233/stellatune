use std::time::Duration;

use anyhow::{Context, Error, Result, anyhow};
use reqwest::StatusCode;
use serde_json::Value;
use url::Url;

use crate::{LyricsDoc, LyricsQuery, LyricsSearchCandidate};

use super::actor::{LyricsServiceCore, SourceHealth};
use super::parser::{
    candidate_from_lrclib_value, doc_from_lrclib_value, is_retriable_error, is_retriable_status,
    json_string, parse_plain, parse_retry_after_ms, retry_backoff_ms, unix_now_ms,
};

const HTTP_TIMEOUT_MS: u64 = 7_000;
const HTTP_RETRY_MAX_ATTEMPTS: usize = 3;
const HTTP_MIN_REQUEST_INTERVAL_MS: i64 = 180;
const SOURCE_COOLDOWN_MS: i64 = 5 * 60 * 1_000;
const SOURCE_FAILURE_THRESHOLD: u32 = 3;
const SOURCE_LRCLIB: &str = "lrclib";
const SOURCE_LYRICS_OVH: &str = "lyrics_ovh";

impl LyricsServiceCore {
    pub(super) async fn fetch_online(&self, query: &LyricsQuery) -> Result<Option<LyricsDoc>> {
        let mut had_network_error = false;

        if self.source_available(SOURCE_LRCLIB) {
            match self.fetch_lrclib_get(query).await {
                Ok(Some(doc)) => {
                    self.mark_source_success(SOURCE_LRCLIB);
                    return Ok(Some(doc));
                },
                Ok(None) => {
                    self.mark_source_success(SOURCE_LRCLIB);
                    match self.fetch_lrclib_search(query).await {
                        Ok(Some(doc)) => return Ok(Some(doc)),
                        Ok(None) => {},
                        Err(err) => {
                            had_network_error = true;
                            self.mark_source_failure(SOURCE_LRCLIB);
                            tracing::warn!("lyrics source lrclib search failed: {err}");
                        },
                    }
                },
                Err(err) => {
                    had_network_error = true;
                    self.mark_source_failure(SOURCE_LRCLIB);
                    tracing::warn!("lyrics source lrclib get failed: {err}");
                },
            }
        }

        if self.source_available(SOURCE_LYRICS_OVH) {
            match self.fetch_lyrics_ovh_doc(query).await {
                Ok(Some(doc)) => {
                    self.mark_source_success(SOURCE_LYRICS_OVH);
                    return Ok(Some(doc));
                },
                Ok(None) => {
                    self.mark_source_success(SOURCE_LYRICS_OVH);
                },
                Err(err) => {
                    had_network_error = true;
                    self.mark_source_failure(SOURCE_LYRICS_OVH);
                    tracing::warn!("lyrics source lyrics.ovh failed: {err}");
                },
            }
        }

        if had_network_error {
            return Err(anyhow!("all online lyrics sources failed"));
        }
        Ok(None)
    }

    pub(super) async fn search_candidates_lrclib(
        &self,
        query: &LyricsQuery,
    ) -> Result<Vec<LyricsSearchCandidate>> {
        if !self.source_available(SOURCE_LRCLIB) {
            return Ok(Vec::new());
        }

        let mut url =
            Url::parse("https://lrclib.net/api/search").expect("valid lrclib search endpoint");
        let q = match query.artist.as_ref() {
            Some(artist) if !artist.is_empty() => format!("{} {}", query.title, artist),
            _ => query.title.clone(),
        };
        url.query_pairs_mut().append_pair("q", &q);

        let value = self
            .http_get_json_with_retry(url, "lrclib search candidates", false, SOURCE_LRCLIB)
            .await?
            .unwrap_or(Value::Null);
        let Some(items) = value.as_array() else {
            return Ok(Vec::new());
        };

        let mut out = Vec::new();
        for (idx, item) in items.iter().enumerate() {
            if let Some(c) = candidate_from_lrclib_value(&query.track_key, item, idx) {
                out.push(c);
            }
        }
        Ok(out)
    }

    pub(super) async fn candidate_from_lyrics_ovh(
        &self,
        query: &LyricsQuery,
    ) -> Result<Option<LyricsSearchCandidate>> {
        if !self.source_available(SOURCE_LYRICS_OVH) {
            return Ok(None);
        }
        let Some(doc) = self.fetch_lyrics_ovh_doc(query).await? else {
            return Ok(None);
        };

        let title = query.title.clone();
        let artist = query.artist.clone();
        let preview = doc
            .lines
            .iter()
            .map(|l| l.text.trim())
            .find(|line| !line.is_empty())
            .map(str::to_string);
        let candidate_id = format!(
            "{}:{}:{}",
            SOURCE_LYRICS_OVH,
            artist.as_deref().unwrap_or(""),
            title
        );

        Ok(Some(LyricsSearchCandidate {
            candidate_id,
            title,
            artist,
            album: query.album.clone(),
            source: SOURCE_LYRICS_OVH.to_string(),
            is_synced: false,
            preview,
            doc,
        }))
    }

    pub(super) async fn fetch_lrclib_get(&self, query: &LyricsQuery) -> Result<Option<LyricsDoc>> {
        let mut url = Url::parse("https://lrclib.net/api/get").expect("valid lrclib get url");
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("track_name", &query.title);
            if let Some(artist) = query.artist.as_ref() {
                qp.append_pair("artist_name", artist);
            }
            if let Some(album) = query.album.as_ref() {
                qp.append_pair("album_name", album);
            }
            if let Some(duration_ms) = query.duration_ms.filter(|&d| d > 0) {
                qp.append_pair("duration", &(duration_ms / 1000).to_string());
            }
        }

        let value = self
            .http_get_json_with_retry(url, "lrclib get", true, SOURCE_LRCLIB)
            .await?;
        Ok(value.and_then(|v| doc_from_lrclib_value(&query.track_key, &v)))
    }

    pub(super) async fn fetch_lrclib_search(
        &self,
        query: &LyricsQuery,
    ) -> Result<Option<LyricsDoc>> {
        let mut url =
            Url::parse("https://lrclib.net/api/search").expect("valid lrclib search endpoint");
        let q = match query.artist.as_ref() {
            Some(artist) if !artist.is_empty() => format!("{} {}", query.title, artist),
            _ => query.title.clone(),
        };
        url.query_pairs_mut().append_pair("q", &q);

        let value = self
            .http_get_json_with_retry(url, "lrclib search", false, SOURCE_LRCLIB)
            .await?
            .unwrap_or(Value::Null);
        let Some(items) = value.as_array() else {
            return Ok(None);
        };

        for item in items {
            if let Some(doc) = doc_from_lrclib_value(&query.track_key, item) {
                return Ok(Some(doc));
            }
        }
        Ok(None)
    }

    pub(super) async fn fetch_lyrics_ovh_doc(
        &self,
        query: &LyricsQuery,
    ) -> Result<Option<LyricsDoc>> {
        let Some(artist) = query.artist.as_ref().filter(|v| !v.is_empty()) else {
            return Ok(None);
        };

        let mut url = Url::parse("https://api.lyrics.ovh/v1/").expect("valid lyrics.ovh url");
        {
            let mut segs = url
                .path_segments_mut()
                .map_err(|_| anyhow!("lyrics.ovh url is not base"))?;
            segs.push(artist);
            segs.push(&query.title);
        }

        let value = self
            .http_get_json_with_retry(url, "lyrics.ovh get", true, SOURCE_LYRICS_OVH)
            .await?;
        let Some(value) = value else {
            return Ok(None);
        };
        let Some(plain) = json_string(&value, &["lyrics"]) else {
            return Ok(None);
        };

        Ok(parse_plain(
            &query.track_key,
            SOURCE_LYRICS_OVH,
            plain.trim(),
        ))
    }

    pub(super) fn source_available(&self, source: &'static str) -> bool {
        let now_ms = unix_now_ms();
        let health = self
            .source_health
            .lock()
            .expect("lyrics source health mutex poisoned");
        match health.get(source) {
            Some(state) => now_ms >= state.blocked_until_ms,
            None => true,
        }
    }

    pub(super) fn mark_source_success(&self, source: &'static str) {
        let mut health = self
            .source_health
            .lock()
            .expect("lyrics source health mutex poisoned");
        health.insert(source, SourceHealth::default());
    }

    pub(super) fn mark_source_failure(&self, source: &'static str) {
        let now_ms = unix_now_ms();
        let mut health = self
            .source_health
            .lock()
            .expect("lyrics source health mutex poisoned");
        let entry = health.entry(source).or_default();
        entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
        if entry.consecutive_failures >= SOURCE_FAILURE_THRESHOLD {
            entry.blocked_until_ms = now_ms.saturating_add(SOURCE_COOLDOWN_MS);
            tracing::warn!(
                "lyrics source {} is temporarily blocked for {}ms after {} consecutive failures",
                source,
                SOURCE_COOLDOWN_MS,
                entry.consecutive_failures
            );
        }
    }

    pub(super) async fn wait_rate_limit_slot(&self) {
        let wait_ms = {
            let mut rate = self
                .http_rate
                .lock()
                .expect("lyrics http rate mutex poisoned");
            let now_ms = unix_now_ms();
            let wait_ms = (rate.next_allowed_at_ms - now_ms).max(0);
            let base = now_ms.max(rate.next_allowed_at_ms);
            rate.next_allowed_at_ms = base.saturating_add(HTTP_MIN_REQUEST_INTERVAL_MS);
            wait_ms
        };
        if wait_ms > 0 {
            tokio::time::sleep(Duration::from_millis(wait_ms as u64)).await;
        }
    }

    pub(super) async fn http_get_json_with_retry(
        &self,
        url: Url,
        op_name: &str,
        allow_not_found: bool,
        source: &'static str,
    ) -> Result<Option<Value>> {
        let mut last_error: Option<Error> = None;

        for attempt in 1..=HTTP_RETRY_MAX_ATTEMPTS {
            self.wait_rate_limit_slot().await;

            let response = self
                .client
                .get(url.clone())
                .timeout(Duration::from_millis(HTTP_TIMEOUT_MS))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if allow_not_found && status == StatusCode::NOT_FOUND {
                        self.mark_source_success(source);
                        return Ok(None);
                    }
                    if status.is_success() {
                        let body = resp
                            .text()
                            .await
                            .with_context(|| format!("{} response body read failed", op_name))?;
                        let value: Value = serde_json::from_str(&body)
                            .with_context(|| format!("{} response json parsing failed", op_name))?;
                        self.mark_source_success(source);
                        return Ok(Some(value));
                    }

                    let retriable = is_retriable_status(status);
                    if retriable && attempt < HTTP_RETRY_MAX_ATTEMPTS {
                        let retry_delay_ms = parse_retry_after_ms(resp.headers())
                            .unwrap_or_else(|| retry_backoff_ms(attempt));
                        tokio::time::sleep(Duration::from_millis(retry_delay_ms as u64)).await;
                        continue;
                    }

                    let err = anyhow!("{} failed with status {}", op_name, status);
                    last_error = Some(err);
                    break;
                },
                Err(err) => {
                    let retriable = is_retriable_error(&err);
                    if retriable && attempt < HTTP_RETRY_MAX_ATTEMPTS {
                        let retry_delay_ms = retry_backoff_ms(attempt);
                        tokio::time::sleep(Duration::from_millis(retry_delay_ms as u64)).await;
                        continue;
                    }
                    last_error = Some(anyhow!("{} request failed: {}", op_name, err));
                    break;
                },
            }
        }

        self.mark_source_failure(source);
        Err(last_error.unwrap_or_else(|| anyhow!("{} failed", op_name)))
    }
}
