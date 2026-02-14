use crossbeam_channel::Sender;

use crate::engine::messages::InternalMsg;

use super::preload_actor::handlers::enqueue::PreloadEnqueueMessage;
use super::{EngineState, PreloadWorker, TRACK_REF_TOKEN_PREFIX, preload_actor};

pub(super) fn start_preload_worker(internal_tx: Sender<InternalMsg>) -> PreloadWorker {
    let (actor_ref, join) =
        preload_actor::spawn_preload_actor(internal_tx).expect("failed to spawn preload actor");
    PreloadWorker { actor_ref, join }
}

pub(super) fn enqueue_preload_task(
    state: &mut EngineState,
    path: String,
    position_ms: u64,
    token: u64,
) {
    let Some(worker) = state.preload_worker.as_ref() else {
        return;
    };
    let _ = worker.actor_ref.cast(PreloadEnqueueMessage {
        path,
        position_ms,
        token,
    });
}

pub(super) fn engine_token_to_track_ref(token: &str) -> Option<stellatune_core::TrackRef> {
    let json = token.strip_prefix(TRACK_REF_TOKEN_PREFIX)?;
    serde_json::from_str::<stellatune_core::TrackRef>(json).ok()
}

pub(super) fn event_path_from_engine_token(token: &str) -> String {
    match engine_token_to_track_ref(token) {
        Some(track) => track.locator,
        None => token.to_string(),
    }
}

pub(super) fn track_ref_to_event_path(track: &stellatune_core::TrackRef) -> Option<String> {
    let locator = track.locator.trim();
    if locator.is_empty() {
        None
    } else {
        Some(locator.to_string())
    }
}

pub(super) fn track_ref_to_engine_token(track: &stellatune_core::TrackRef) -> Option<String> {
    if track.source_id.trim().eq_ignore_ascii_case("local") {
        return track_ref_to_event_path(track);
    }
    let json = serde_json::to_string(track).ok()?;
    Some(format!("{TRACK_REF_TOKEN_PREFIX}{json}"))
}
