use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use stellatune_audio_core::pipeline::context::PipelineContext;

use crate::types::PlayerState;
use crate::worker::decode_loop::{DecodeLoopEvent, DecodeLoopEventCallback};

pub(crate) fn recv_result(
    resp_rx: Receiver<Result<(), String>>,
    timeout: Duration,
) -> Result<(), String> {
    resp_rx
        .recv_timeout(timeout)
        .map_err(|_| "decode loop command timed out".to_string())?
}

pub(crate) fn update_state(
    callback: &DecodeLoopEventCallback,
    current_state: &mut PlayerState,
    next_state: PlayerState,
) {
    if *current_state == next_state {
        return;
    }
    *current_state = next_state;
    callback(DecodeLoopEvent::StateChanged(next_state));
}

pub(crate) fn maybe_emit_position(
    callback: &DecodeLoopEventCallback,
    ctx: &PipelineContext,
    last_emit_at: &mut Instant,
) {
    if last_emit_at.elapsed() < Duration::from_millis(200) {
        return;
    }
    *last_emit_at = Instant::now();
    callback(DecodeLoopEvent::Position {
        position_ms: ctx.position_ms.max(0),
    });
}
