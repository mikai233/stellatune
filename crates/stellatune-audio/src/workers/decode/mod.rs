//! Playback pipeline state and bounded actor-turn helpers.

pub(crate) mod handlers;
mod pipeline_policies;
pub(crate) mod recovery;
pub(crate) mod state;
mod util;
#[path = "loop.rs"]
pub(crate) mod worker_loop;

use std::sync::Arc;

use crate::error::DecodeError;

#[derive(Debug, Clone)]
pub(crate) enum DecodeWorkerEvent {
    TrackChanged { track_token: String },
    Recovering { attempt: u32, backoff_ms: u64 },
    Position { position_ms: i64 },
    Eof,
    AudioStart,
    AudioEnd,
    Error(DecodeError),
}

pub(crate) type DecodeWorkerEventCallback = Arc<dyn Fn(DecodeWorkerEvent) + Send + Sync>;
