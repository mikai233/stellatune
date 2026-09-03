use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use arc_swap::ArcSwapOption;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::{LyricsDoc, LyricsEvent, LyricsQuery, LyricsSearchCandidate};

use super::actor::{
    ActiveFetchState, HttpRateState, LyricsEventHub, LyricsServiceCore, LyricsState,
};
use super::parser::{
    find_line_index, load_local_lrc_doc_async, normalize_query, push_candidate_unique,
};
impl LyricsServiceCore {
    pub(super) fn new() -> Self {
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
            fs::create_dir_all(parent)
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
            },
            Err(err) => {
                tracing::warn!("lyrics candidate search (lrclib) failed: {err}");
            },
        }

        match self.candidate_from_lyrics_ovh(&query).await {
            Ok(Some(item)) => {
                push_candidate_unique(&mut out, &mut seen, item);
            },
            Ok(None) => {},
            Err(err) => {
                tracing::warn!("lyrics candidate search (lyrics.ovh) failed: {err}");
            },
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
        tokio::spawn(async move {
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
        tokio::spawn(async move {
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

    pub(super) fn cancel_active_fetch(&self) {
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

    pub(super) fn begin_active_fetch(&self) -> (u64, CancellationToken) {
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

    pub(super) fn clear_active_fetch_if(&self, id: u64) {
        let mut active = self
            .active_fetch
            .lock()
            .expect("lyrics active fetch mutex poisoned");
        if active.latest_id == id {
            active.token = None;
        }
    }

    pub(super) async fn fetch_and_publish(
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
            },
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
            },
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
            },
        }
    }

    pub(super) async fn fetch_and_cache_only(self: Arc<Self>, query: LyricsQuery) {
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
}
