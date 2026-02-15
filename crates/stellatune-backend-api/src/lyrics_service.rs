use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use arc_swap::ArcSwapOption;
use reqwest::StatusCode;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{Connection, Row, SqliteConnection};
use stellatune_runtime as global_runtime;
use stellatune_runtime::tokio_actor::ActorRef;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{LyricLine, LyricsDoc, LyricsEvent, LyricsQuery, LyricsSearchCandidate};

mod handlers;

use self::handlers::apply_candidate::ApplyCandidateMessage;
use self::handlers::clear_cache::ClearCacheMessage;
use self::handlers::prefetch::PrefetchMessage;
use self::handlers::prepare::PrepareMessage;
use self::handlers::refresh_current::RefreshCurrentMessage;
use self::handlers::search_candidates::SearchCandidatesMessage;
use self::handlers::set_cache_db_path::SetCacheDbPathMessage;
use self::handlers::set_position_ms::SetPositionMsMessage;

struct LyricsEventHub {
    tx: broadcast::Sender<LyricsEvent>,
}

impl Default for LyricsEventHub {
    fn default() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }
}

impl LyricsEventHub {
    fn subscribe(&self) -> broadcast::Receiver<LyricsEvent> {
        self.tx.subscribe()
    }

    fn emit(&self, event: LyricsEvent) {
        let _ = self.tx.send(event);
    }
}

#[derive(Default)]
struct LyricsState {
    current_track_key: Option<String>,
    current_query: Option<LyricsQuery>,
    current_doc: Option<LyricsDoc>,
    current_line_index: i64,
    cache: HashMap<String, LyricsDoc>,
}

const CACHE_TTL_MS: i64 = 30_i64 * 24 * 60 * 60 * 1000;
const HTTP_TIMEOUT_MS: u64 = 7_000;
const HTTP_RETRY_MAX_ATTEMPTS: usize = 3;
const HTTP_RETRY_BASE_BACKOFF_MS: i64 = 300;
const HTTP_RETRY_MAX_BACKOFF_MS: i64 = 2_000;
const HTTP_MIN_REQUEST_INTERVAL_MS: i64 = 180;
const SOURCE_COOLDOWN_MS: i64 = 5 * 60 * 1_000;
const SOURCE_FAILURE_THRESHOLD: u32 = 3;
const SOURCE_LRCLIB: &str = "lrclib";
const SOURCE_LYRICS_OVH: &str = "lyrics_ovh";
const LYRICS_ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Default)]
struct HttpRateState {
    next_allowed_at_ms: i64,
}

#[derive(Default, Clone, Copy)]
struct SourceHealth {
    consecutive_failures: u32,
    blocked_until_ms: i64,
}

#[derive(Default)]
struct ActiveFetchState {
    latest_id: u64,
    token: Option<CancellationToken>,
}

struct LyricsServiceCore {
    hub: LyricsEventHub,
    state: Mutex<LyricsState>,
    client: reqwest::Client,
    cache_db_path: ArcSwapOption<PathBuf>,
    http_rate: Mutex<HttpRateState>,
    source_health: Mutex<HashMap<&'static str, SourceHealth>>,
    active_fetch: Mutex<ActiveFetchState>,
}

struct LyricsServiceActor {
    core: Arc<LyricsServiceCore>,
}

pub struct LyricsService {
    core: Arc<LyricsServiceCore>,
    actor_ref: ActorRef<LyricsServiceActor>,
}

impl LyricsService {
    pub fn new() -> Arc<Self> {
        let core = Arc::new(LyricsServiceCore::new());
        let (actor_ref, _join) = stellatune_runtime::tokio_actor::spawn_actor(LyricsServiceActor {
            core: Arc::clone(&core),
        });
        Arc::new(Self { core, actor_ref })
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LyricsEvent> {
        self.core.subscribe_events()
    }

    pub async fn set_cache_db_path(&self, db_path: String) -> Result<()> {
        match self
            .actor_ref
            .call(SetCacheDbPathMessage { db_path }, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn clear_cache(&self) -> Result<()> {
        match self
            .actor_ref
            .call(ClearCacheMessage, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn search_candidates(
        &self,
        query: LyricsQuery,
    ) -> Result<Vec<LyricsSearchCandidate>> {
        match self
            .actor_ref
            .call(SearchCandidatesMessage { query }, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn apply_candidate(&self, track_key: String, doc: LyricsDoc) -> Result<()> {
        match self
            .actor_ref
            .call(
                ApplyCandidateMessage { track_key, doc },
                LYRICS_ACTOR_CALL_TIMEOUT,
            )
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn prefetch(self: &Arc<Self>, query: LyricsQuery) -> Result<()> {
        match self
            .actor_ref
            .call(PrefetchMessage { query }, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn prepare(self: &Arc<Self>, query: LyricsQuery) -> Result<()> {
        match self
            .actor_ref
            .call(PrepareMessage { query }, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn refresh_current(self: &Arc<Self>) -> Result<()> {
        match self
            .actor_ref
            .call(RefreshCurrentMessage, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub fn set_position_ms(&self, position_ms: u64) {
        let _ = self.actor_ref.cast(SetPositionMsMessage { position_ms });
    }
}

impl LyricsServiceCore {
    fn new() -> Self {
        Self {
            hub: LyricsEventHub::default(),
            state: Mutex::new(LyricsState::default()),
            client: reqwest::Client::builder()
                .user_agent("StellaTune/0.1")
                .build()
                .expect("failed to build lyrics http client"),
            cache_db_path: ArcSwapOption::new(None),
            http_rate: Mutex::new(HttpRateState::default()),
            source_health: Mutex::new(HashMap::new()),
            active_fetch: Mutex::new(ActiveFetchState::default()),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LyricsEvent> {
        self.hub.subscribe()
    }

    pub async fn set_cache_db_path(&self, db_path: String) -> Result<()> {
        let db_path = db_path.trim();
        if db_path.is_empty() {
            return Err(anyhow!("lyrics cache db path is empty"));
        }
        let path = PathBuf::from(db_path);
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create lyrics db dir: {}", parent.display()))?;
        }

        Self::init_cache_db(&path).await?;

        self.cache_db_path.store(Some(Arc::new(path)));
        Ok(())
    }

    pub async fn clear_cache(&self) -> Result<()> {
        let current_track_key = {
            let mut state = self.state.lock().expect("lyrics state mutex poisoned");
            state.cache.clear();
            state.current_doc = None;
            state.current_line_index = -1;
            state.current_track_key.clone()
        };

        if let Some(db_path) = self.cache_db_path() {
            let mut conn = Self::open_cache_db(&db_path).await?;
            sqlx::query("DELETE FROM lyrics_cache")
                .execute(&mut conn)
                .await
                .context("clear lyrics cache table failed")?;
        }

        if let Some(track_key) = current_track_key {
            self.hub.emit(LyricsEvent::Empty { track_key });
        }
        Ok(())
    }

    pub async fn search_candidates(
        &self,
        query: LyricsQuery,
    ) -> Result<Vec<LyricsSearchCandidate>> {
        let query = normalize_query(query);
        if query.track_key.is_empty() || query.title.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = Vec::new();
        let mut seen = HashSet::new();

        match self.search_candidates_lrclib(&query).await {
            Ok(items) => {
                for item in items {
                    push_candidate_unique(&mut out, &mut seen, item);
                }
            }
            Err(err) => {
                tracing::warn!("lyrics candidate search (lrclib) failed: {err}");
            }
        }

        match self.candidate_from_lyrics_ovh(&query).await {
            Ok(Some(item)) => {
                push_candidate_unique(&mut out, &mut seen, item);
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!("lyrics candidate search (lyrics.ovh) failed: {err}");
            }
        }

        Ok(out)
    }

    pub async fn apply_candidate(&self, track_key: String, mut doc: LyricsDoc) -> Result<()> {
        let track_key = track_key.trim().to_string();
        if track_key.is_empty() {
            return Ok(());
        }
        doc.track_key = track_key.clone();
        if doc.lines.is_empty() {
            return Ok(());
        }

        self.persist_doc_to_cache_db(&doc).await?;
        let mut emit_ready = false;
        {
            let mut state = self.state.lock().expect("lyrics state mutex poisoned");
            state.cache.insert(track_key.clone(), doc.clone());
            if state.current_track_key.as_deref() == Some(track_key.as_str()) {
                state.current_doc = Some(doc.clone());
                state.current_line_index = -1;
                emit_ready = true;
            }
        }

        if emit_ready {
            self.hub.emit(LyricsEvent::Ready { track_key, doc });
        }
        Ok(())
    }

    pub async fn prefetch(self: &Arc<Self>, query: LyricsQuery) -> Result<()> {
        let query = normalize_query(query);
        if query.track_key.is_empty() || query.title.is_empty() {
            return Ok(());
        }

        if let Some(doc) = load_local_lrc_doc_async(query.track_key.clone()).await {
            self.state
                .lock()
                .expect("lyrics state mutex poisoned")
                .cache
                .insert(query.track_key, doc);
            return Ok(());
        }

        if self
            .state
            .lock()
            .expect("lyrics state mutex poisoned")
            .cache
            .contains_key(&query.track_key)
        {
            return Ok(());
        }

        if let Some(doc) = self.load_doc_from_cache_db(&query.track_key).await {
            self.state
                .lock()
                .expect("lyrics state mutex poisoned")
                .cache
                .insert(query.track_key, doc);
            return Ok(());
        }

        let service = Arc::clone(self);
        global_runtime::spawn(async move {
            service.fetch_and_cache_only(query).await;
        });

        Ok(())
    }

    pub async fn prepare(self: &Arc<Self>, query: LyricsQuery) -> Result<()> {
        let query = normalize_query(query);
        // Switching tracks should stop any in-flight request for the previous track.
        self.cancel_active_fetch();
        if query.track_key.is_empty() || query.title.is_empty() {
            return Ok(());
        }

        if let Some(doc) = load_local_lrc_doc_async(query.track_key.clone()).await {
            {
                let mut state = self.state.lock().expect("lyrics state mutex poisoned");
                state.current_track_key = Some(query.track_key.clone());
                state.current_query = Some(query.clone());
                state.current_doc = Some(doc.clone());
                state.current_line_index = -1;
                state.cache.insert(query.track_key.clone(), doc.clone());
            }
            self.hub.emit(LyricsEvent::Ready {
                track_key: query.track_key,
                doc,
            });
            return Ok(());
        }

        {
            let mut state = self.state.lock().expect("lyrics state mutex poisoned");
            state.current_track_key = Some(query.track_key.clone());
            state.current_query = Some(query.clone());
            state.current_line_index = -1;

            if let Some(doc) = state.cache.get(&query.track_key).cloned() {
                state.current_doc = Some(doc.clone());
                drop(state);
                self.hub.emit(LyricsEvent::Ready {
                    track_key: query.track_key,
                    doc,
                });
                return Ok(());
            }

            state.current_doc = None;
        }

        if let Some(doc) = self.load_doc_from_cache_db(&query.track_key).await {
            {
                let mut state = self.state.lock().expect("lyrics state mutex poisoned");
                state.cache.insert(query.track_key.clone(), doc.clone());
                if state.current_track_key.as_deref() == Some(query.track_key.as_str()) {
                    state.current_doc = Some(doc.clone());
                    state.current_line_index = -1;
                }
            }
            self.hub.emit(LyricsEvent::Ready {
                track_key: query.track_key,
                doc,
            });
            return Ok(());
        }

        self.hub.emit(LyricsEvent::Loading {
            track_key: query.track_key.clone(),
        });

        let (fetch_id, cancel) = self.begin_active_fetch();
        let service = Arc::clone(self);
        global_runtime::spawn(async move {
            service.fetch_and_publish(query, fetch_id, cancel).await;
        });

        Ok(())
    }

    pub async fn refresh_current(self: &Arc<Self>) -> Result<()> {
        let query = self
            .state
            .lock()
            .expect("lyrics state mutex poisoned")
            .current_query
            .clone();
        if let Some(query) = query {
            self.prepare(query).await?;
        }
        Ok(())
    }

    pub fn set_position_ms(&self, position_ms: u64) {
        let to_emit = {
            let mut state = self.state.lock().expect("lyrics state mutex poisoned");
            let track_key = match state.current_track_key.clone() {
                Some(v) => v,
                None => return,
            };
            let doc = match state.current_doc.as_ref() {
                Some(v) => v,
                None => return,
            };
            if !doc.is_synced {
                return;
            }

            let idx = find_line_index(&doc.lines, position_ms as i64);
            if idx == state.current_line_index {
                None
            } else {
                state.current_line_index = idx;
                Some((track_key, idx))
            }
        };

        if let Some((track_key, line_index)) = to_emit {
            self.hub.emit(LyricsEvent::Cursor {
                track_key,
                line_index,
            });
        }
    }

    fn cancel_active_fetch(&self) {
        let old = self
            .active_fetch
            .lock()
            .expect("lyrics active fetch mutex poisoned")
            .token
            .take();
        if let Some(token) = old {
            token.cancel();
        }
    }

    fn begin_active_fetch(&self) -> (u64, CancellationToken) {
        let mut active = self
            .active_fetch
            .lock()
            .expect("lyrics active fetch mutex poisoned");
        if let Some(old) = active.token.take() {
            old.cancel();
        }
        active.latest_id = active.latest_id.wrapping_add(1);
        let id = active.latest_id;
        let token = CancellationToken::new();
        active.token = Some(token.clone());
        (id, token)
    }

    fn clear_active_fetch_if(&self, id: u64) {
        let mut active = self
            .active_fetch
            .lock()
            .expect("lyrics active fetch mutex poisoned");
        if active.latest_id == id {
            active.token = None;
        }
    }

    async fn fetch_and_publish(
        self: Arc<Self>,
        query: LyricsQuery,
        fetch_id: u64,
        cancel: CancellationToken,
    ) {
        let track_key = query.track_key.clone();
        let fetch_result = tokio::select! {
            _ = cancel.cancelled() => None,
            r = self.fetch_online(&query) => Some(r),
        };
        self.clear_active_fetch_if(fetch_id);

        let Some(fetch_result) = fetch_result else {
            return;
        };

        match fetch_result {
            Ok(Some(doc)) => {
                if let Err(err) = self.persist_doc_to_cache_db(&doc).await {
                    tracing::warn!("persist lyrics cache failed: {err}");
                }
                let mut should_emit = false;
                {
                    let mut state = self.state.lock().expect("lyrics state mutex poisoned");
                    state.cache.insert(track_key.clone(), doc.clone());
                    if state.current_track_key.as_deref() == Some(track_key.as_str()) {
                        state.current_doc = Some(doc.clone());
                        state.current_line_index = -1;
                        should_emit = true;
                    }
                }
                if should_emit {
                    self.hub.emit(LyricsEvent::Ready { track_key, doc });
                }
            }
            Ok(None) => {
                let mut should_emit = false;
                {
                    let mut state = self.state.lock().expect("lyrics state mutex poisoned");
                    if state.current_track_key.as_deref() == Some(track_key.as_str()) {
                        state.current_doc = None;
                        state.current_line_index = -1;
                        should_emit = true;
                    }
                }
                if should_emit {
                    self.hub.emit(LyricsEvent::Empty { track_key });
                }
            }
            Err(err) => {
                let should_emit = self
                    .state
                    .lock()
                    .expect("lyrics state mutex poisoned")
                    .current_track_key
                    .as_deref()
                    == Some(track_key.as_str());
                if should_emit {
                    self.hub.emit(LyricsEvent::Error {
                        track_key,
                        message: err.to_string(),
                    });
                }
            }
        }
    }

    async fn fetch_and_cache_only(self: Arc<Self>, query: LyricsQuery) {
        if let Ok(Some(doc)) = self.fetch_online(&query).await {
            if let Err(err) = self.persist_doc_to_cache_db(&doc).await {
                tracing::warn!("persist prefetched lyrics cache failed: {err}");
            }
            self.state
                .lock()
                .expect("lyrics state mutex poisoned")
                .cache
                .insert(query.track_key, doc);
        }
    }

    async fn fetch_online(&self, query: &LyricsQuery) -> Result<Option<LyricsDoc>> {
        let mut had_network_error = false;

        if self.source_available(SOURCE_LRCLIB) {
            match self.fetch_lrclib_get(query).await {
                Ok(Some(doc)) => {
                    self.mark_source_success(SOURCE_LRCLIB);
                    return Ok(Some(doc));
                }
                Ok(None) => {
                    self.mark_source_success(SOURCE_LRCLIB);
                    match self.fetch_lrclib_search(query).await {
                        Ok(Some(doc)) => return Ok(Some(doc)),
                        Ok(None) => {}
                        Err(err) => {
                            had_network_error = true;
                            self.mark_source_failure(SOURCE_LRCLIB);
                            tracing::warn!("lyrics source lrclib search failed: {err}");
                        }
                    }
                }
                Err(err) => {
                    had_network_error = true;
                    self.mark_source_failure(SOURCE_LRCLIB);
                    tracing::warn!("lyrics source lrclib get failed: {err}");
                }
            }
        }

        if self.source_available(SOURCE_LYRICS_OVH) {
            match self.fetch_lyrics_ovh_doc(query).await {
                Ok(Some(doc)) => {
                    self.mark_source_success(SOURCE_LYRICS_OVH);
                    return Ok(Some(doc));
                }
                Ok(None) => {
                    self.mark_source_success(SOURCE_LYRICS_OVH);
                }
                Err(err) => {
                    had_network_error = true;
                    self.mark_source_failure(SOURCE_LYRICS_OVH);
                    tracing::warn!("lyrics source lyrics.ovh failed: {err}");
                }
            }
        }

        if had_network_error {
            return Err(anyhow!("all online lyrics sources failed"));
        }
        Ok(None)
    }

    async fn search_candidates_lrclib(
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

    async fn candidate_from_lyrics_ovh(
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

    async fn fetch_lrclib_get(&self, query: &LyricsQuery) -> Result<Option<LyricsDoc>> {
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

    async fn fetch_lrclib_search(&self, query: &LyricsQuery) -> Result<Option<LyricsDoc>> {
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

    async fn fetch_lyrics_ovh_doc(&self, query: &LyricsQuery) -> Result<Option<LyricsDoc>> {
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

    fn source_available(&self, source: &'static str) -> bool {
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

    fn mark_source_success(&self, source: &'static str) {
        let mut health = self
            .source_health
            .lock()
            .expect("lyrics source health mutex poisoned");
        health.insert(source, SourceHealth::default());
    }

    fn mark_source_failure(&self, source: &'static str) {
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

    async fn wait_rate_limit_slot(&self) {
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

    async fn http_get_json_with_retry(
        &self,
        url: Url,
        op_name: &str,
        allow_not_found: bool,
        source: &'static str,
    ) -> Result<Option<Value>> {
        let mut last_error: Option<anyhow::Error> = None;

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
                }
                Err(err) => {
                    let retriable = is_retriable_error(&err);
                    if retriable && attempt < HTTP_RETRY_MAX_ATTEMPTS {
                        let retry_delay_ms = retry_backoff_ms(attempt);
                        tokio::time::sleep(Duration::from_millis(retry_delay_ms as u64)).await;
                        continue;
                    }
                    last_error = Some(anyhow!("{} request failed: {}", op_name, err));
                    break;
                }
            }
        }

        self.mark_source_failure(source);
        Err(last_error.unwrap_or_else(|| anyhow!("{} failed", op_name)))
    }

    fn cache_db_path(&self) -> Option<PathBuf> {
        self.cache_db_path
            .load_full()
            .map(|db_path| db_path.as_ref().clone())
    }

    async fn load_doc_from_cache_db(&self, track_key: &str) -> Option<LyricsDoc> {
        let db_path = self.cache_db_path()?;
        match async {
            let mut conn = Self::open_cache_db(&db_path).await?;
            let row = sqlx::query(
                "SELECT doc_json, updated_at_ms FROM lyrics_cache WHERE track_key = ?1 LIMIT 1",
            )
            .bind(track_key)
            .fetch_optional(&mut conn)
            .await
            .context("query lyrics cache failed")?;
            let Some(row) = row else {
                return Ok(None);
            };
            let updated_at_ms: i64 = row
                .try_get("updated_at_ms")
                .context("lyrics cache missing updated_at_ms")?;
            let now_ms = unix_now_ms();
            if now_ms.saturating_sub(updated_at_ms) > CACHE_TTL_MS {
                sqlx::query("DELETE FROM lyrics_cache WHERE track_key = ?1")
                    .bind(track_key)
                    .execute(&mut conn)
                    .await
                    .context("delete stale lyrics cache row failed")?;
                return Ok(None);
            }
            let doc_json: String = row
                .try_get("doc_json")
                .context("lyrics cache missing doc_json")?;
            let doc: LyricsDoc =
                serde_json::from_str(&doc_json).context("parse lyrics cache doc_json failed")?;
            Ok::<_, anyhow::Error>(Some(doc))
        }
        .await
        {
            Ok(doc) => doc,
            Err(err) => {
                tracing::warn!("load lyrics cache failed: {err}");
                None
            }
        }
    }

    async fn persist_doc_to_cache_db(&self, doc: &LyricsDoc) -> Result<()> {
        let Some(db_path) = self.cache_db_path() else {
            return Ok(());
        };
        let track_key = doc.track_key.clone();
        let source = doc.source.clone();
        let is_synced = if doc.is_synced { 1_i64 } else { 0_i64 };
        let doc_json = serde_json::to_string(doc).context("serialize lyrics doc failed")?;
        let updated_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time before unix epoch")?
            .as_millis() as i64;

        let mut conn = Self::open_cache_db(&db_path).await?;
        sqlx::query(
            "INSERT INTO lyrics_cache (track_key, source, is_synced, doc_json, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(track_key) DO UPDATE SET
               source = excluded.source,
               is_synced = excluded.is_synced,
               doc_json = excluded.doc_json,
               updated_at_ms = excluded.updated_at_ms",
        )
        .bind(track_key)
        .bind(source)
        .bind(is_synced)
        .bind(doc_json)
        .bind(updated_at_ms)
        .execute(&mut conn)
        .await
        .context("upsert lyrics cache failed")?;
        Ok(())
    }

    async fn init_cache_db(path: &Path) -> Result<()> {
        let mut conn = Self::open_cache_db(path).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS lyrics_cache (
               track_key TEXT PRIMARY KEY NOT NULL,
               source TEXT NOT NULL,
               is_synced INTEGER NOT NULL,
               doc_json TEXT NOT NULL,
               updated_at_ms INTEGER NOT NULL
             )",
        )
        .execute(&mut conn)
        .await
        .context("create lyrics cache table failed")?;
        Ok(())
    }

    async fn open_cache_db(path: &Path) -> Result<SqliteConnection> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let conn = SqliteConnection::connect_with(&options)
            .await
            .with_context(|| format!("open lyrics cache db failed: {}", path.display()))?;
        Ok(conn)
    }
}

fn unix_now_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(_) => 0,
    }
}

async fn load_local_lrc_doc_async(track_key: String) -> Option<LyricsDoc> {
    match tokio::task::spawn_blocking(move || load_local_lrc_doc_blocking(&track_key)).await {
        Ok(doc) => doc,
        Err(err) => {
            tracing::warn!("load local lrc task failed: {err}");
            None
        }
    }
}

fn load_local_lrc_doc_blocking(track_key: &str) -> Option<LyricsDoc> {
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
    let entries = std::fs::read_dir(parent).ok()?;
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

fn read_and_parse_lrc(path: &Path, track_key: &str) -> Option<LyricsDoc> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_lrc(track_key, "local_lrc", &content)
        .or_else(|| parse_plain(track_key, "local_lrc", content.trim()))
}

fn normalize_query(query: LyricsQuery) -> LyricsQuery {
    LyricsQuery {
        track_key: query.track_key.trim().to_string(),
        title: query.title.trim().to_string(),
        artist: trim_to_option(query.artist),
        album: trim_to_option(query.album),
        duration_ms: query.duration_ms,
    }
}

fn trim_to_option(input: Option<String>) -> Option<String> {
    input
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn find_line_index(lines: &[LyricLine], position_ms: i64) -> i64 {
    let mut idx = -1_i64;
    for (i, line) in lines.iter().enumerate() {
        match line.start_ms {
            Some(start_ms) if position_ms >= start_ms => {
                idx = i as i64;
            }
            Some(_) => break,
            None => {}
        }
    }
    idx
}

fn doc_from_lrclib_value(track_key: &str, value: &Value) -> Option<LyricsDoc> {
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

fn candidate_from_lrclib_value(
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

fn push_candidate_unique(
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

fn is_retriable_status(status: StatusCode) -> bool {
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

fn is_retriable_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request() || err.is_body()
}

fn retry_backoff_ms(attempt: usize) -> i64 {
    let exp = (attempt.saturating_sub(1)).min(4);
    let factor = 1_i64 << exp;
    (HTTP_RETRY_BASE_BACKOFF_MS * factor).min(HTTP_RETRY_MAX_BACKOFF_MS)
}

fn parse_retry_after_ms(headers: &reqwest::header::HeaderMap) -> Option<i64> {
    let raw = headers.get(reqwest::header::RETRY_AFTER)?;
    let text = raw.to_str().ok()?.trim();
    if text.is_empty() {
        return None;
    }
    let secs: i64 = text.parse().ok()?;
    Some((secs * 1_000).clamp(0, HTTP_RETRY_MAX_BACKOFF_MS))
}

fn json_string(value: &Value, keys: &[&str]) -> Option<String> {
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

fn json_opt_string(value: &Value, keys: &[&str]) -> Option<String> {
    json_string(value, keys)
}

fn parse_plain(track_key: &str, source: &str, plain: &str) -> Option<LyricsDoc> {
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

fn parse_lrc(track_key: &str, source: &str, raw: &str) -> Option<LyricsDoc> {
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

fn parse_timestamp_tag(tag: &str) -> Option<i64> {
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
