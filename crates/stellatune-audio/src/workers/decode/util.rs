//! Small helpers shared by decode worker command handlers and loop code.

use std::time::{Duration, Instant};

use stellatune_audio_core::pipeline::context::PipelineContext;

use crate::config::engine::PlayerState;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn update_state(
    _callback: &DecodeWorkerEventCallback,
    pumping: &mut bool,
    next_state: PlayerState,
) {
    *pumping = next_state == PlayerState::Playing;
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
