use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use arc_swap::ArcSwapOption;
use lattice_actor::{error::ActorError, state_machine::Stateless, traits::Actor};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::{LyricsDoc, LyricsEvent, LyricsQuery};

pub(super) struct LyricsEventHub {
    tx: broadcast::Sender<LyricsEvent>,
}

impl Default for LyricsEventHub {
    fn default() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }
}

impl LyricsEventHub {
    pub(super) fn subscribe(&self) -> broadcast::Receiver<LyricsEvent> {
        self.tx.subscribe()
    }

    pub(super) fn emit(&self, event: LyricsEvent) {
        let _ = self.tx.send(event);
    }
}

#[derive(Default)]
pub(super) struct LyricsState {
    pub(super) current_track_key: Option<String>,
    pub(super) current_query: Option<LyricsQuery>,
    pub(super) current_doc: Option<LyricsDoc>,
    pub(super) current_line_index: i64,
    pub(super) cache: HashMap<String, LyricsDoc>,
}

#[derive(Default)]
pub(super) struct HttpRateState {
    pub(super) next_allowed_at_ms: i64,
}

#[derive(Default, Clone, Copy)]
pub(super) struct SourceHealth {
    pub(super) consecutive_failures: u32,
    pub(super) blocked_until_ms: i64,
}

#[derive(Default)]
pub(super) struct ActiveFetchState {
    pub(super) latest_id: u64,
    pub(super) token: Option<CancellationToken>,
}

pub(super) struct LyricsServiceCore {
    pub(super) hub: LyricsEventHub,
    pub(super) state: Mutex<LyricsState>,
    pub(super) client: reqwest::Client,
    pub(super) cache_db_path: ArcSwapOption<PathBuf>,
    pub(super) http_rate: Mutex<HttpRateState>,
    pub(super) source_health: Mutex<HashMap<&'static str, SourceHealth>>,
    pub(super) active_fetch: Mutex<ActiveFetchState>,
}

pub(super) struct LyricsServiceActor {
    pub(super) core: Arc<LyricsServiceCore>,
}

impl Actor for LyricsServiceActor {
    type Error = ActorError;
    type Behavior = Stateless;
}
