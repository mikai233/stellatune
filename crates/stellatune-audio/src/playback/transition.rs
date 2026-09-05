//! Gapless promotion, fade envelopes, and two-track crossfade mixing.
//!
//! A crossfade starts only when the next pipeline can be normalized to the
//! current mix format and both tracks have compatible post-mix output. Current
//! and secondary tracks are decoded independently, receive per-track gains,
//! and are summed before the current track's shared post-mix chain and sink.
//!
//! Item-boundary markers are inserted in the PCM FIFO before overlap output.
//! Promotion keeps the existing sink and post-mix transforms, then re-bases the
//! new current track to that boundary. If the secondary track fails after an
//! overlap begins, the current track ramps from its instantaneous gain back to
//! unity instead of being discarded.

use stellatune_audio_core::error::FailureCode;
use stellatune_audio_core::{
    error::{FailureStage, PlaybackControlError, PlaybackFailure},
    format::{AudioBlock, PcmFormat},
    playback::MediaTime,
    transform::TransformPlacement,
};
use tokio::sync::broadcast;

use crate::planner::{CrossfadeCurve, GainCurve, TransitionPolicy};

use super::event::{PlaybackEvent, PlaybackState};
use super::lifecycle::set_state;
use super::normalizer::PcmNormalizer;
use super::pipeline::{TrackBlockStatus, process_transform_chain};
use super::pump::{begin_recovery, decode_secondary_block};
use super::runtime::PlaybackRuntimeConfig;
use super::sink_worker::{PendingWrite, SinkWorker};
use super::state::{
    ActiveTrack, CrossfadeState, DrainPhase, PlaybackSession, PreparedTrack, SecondaryTrack,
    TransitionRecoveryFade,
};
pub(super) enum CrossfadeStart {
    Unavailable,
    Pending,
    Started,
}

/// Starts a crossfade after its frame threshold and compatibility checks pass.
pub(super) fn maybe_start_crossfade(
    actor: &mut PlaybackSession,
) -> Result<CrossfadeStart, PlaybackControlError> {
    if actor.crossfade.is_some() {
        return Ok(CrossfadeStart::Unavailable);
    }
    let forced = actor.force_transition;
    let Some(current) = actor.current.as_mut() else {
        return Ok(CrossfadeStart::Unavailable);
    };
    let TransitionPolicy::Crossfade {
        duration_frames,
        curve,
        ..
    } = current.transition
    else {
        return Ok(CrossfadeStart::Unavailable);
    };
    let duration = match current.pipeline.duration_frames {
        Some(duration) => duration,
        None if forced => current
            .pipeline
            .produced_audible_frame
            .saturating_add(duration_frames),
        None => return Ok(CrossfadeStart::Unavailable),
    };
    if duration_frames == 0 {
        return Ok(CrossfadeStart::Unavailable);
    }
    let clock = current.output.clock();
    let produced_frontier = current
        .consumed_position_frame()
        .saturating_add(current.output_frames_to_mix(clock.buffered_frames));
    if !forced && produced_frontier < duration.saturating_sub(duration_frames) {
        return Ok(CrossfadeStart::Unavailable);
    }
    let Some(next) = actor.next.as_mut() else {
        return Ok(CrossfadeStart::Unavailable);
    };
    if normalize_prepared_for_mix(next, current.pipeline.mix_format).is_err() {
        return Ok(CrossfadeStart::Unavailable);
    }
    let compatible = current.pipeline.mix_format == next.pipeline.mix_format
        && current.output_format == next.output_format
        && current
            .sink_factory
            .compatibility_key(current.output_format)
            .ok()
            == next.plan.sink.compatibility_key(next.output_format).ok();
    if !compatible {
        return Ok(CrossfadeStart::Unavailable);
    }
    let item_id = next.plan.item.id;
    let boundary_base = clock.consumed_frames.saturating_add(clock.buffered_frames);
    if !current.output.mark_boundary(item_id)? {
        return Ok(CrossfadeStart::Pending);
    }
    let next = actor.next.take().expect("next checked above");
    actor.crossfade = Some(CrossfadeState {
        next: secondary_from_prepared(next),
        duration_frames,
        curve,
        progressed_frames: 0,
        current_block: current.pending_block.take(),
        next_block: None,
        sink_consumed_base_frame: boundary_base,
        boundary_announced: false,
    });
    actor.force_transition = false;
    Ok(CrossfadeStart::Started)
}

/// Converts a prepared successor to the current mix format and rebuilds post-mix stages.
pub(super) fn normalize_prepared_for_mix(
    prepared: &mut PreparedTrack,
    target: PcmFormat,
) -> Result<(), PlaybackControlError> {
    if prepared.pipeline.mix_format == target {
        return Ok(());
    }
    let source = prepared.pipeline.mix_format;
    let normalizer = PcmNormalizer::new(prepared.pipeline.normalizer_input_format, target)?;
    let mut post_mix_transforms = Vec::new();
    let mut output_format = target;
    for factory in &prepared.plan.transforms {
        if factory.descriptor().placement != TransformPlacement::PostMix {
            continue;
        }
        let mut transform = factory.create().map_err(|error| {
            PlaybackControlError::factory(
                FailureStage::Transform,
                factory.descriptor().id.clone(),
                error,
            )
        })?;
        output_format = transform.configure(output_format).map_err(|error| {
            PlaybackControlError::transform(error, factory.descriptor().id.clone())
        })?;
        post_mix_transforms.push(super::pipeline::ConfiguredTransform::new(
            transform,
            output_format,
            factory.descriptor().id.clone(),
        ));
    }
    prepared.pipeline.duration_frames = prepared.pipeline.duration_frames.map(|frames| {
        MediaTime::from_frames(frames, source.sample_rate).to_frames(target.sample_rate)
    });
    prepared.pipeline.normalizer = Some(normalizer);
    prepared.pipeline.mix_format = target;
    prepared.output_format = output_format;
    prepared.post_mix_transforms = post_mix_transforms;
    Ok(())
}

/// Removes output-only state while converting a prepared track for overlap decoding.
pub(super) fn secondary_from_prepared(prepared: PreparedTrack) -> SecondaryTrack {
    let recovery_plan = prepared.plan.clone();
    SecondaryTrack {
        pipeline: prepared.pipeline,
        recovery_plan,
        item_id: prepared.plan.item.id,
        sink_factory: prepared.plan.sink,
        transition: prepared.plan.policies.transition,
        seek_fade_frames: prepared.plan.policies.seek_fade_frames,
    }
}

/// Decodes and mixes at most one bounded block from each side of an overlap.
pub(super) fn pump_crossfade(
    config: &PlaybackRuntimeConfig,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
) {
    let Some(current) = actor.current.as_mut() else {
        actor.crossfade = None;
        return;
    };
    let Some(crossfade) = actor.crossfade.as_mut() else {
        return;
    };

    while let Some(item_id) = current.output.try_boundary() {
        if item_id == crossfade.next.item_id && !crossfade.boundary_announced {
            crossfade.boundary_announced = true;
            let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
        }
    }
    if let Some(block) = current.pending_block.take() {
        match current.output.try_write(block) {
            Ok(()) => return,
            Err(PendingWrite::Full(block)) => {
                current.pending_block = Some(block);
                return;
            },
            Err(PendingWrite::Closed) => {
                begin_recovery(
                    config,
                    event_tx,
                    actor,
                    state,
                    PlaybackControlError::failed(FailureStage::Sink, "sink worker closed"),
                );
                return;
            },
        }
    }

    if crossfade.current_block.is_none() {
        match current
            .pipeline
            .decode(config.block_frames, current.output.epoch())
        {
            Ok(TrackBlockStatus::Data(block)) => crossfade.current_block = Some(block),
            Ok(TrackBlockStatus::Pending) => {
                if current.output.clock().buffered_frames == 0 {
                    set_state(state, PlaybackState::Buffering, event_tx);
                }
                return;
            },
            Ok(TrackBlockStatus::EndOfStream) => {
                crossfade.progressed_frames = crossfade.duration_frames;
            },
            Err(PlaybackControlError::Failed(failure))
                if failure.stage == FailureStage::Decoder && failure.code == FailureCode::Io =>
            {
                begin_recovery(
                    config,
                    event_tx,
                    actor,
                    state,
                    PlaybackControlError::Failed(failure),
                );
                return;
            },
            Err(error) => {
                super::lifecycle::fail_current_error(actor, state, event_tx, error);
                return;
            },
        }
    }
    if crossfade.progressed_frames >= crossfade.duration_frames {
        finish_crossfade(actor, state, event_tx);
        return;
    }
    if crossfade.next_block.is_none() {
        let next_failure = match decode_secondary_block(
            &mut crossfade.next,
            config.block_frames,
            current.output.epoch(),
        ) {
            Ok(TrackBlockStatus::Data(block)) => {
                crossfade.next_block = Some(block);
                None
            },
            Ok(TrackBlockStatus::Pending) => {
                set_state(state, PlaybackState::Buffering, event_tx);
                return;
            },
            Ok(TrackBlockStatus::EndOfStream) => Some(PlaybackFailure::new(
                FailureStage::Decoder,
                FailureCode::StageFailed,
                None,
                "next track ended during an active crossfade",
            )),
            Err(PlaybackControlError::Failed(failure)) => Some(failure),
            Err(error) => Some(PlaybackFailure::new(
                FailureStage::Decoder,
                FailureCode::StageFailed,
                None,
                error.to_string(),
            )),
        };
        if let Some(failure) = next_failure {
            let progress =
                crossfade.progressed_frames as f32 / crossfade.duration_frames.max(1) as f32;
            let (current_gain, _) = crossfade_gains(progress.clamp(0.0, 1.0), crossfade.curve);
            let remaining = crossfade
                .duration_frames
                .saturating_sub(crossfade.progressed_frames)
                .max(1);
            if let Some(mut block) = crossfade.current_block.take() {
                current.recovery_fade = Some(TransitionRecoveryFade {
                    start_frame: block.timeline.start_frame,
                    duration_frames: remaining,
                    start_gain: current_gain,
                });
                apply_track_transition_gain(current, &mut block);
                if let Err(error) =
                    process_transform_chain(&mut current.post_mix_transforms, &mut block)
                {
                    super::lifecycle::fail_current_error(actor, state, event_tx, error);
                    return;
                }
                if !block.samples.is_empty() {
                    current.pending_block = Some(block);
                }
            }
            let failure = failure.with_context(Some(crossfade.next.item_id), actor.generation);
            let _ = event_tx.send(PlaybackEvent::Failed(failure));
            actor.crossfade = None;
            set_state(state, PlaybackState::Playing, event_tx);
            return;
        }
    }
    let current_frames = crossfade
        .current_block
        .as_ref()
        .map(AudioBlock::frames)
        .unwrap_or(0);
    let next_frames = crossfade
        .next_block
        .as_ref()
        .map(AudioBlock::frames)
        .unwrap_or(0);
    let frames = current_frames.min(next_frames).min(
        crossfade
            .duration_frames
            .saturating_sub(crossfade.progressed_frames) as usize,
    );
    if frames == 0 {
        return;
    }
    let channels = usize::from(current.pipeline.mix_format.channel_layout.channel_count());
    let sample_count = frames.saturating_mul(channels);
    let mut mixed = AudioBlock::new(current.pipeline.mix_format);
    mixed.timeline.start_frame = current
        .pipeline
        .produced_audible_frame
        .saturating_sub(current_frames as u64);
    mixed.timeline.epoch = current.output.epoch();
    mixed.samples.reserve(sample_count);
    let current_samples = &crossfade.current_block.as_ref().unwrap().samples[..sample_count];
    let next_samples = &crossfade.next_block.as_ref().unwrap().samples[..sample_count];
    for frame in 0..frames {
        let progress = (crossfade.progressed_frames.saturating_add(frame as u64) as f32
            / crossfade.duration_frames.max(1) as f32)
            .clamp(0.0, 1.0);
        let (gain_a, gain_b) = crossfade_gains(progress, crossfade.curve);
        let offset = frame.saturating_mul(channels);
        for channel in 0..channels {
            mixed.samples.push(
                current_samples[offset + channel] * gain_a
                    + next_samples[offset + channel] * gain_b,
            );
        }
    }
    consume_block_prefix(&mut crossfade.current_block, sample_count, frames as u64);
    consume_block_prefix(&mut crossfade.next_block, sample_count, frames as u64);
    crossfade.progressed_frames = crossfade.progressed_frames.saturating_add(frames as u64);
    if let Err(error) = process_transform_chain(&mut current.post_mix_transforms, &mut mixed) {
        super::lifecycle::fail_current_error(actor, state, event_tx, error);
        return;
    }
    if mixed.samples.is_empty() {
        return;
    }
    if *state == PlaybackState::Buffering {
        set_state(state, PlaybackState::Playing, event_tx);
    }
    match current.output.try_write(mixed) {
        Ok(()) => {},
        Err(PendingWrite::Full(block)) => current.pending_block = Some(block),
        Err(PendingWrite::Closed) => {
            begin_recovery(
                config,
                event_tx,
                actor,
                state,
                PlaybackControlError::failed(FailureStage::Sink, "sink worker closed"),
            );
            return;
        },
    }
    if crossfade.progressed_frames >= crossfade.duration_frames {
        finish_crossfade(actor, state, event_tx);
    }
}

/// Removes a mixed prefix and advances the remaining block's timeline.
#[allow(clippy::too_many_arguments)]
pub(super) fn consume_block_prefix(block: &mut Option<AudioBlock>, samples: usize, frames: u64) {
    let Some(value) = block.as_mut() else {
        return;
    };
    value.samples.drain(..samples);
    value.timeline.start_frame = value.timeline.start_frame.saturating_add(frames);
    if value.samples.is_empty() {
        *block = None;
    }
}

/// Returns current and next gains for normalized overlap progress.
pub(super) fn crossfade_gains(progress: f32, curve: CrossfadeCurve) -> (f32, f32) {
    match curve {
        CrossfadeCurve::Linear => (1.0 - progress, progress),
        CrossfadeCurve::EqualPower => {
            let phase = progress * std::f32::consts::FRAC_PI_2;
            (phase.cos(), phase.sin())
        },
    }
}

/// Promotes the secondary track while retaining the shared output pipeline.
pub(super) fn finish_crossfade(
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    let Some(crossfade) = actor.crossfade.take() else {
        return;
    };
    let Some(mut ended) = actor.current.take() else {
        return;
    };
    ended.pipeline.decoder.reset();
    for transform in &mut ended.pipeline.pre_mix_transforms {
        transform.reset();
    }
    if let Some(normalizer) = ended.pipeline.normalizer.as_mut() {
        normalizer.reset();
    }
    let output = ended.output;
    let next = crossfade.next;
    actor.current = Some(ActiveTrack {
        pipeline: next.pipeline,
        recovery_plan: next.recovery_plan,
        item_id: next.item_id,
        post_mix_transforms: ended.post_mix_transforms,
        output_format: ended.output_format,
        position_base_frame: 0,
        last_reported_position_frame: 0,
        pending_block: ended.pending_block,
        sink_factory: next.sink_factory,
        output,
        sink_consumed_base_frame: crossfade.sink_consumed_base_frame,
        boundary_announced: crossfade.boundary_announced,
        transition: next.transition,
        fade_in_frames: 0,
        fade_in_start_frame: 0,
        recovery_fade: None,
        seek_fade_frames: next.seek_fade_frames,
        forced_end_frame: None,
        drain_phase: DrainPhase::Decoding,
    });
    set_state(state, PlaybackState::Playing, event_tx);
}

/// Converts a forced switch into the active policy's earliest valid end frame.
pub(super) fn configure_forced_transition(actor: &mut PlaybackSession) {
    if !actor.force_transition {
        return;
    }
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    match current.transition {
        TransitionPolicy::Gapless => {
            current.forced_end_frame = Some(current.pipeline.produced_audible_frame);
        },
        TransitionPolicy::FadeOutIn {
            fade_out_frames, ..
        } => {
            let end = current
                .pipeline
                .produced_audible_frame
                .saturating_add(fade_out_frames);
            current.forced_end_frame = Some(end);
        },
        TransitionPolicy::Crossfade { .. } => {},
    }
}

/// Activates a prepared track using an existing compatible sink and post-mix chain.
pub(super) fn activate_with_output(
    prepared: PreparedTrack,
    output: SinkWorker,
    post_mix_transforms: Vec<super::pipeline::ConfiguredTransform>,
    sink_consumed_base_frame: u64,
    boundary_announced: bool,
    fade_in_frames: u64,
) -> ActiveTrack {
    let recovery_plan = prepared.plan.clone();
    let transition = prepared.plan.policies.transition;
    let seek_fade_frames = prepared.plan.policies.seek_fade_frames;
    ActiveTrack {
        recovery_plan,
        item_id: prepared.plan.item.id,
        post_mix_transforms,
        output_format: prepared.output_format,
        position_base_frame: prepared.pipeline.produced_audible_frame,
        last_reported_position_frame: prepared.pipeline.produced_audible_frame,
        pipeline: prepared.pipeline,
        pending_block: None,
        sink_factory: prepared.plan.sink,
        output,
        sink_consumed_base_frame,
        boundary_announced,
        transition,
        fade_in_frames,
        fade_in_start_frame: 0,
        recovery_fade: None,
        seek_fade_frames,
        forced_end_frame: None,
        drain_phase: DrainPhase::Decoding,
    }
}

/// Returns the fade-in duration used after a non-overlapping promotion.
pub(super) fn transition_fade_in_frames(transition: TransitionPolicy) -> u64 {
    match transition {
        TransitionPolicy::FadeOutIn { fade_in_frames, .. } => fade_in_frames,
        TransitionPolicy::Crossfade {
            fallback: crate::planner::CrossfadeFallback::FadeOutIn,
            duration_frames,
            ..
        } => duration_frames,
        _ => 0,
    }
}

/// Applies seek, recovery, fade-in, and end-of-track envelopes to one block.
pub(super) fn apply_track_transition_gain(current: &ActiveTrack, block: &mut AudioBlock) {
    let channels = usize::from(block.format.channel_layout.channel_count());
    let fade_out = match current.transition {
        TransitionPolicy::FadeOutIn {
            fade_out_frames,
            curve,
            ..
        } => Some((fade_out_frames, curve)),
        TransitionPolicy::Crossfade {
            duration_frames,
            fallback: crate::planner::CrossfadeFallback::FadeOutIn,
            ..
        } => Some((duration_frames, GainCurve::Linear)),
        _ => None,
    };
    for (frame_index, frame) in block.samples.chunks_exact_mut(channels).enumerate() {
        let timeline_frame = block
            .timeline
            .start_frame
            .saturating_add(frame_index as u64);
        let mut gain = 1.0_f32;
        if current.fade_in_frames > 0
            && timeline_frame >= current.fade_in_start_frame
            && timeline_frame
                < current
                    .fade_in_start_frame
                    .saturating_add(current.fade_in_frames)
        {
            gain *= curve_gain(
                timeline_frame.saturating_sub(current.fade_in_start_frame) as f32
                    / current.fade_in_frames.max(1) as f32,
                GainCurve::Linear,
            );
        }
        if let Some(recovery) = current.recovery_fade
            && timeline_frame >= recovery.start_frame
            && timeline_frame
                < recovery
                    .start_frame
                    .saturating_add(recovery.duration_frames)
        {
            let progress = timeline_frame.saturating_sub(recovery.start_frame) as f32
                / recovery.duration_frames.max(1) as f32;
            gain *= recovery.start_gain + (1.0 - recovery.start_gain) * progress;
        }
        if let (Some(duration), Some((fade_frames, curve))) = (
            current
                .forced_end_frame
                .or(current.pipeline.duration_frames),
            fade_out,
        ) && fade_frames > 0
            && timeline_frame >= duration.saturating_sub(fade_frames)
        {
            let remaining = duration.saturating_sub(timeline_frame);
            gain *= curve_gain(remaining as f32 / fade_frames as f32, curve);
        }
        if gain != 1.0 {
            for sample in frame {
                *sample *= gain;
            }
        }
    }
}

/// Evaluates a one-sided gain curve after clamping progress to `0.0..=1.0`.
pub(super) fn curve_gain(progress: f32, curve: GainCurve) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    match curve {
        GainCurve::Linear => progress,
        GainCurve::EqualPower => (progress * std::f32::consts::FRAC_PI_2).sin(),
    }
}
