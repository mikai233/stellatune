//! Small helpers shared by decode worker command handlers and loop code.

use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use stellatune_audio_core::pipeline::context::PipelineContext;

use crate::config::engine::PlayerState;
use crate::error::DecodeError;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn recv_result(
    resp_rx: Receiver<Result<(), DecodeError>>,
    timeout: Duration,
) -> Result<(), DecodeError> {
    resp_rx
        .recv_timeout(timeout)
        .map_err(|_| DecodeError::CommandTimedOut {
            timeout_ms: timeout.as_millis(),
        })?
}

pub(crate) fn update_state(
    callback: &DecodeWorkerEventCallback,
    current_state: &mut PlayerState,
    next_state: PlayerState,
) {
    if *current_state == next_state {
        return;
    }
    *current_state = next_state;
    callback(DecodeWorkerEvent::StateChanged(next_state));
}

pub(crate) fn maybe_emit_position(
    callback: &DecodeWorkerEventCallback,
    ctx: &PipelineContext,
    last_emit_at: &mut Instant,
) {
    if last_emit_at.elapsed() < Duration::from_millis(200) {
        return;
    }
    *last_emit_at = Instant::now();
    callback(DecodeWorkerEvent::Position {
        position_ms: ctx.position_ms.max(0),
    });
}
