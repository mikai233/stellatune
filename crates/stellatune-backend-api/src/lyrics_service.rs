use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use lattice_actor::{handle::ActorHandle, mailbox::MailboxConfig, runtime::spawn_actor};
use tokio::sync::broadcast;

use crate::{LyricsDoc, LyricsEvent, LyricsQuery, LyricsSearchCandidate};

mod actor;
mod cache;
mod core;
mod handlers;
mod parser;
mod providers;

use self::actor::{LyricsServiceActor, LyricsServiceCore};

use self::handlers::apply_candidate::ApplyCandidateMessage;
use self::handlers::clear_cache::ClearCacheMessage;
use self::handlers::prefetch::PrefetchMessage;
use self::handlers::prepare::PrepareMessage;
use self::handlers::refresh_current::RefreshCurrentMessage;
use self::handlers::search_candidates::SearchCandidatesMessage;
use self::handlers::set_cache_db_path::SetCacheDbPathMessage;
use self::handlers::set_position_ms::SetPositionMsMessage;

const LYRICS_ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(30);

pub struct LyricsService {
    core: Arc<LyricsServiceCore>,
    actor_ref: ActorHandle<LyricsServiceActor>,
}

impl LyricsService {
    pub fn new() -> Arc<Self> {
        let core = Arc::new(LyricsServiceCore::new());
        let actor_ref = spawn_actor(
            LyricsServiceActor {
                core: Arc::clone(&core),
            },
            MailboxConfig::bounded(256),
        );
        Arc::new(Self { core, actor_ref })
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LyricsEvent> {
        self.core.subscribe_events()
    }

    pub async fn set_cache_db_path(&self, db_path: String) -> Result<()> {
        match self
            .actor_ref
            .ask(SetCacheDbPathMessage { db_path }, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn clear_cache(&self) -> Result<()> {
        match self
            .actor_ref
            .ask(ClearCacheMessage, LYRICS_ACTOR_CALL_TIMEOUT)
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
            .ask(SearchCandidatesMessage { query }, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn apply_candidate(&self, track_key: String, doc: LyricsDoc) -> Result<()> {
        match self
            .actor_ref
            .ask(
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
            .ask(PrefetchMessage { query }, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn prepare(self: &Arc<Self>, query: LyricsQuery) -> Result<()> {
        match self
            .actor_ref
            .ask(PrepareMessage { query }, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub async fn refresh_current(self: &Arc<Self>) -> Result<()> {
        match self
            .actor_ref
            .ask(RefreshCurrentMessage, LYRICS_ACTOR_CALL_TIMEOUT)
            .await
        {
            Ok(result) => result,
            Err(err) => Err(anyhow!("lyrics actor unavailable: {err:?}")),
        }
    }

    pub fn set_position_ms(&self, position_ms: u64) {
        let _ = self
            .actor_ref
            .try_tell(SetPositionMsMessage { position_ms });
    }
}
