use std::time::Instant;

use crossbeam_channel::Sender;
use tokio::sync::mpsc;

use crate::engine::decode::decoder::open_engine_decoder;
use crate::engine::messages::{InternalMsg, PredecodedChunk};
use stellatune_runtime as global_runtime;

use super::{EngineState, PreloadJob, PreloadWorker, TRACK_REF_TOKEN_PREFIX};

pub(super) fn start_preload_worker(internal_tx: Sender<InternalMsg>) -> PreloadWorker {
    let (tx, mut rx) = mpsc::unbounded_channel::<PreloadJob>();
    let join = global_runtime::spawn(async move {
        while let Some(job) = rx.recv().await {
            match job {
                PreloadJob::Task {
                    path,
                    position_ms,
                    token,
                } => {
                    let internal_tx = internal_tx.clone();
                    if let Err(join_err) = tokio::task::spawn_blocking(move || {
                        handle_preload_task(path, position_ms, token, &internal_tx)
                    })
                    .await
                    {
                        tracing::warn!("preload task join failed: {join_err}");
                    }
                }
                PreloadJob::Shutdown => break,
            }
        }
    });
    PreloadWorker { tx, join }
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
    let _ = worker.tx.send(PreloadJob::Task {
        path,
        position_ms,
        token,
    });
}

pub(super) fn handle_preload_task(
    path: String,
    position_ms: u64,
    token: u64,
    internal_tx: &Sender<InternalMsg>,
) {
    let t0 = Instant::now();
    match open_engine_decoder(&path) {
        Ok((mut decoder, track_info)) => {
            if position_ms > 0
                && let Err(err) = decoder.seek_ms(position_ms)
            {
                let _ = internal_tx.send(InternalMsg::PreloadFailed {
                    path: path.clone(),
                    position_ms,
                    message: err,
                    took_ms: t0.elapsed().as_millis() as u64,
                    token,
                });
                return;
            }
            match decoder.next_block(2048) {
                Ok(Some(samples)) if !samples.is_empty() => {
                    let _ = internal_tx.send(InternalMsg::PreloadReady {
                        path: path.clone(),
                        position_ms,
                        track_info: track_info.clone(),
                        chunk: PredecodedChunk {
                            samples,
                            sample_rate: track_info.sample_rate,
                            channels: track_info.channels,
                            start_at_ms: position_ms,
                        },
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
                Ok(_) => {
                    let _ = internal_tx.send(InternalMsg::PreloadFailed {
                        path: path.clone(),
                        position_ms,
                        message: "decoder returned no preload audio".to_string(),
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
                Err(err) => {
                    let _ = internal_tx.send(InternalMsg::PreloadFailed {
                        path: path.clone(),
                        position_ms,
                        message: err,
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
            }
        }
        Err(err) => {
            let _ = internal_tx.send(InternalMsg::PreloadFailed {
                path: path.clone(),
                position_ms,
                message: err,
                took_ms: t0.elapsed().as_millis() as u64,
                token,
            });
        }
    }
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
