//! Shared state transitions, seeking, failure publication, and teardown.
//!
//! Helpers in this module keep state changes and their observable events paired.
//! `set_state` is the only ordinary path that emits `StateChanged`, preventing
//! duplicate notifications. Seeking advances the PCM epoch before touching the
//! decoder so blocks queued for the previous position cannot reach the sink.

use stellatune_audio_core::error::FailureCode;
use stellatune_audio_core::error::FailureStage;
use stellatune_audio_core::{
    decoder::{DecoderSeekStatus, SeekResult},
    error::{PlaybackControlError, PlaybackFailure},
    playback::{MediaTime, PlaybackItemId},
};
use tokio::sync::broadcast;

use super::event::{PlaybackEvent, PlaybackState};
use super::pump::emit_position_if_due;
use super::state::{DrainPhase, PlaybackSession};
/// Invalidates queued PCM and begins a decoder seek for the current item.
pub(super) fn start_seek(
    actor: &mut PlaybackSession,
    position: MediaTime,
) -> Result<(PlaybackItemId, DecoderSeekStatus), PlaybackControlError> {
    actor.crossfade = None;
    actor.force_transition = false;
    actor.advance_options = None;
    if let Some(pending) = actor.pending_preparation.take() {
        pending.cancellation.cancel();
    }
    if let Some(pending) = actor.pending_recovery.take() {
        pending.cancellation.cancel();
    }
    let current = actor
        .current
        .as_mut()
        .ok_or(PlaybackControlError::InvalidState)?;
    current.forced_end_frame = None;
    current.recovery_fade = None;
    current.pending_block = None;
    current.output.discard()?;
    current.sink_consumed_base_frame = 0;
    current.boundary_announced = true;
    let target = position
        .to_frames(current.pipeline.decoded_format.sample_rate)
        .saturating_add(current.pipeline.trim_head_frames);
    let status = current
        .pipeline
        .decoder
        .start_seek(target)
        .map_err(|error| {
            PlaybackControlError::decoder(error, current.pipeline.decoder_id.clone())
        })?;
    Ok((current.item_id, status))
}

/// Performs one bounded continuation of a pending decoder seek.
pub(super) fn advance_pending_seek(
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
) {
    let Some(pending) = actor.pending_seek.take() else {
        return;
    };
    let status = actor
        .current
        .as_mut()
        .ok_or(PlaybackControlError::InvalidState)
        .and_then(|current| {
            current.pipeline.decoder.continue_seek().map_err(|error| {
                PlaybackControlError::decoder(error, current.pipeline.decoder_id.clone())
            })
        });
    match status {
        Ok(DecoderSeekStatus::Pending) => actor.pending_seek = Some(pending),
        Ok(DecoderSeekStatus::Complete(result)) => {
            finish_seek(actor, result, event_tx);
            set_state(
                state,
                if actor.wants_playing {
                    PlaybackState::Playing
                } else {
                    PlaybackState::Paused
                },
                event_tx,
            );
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id: pending.item_id,
                active: false,
            });
            if let Some(current) = actor.current.as_ref() {
                current.output.reply_when_settled(pending.response);
            }
        },
        Err(error) => {
            set_state(state, PlaybackState::Failed, event_tx);
            let _ = pending.response.send(Err(error));
        },
    }
}

/// Re-bases frame counters and resets buffered stages after decoder seek completion.
pub(super) fn finish_seek(
    actor: &mut PlaybackSession,
    result: SeekResult,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    current.pipeline.decoded_frame = result.actual_frame;
    let audible_frame = MediaTime::from_frames(
        result
            .actual_frame
            .saturating_sub(current.pipeline.trim_head_frames),
        current.pipeline.decoded_format.sample_rate,
    )
    .to_frames(current.pipeline.mix_format.sample_rate);
    current.pipeline.produced_audible_frame = audible_frame;
    current.position_base_frame = audible_frame;
    current.last_reported_position_frame = audible_frame;
    current.pipeline.tail_buffer.clear();
    current.drain_phase = DrainPhase::Decoding;
    current.fade_in_start_frame = audible_frame;
    current.fade_in_frames = current.seek_fade_frames;
    for transform in &mut current.pipeline.pre_mix_transforms {
        transform.reset();
    }
    for transform in &mut current.post_mix_transforms {
        transform.reset();
    }
    if let Some(normalizer) = current.pipeline.normalizer.as_mut() {
        normalizer.reset();
    }
    emit_position_if_due(current, event_tx, true);
}

/// Updates state and publishes exactly one event when the value changes.
pub(super) fn set_state(
    current: &mut PlaybackState,
    state: PlaybackState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    if *current != state {
        *current = state;
        let _ = event_tx.send(PlaybackEvent::StateChanged(state));
    }
}

/// Publishes the contextual payload of a failed control result.
pub(super) fn publish_control_failure(
    error: &PlaybackControlError,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    if let PlaybackControlError::Failed(failure) = error {
        let _ = event_tx.send(PlaybackEvent::Failed(failure.clone()));
    }
}

/// Tears down the current pipeline and publishes a terminal contextual failure.
pub(super) fn fail_current(
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    stage: FailureStage,
    message: String,
) {
    let item_id = actor.current.as_ref().map(|current| current.item_id);
    stop_current(actor);
    set_state(state, PlaybackState::Failed, event_tx);
    let failure = PlaybackFailure::new(stage, FailureCode::StageFailed, None, message)
        .with_context(item_id, actor.generation);
    let _ = event_tx.send(PlaybackEvent::Failed(failure));
}

/// Completes an outstanding seek reply as closed before replacing session work.
pub(super) fn reject_pending(actor: &mut PlaybackSession) {
    if let Some(pending) = actor.pending_seek.take() {
        let _ = pending.response.send(Err(PlaybackControlError::Closed));
    }
}

/// Resets all current pipeline stages and requests shutdown of its sink worker.
pub(super) fn stop_current(actor: &mut PlaybackSession) {
    if let Some(mut current) = actor.current.take() {
        current.pipeline.decoder.reset();
        for transform in &mut current.pipeline.pre_mix_transforms {
            transform.reset();
        }
        for transform in &mut current.post_mix_transforms {
            transform.reset();
        }
        if let Some(normalizer) = current.pipeline.normalizer.as_mut() {
            normalizer.reset();
        }
        current.output.shutdown();
    }
}

/// Publishes an existing typed failure without discarding its implementation identity.
pub(super) fn fail_current_error(
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    error: PlaybackControlError,
) {
    let item_id = actor.current.as_ref().map(|track| track.item_id);
    stop_current(actor);
    set_state(state, PlaybackState::Failed, event_tx);
    publish_control_failure(&error.with_context(item_id, actor.generation), event_tx);
}
