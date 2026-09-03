use stellatune_audio_core::{
    DecoderSeekStatus, MediaTime, PlaybackControlError, PlaybackFailure, PlaybackItemId, SeekResult,
};
use tokio::sync::broadcast;

use super::control::CommandReply;
use super::event::{PlaybackEvent, PlaybackState};
use super::pump::emit_position_if_due;
use super::state::{ActorState, DrainPhase};
pub(super) fn fail_promoted(
    actor: &mut ActorState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    message: String,
) {
    set_state(actor, PlaybackState::Failed, event_tx);
    let failure = PlaybackFailure::internal("sink", message).with_context(None, actor.generation);
    let _ = event_tx.send(PlaybackEvent::Failed(failure));
}

pub(super) fn start_seek(
    actor: &mut ActorState,
    position: MediaTime,
) -> Result<(PlaybackItemId, DecoderSeekStatus), PlaybackControlError> {
    let current = actor
        .current
        .as_mut()
        .ok_or(PlaybackControlError::InvalidState)?;
    current.epoch = current.epoch.wrapping_add(1);
    current.pending_block = None;
    current.output.discard(current.epoch)?;
    current.sink_consumed_base_frame = 0;
    current.boundary_announced = true;
    let target = position
        .to_frames(current.decoded_format.sample_rate)
        .saturating_add(current.trim_head_frames);
    let status = current
        .decoder
        .start_seek(target)
        .map_err(|error| PlaybackControlError::failed("decoder", error.to_string()))?;
    Ok((current.item_id, status))
}

pub(super) fn advance_pending_seek(
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut ActorState,
) {
    let Some(pending) = actor.pending_seek.take() else {
        return;
    };
    let status = actor
        .current
        .as_mut()
        .ok_or(PlaybackControlError::InvalidState)
        .and_then(|current| {
            current
                .decoder
                .continue_seek()
                .map_err(|error| PlaybackControlError::failed("decoder", error.to_string()))
        });
    match status {
        Ok(DecoderSeekStatus::Pending) => actor.pending_seek = Some(pending),
        Ok(DecoderSeekStatus::Complete(result)) => {
            finish_seek(actor, result, event_tx);
            set_state(actor, pending.resume_state, event_tx);
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id: pending.item_id,
                active: false,
            });
            let _ = pending.response.send(Ok(CommandReply::Unit));
        },
        Err(error) => {
            set_state(actor, PlaybackState::Failed, event_tx);
            let _ = pending.response.send(Err(error));
        },
    }
}

pub(super) fn finish_seek(
    actor: &mut ActorState,
    result: SeekResult,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    current.decoded_frame = result.actual_frame;
    let audible_frame = result.actual_frame.saturating_sub(current.trim_head_frames);
    current.produced_audible_frame = audible_frame;
    current.position_base_frame = audible_frame;
    current.last_reported_position_frame = audible_frame;
    current.tail_buffer.clear();
    current.drain_phase = DrainPhase::Decoding;
    current.fade_in_start_frame = audible_frame;
    current.fade_in_frames = current.seek_fade_frames;
    for transform in &mut current.pre_mix_transforms {
        transform.reset();
    }
    for transform in &mut current.post_mix_transforms {
        transform.reset();
    }
    if let Some(normalizer) = current.normalizer.as_mut() {
        normalizer.reset();
    }
    emit_position_if_due(current, event_tx, true);
}

pub(super) fn set_state(
    actor: &mut ActorState,
    state: PlaybackState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    if actor.state != state {
        actor.state = state;
        let _ = event_tx.send(PlaybackEvent::StateChanged(state));
    }
}

pub(super) fn publish_control_failure(
    error: &PlaybackControlError,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    if let PlaybackControlError::Failed(failure) = error {
        let _ = event_tx.send(PlaybackEvent::Failed(failure.clone()));
    }
}

pub(super) fn fail_current(
    actor: &mut ActorState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    stage: &'static str,
    message: String,
) {
    let item_id = actor.current.as_ref().map(|current| current.item_id);
    stop_current(actor);
    set_state(actor, PlaybackState::Failed, event_tx);
    let failure = PlaybackFailure::internal(stage, message).with_context(item_id, actor.generation);
    let _ = event_tx.send(PlaybackEvent::Failed(failure));
}

pub(super) fn reject_pending(actor: &mut ActorState) {
    if let Some(response) = actor.pending_current_response.take() {
        let _ = response.send(Err(PlaybackControlError::Closed));
    }
    if let Some(response) = actor.pending_next_response.take() {
        let _ = response.send(Err(PlaybackControlError::Closed));
    }
    if let Some(pending) = actor.pending_seek.take() {
        let _ = pending.response.send(Err(PlaybackControlError::Closed));
    }
}

pub(super) fn stop_current(actor: &mut ActorState) {
    if let Some(mut current) = actor.current.take() {
        current.decoder.reset();
        for transform in &mut current.pre_mix_transforms {
            transform.reset();
        }
        for transform in &mut current.post_mix_transforms {
            transform.reset();
        }
        if let Some(normalizer) = current.normalizer.as_mut() {
            normalizer.reset();
        }
        current.output.shutdown();
    }
}
