//! Bounded decoding, PCM pumping, gapless trimming, draining, and recovery.
//!
//! `pump_once` performs at most one meaningful unit of data-plane work: retry a
//! pending sink write, decode one block, advance one drain stage, or advance a
//! crossfade. It never waits for network input or device capacity. Temporary
//! starvation enters `Buffering`; recoverable decoder I/O and sink failures
//! schedule off-turn recovery from a sink-consumed checkpoint.
//!
//! Gapless head trim is removed immediately. Tail trim is withheld until the
//! decoder frontier proves that samples are not encoder padding. On EOF, every
//! pre-mix transform, the normalizer, and every post-mix transform is drained in
//! pipeline order before promotion or `PlaybackEnded`.

use std::sync::Arc;
use stellatune_audio_core::error::FailureCode;
use stellatune_audio_core::error::FailureStage;

use stellatune_audio_core::{
    error::PlaybackControlError, format::AudioBlock, playback::MediaTime, transform::DrainStatus,
};
use tokio::sync::broadcast;

use super::event::{PlaybackEvent, PlaybackState};
use super::lifecycle::{fail_current_error, set_state};
use super::pipeline::{TrackBlockStatus, process_transform_chain};
use super::runtime::PlaybackRuntimeConfig;
use super::sink_worker::{PendingWrite, SinkWorker};
use super::state::{
    ActiveTrack, DrainPhase, PlaybackSession, PreparationPurpose, PreparedTrack,
    RecoveryPreparation, SecondaryTrack,
};
use super::transition::{
    activate_with_output, apply_track_transition_gain, maybe_start_crossfade,
    normalize_prepared_for_mix, pump_crossfade, transition_fade_in_frames,
};
/// Attaches a new sink worker and converts a prepared pipeline into the current track.
pub(super) fn activate(
    prepared: PreparedTrack,
    config: &PlaybackRuntimeConfig,
    output_gain: f32,
    workers: &super::output_workers::OutputWorkers,
) -> Result<ActiveTrack, PlaybackControlError> {
    let recovery_plan = prepared.plan.clone();
    let sink_factory = Arc::clone(&prepared.plan.sink);
    let output = SinkWorker::start(
        Arc::clone(&sink_factory),
        prepared.output_format,
        config.pcm_ring_blocks,
        output_gain,
        workers,
    )?;
    Ok(ActiveTrack {
        recovery_plan,
        item_id: prepared.plan.item.id,
        post_mix_transforms: prepared.post_mix_transforms,
        output_format: prepared.output_format,
        position_base_frame: prepared.pipeline.produced_audible_frame,
        last_reported_position_frame: prepared.pipeline.produced_audible_frame,
        pipeline: prepared.pipeline,
        pending_block: None,
        sink_factory,
        output,
        sink_consumed_base_frame: 0,
        boundary_announced: true,
        transition: prepared.plan.policies.transition,
        fade_in_frames: 0,
        fade_in_start_frame: 0,
        recovery_fade: None,
        seek_fade_frames: prepared.plan.policies.seek_fade_frames,
        forced_end_frame: None,
        drain_phase: DrainPhase::Decoding,
    })
}

/// Advances the active data path by at most one bounded work unit.
pub(super) fn pump_once(
    config: &PlaybackRuntimeConfig,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
) {
    if actor
        .current
        .as_ref()
        .is_some_and(|track| !track.output.is_ready())
    {
        return;
    }
    maybe_start_crossfade(actor);
    if actor.crossfade.is_some() {
        pump_crossfade(config, event_tx, actor, state);
        return;
    }
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    // If a forced overlap could not start, honor its non-overlap fallback now.
    if actor.force_transition
        && actor.next.as_mut().is_some()
        && current.forced_end_frame.is_none()
        && let crate::planner::TransitionPolicy::Crossfade {
            duration_frames,
            fallback,
            ..
        } = current.transition
    {
        let fade = match fallback {
            crate::planner::CrossfadeFallback::Gapless => 0,
            crate::planner::CrossfadeFallback::FadeOutIn => duration_frames,
        };
        current.forced_end_frame =
            Some(current.pipeline.produced_audible_frame.saturating_add(fade));
    }
    while let Some(item_id) = current.output.try_boundary() {
        if item_id == current.item_id && !current.boundary_announced {
            current.boundary_announced = true;
            let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
        }
    }
    emit_position_if_due(current, event_tx, false);
    if current
        .forced_end_frame
        .is_some_and(|end| current.pipeline.produced_audible_frame >= end)
    {
        promote_or_end(actor, state, config, event_tx);
        return;
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
    match current
        .pipeline
        .decode(config.block_frames, current.output.epoch())
    {
        Ok(TrackBlockStatus::Data(mut block)) => {
            apply_track_transition_gain(current, &mut block);
            if let Err(error) =
                process_transform_chain(&mut current.post_mix_transforms, &mut block)
            {
                super::lifecycle::fail_current_error(actor, state, event_tx, error);
                return;
            }
            if block.samples.is_empty() {
                return;
            }
            let item_id = current.item_id;
            match current.output.try_write(block) {
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
                },
            }
            if *state == PlaybackState::Buffering {
                set_state(state, PlaybackState::Playing, event_tx);
                let _ = event_tx.send(PlaybackEvent::Buffering {
                    item_id,
                    active: false,
                });
            }
        },
        Ok(TrackBlockStatus::Pending) => {
            let item_id = current.item_id;
            set_state(state, PlaybackState::Buffering, event_tx);
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id,
                active: true,
            });
        },
        Ok(TrackBlockStatus::EndOfStream) => match drain_current_once(current) {
            Ok(DrainTurn::Produced(block)) => match current.output.try_write(block) {
                Ok(()) => {},
                Err(PendingWrite::Full(block)) => current.pending_block = Some(block),
                Err(PendingWrite::Closed) => begin_recovery(
                    config,
                    event_tx,
                    actor,
                    state,
                    PlaybackControlError::failed(FailureStage::Sink, "sink worker closed"),
                ),
            },
            Ok(DrainTurn::Pending) => {},
            Ok(DrainTurn::Complete) => promote_or_end(actor, state, config, event_tx),
            Err(error) => super::lifecycle::fail_current_error(actor, state, event_tx, error),
        },
        Err(PlaybackControlError::Failed(failure))
            if failure.code == FailureCode::Io && failure.stage == FailureStage::Decoder =>
        {
            begin_recovery(
                config,
                event_tx,
                actor,
                state,
                PlaybackControlError::Failed(failure),
            );
        },
        Err(error) => super::lifecycle::fail_current_error(actor, state, event_tx, error),
    }
}

/// Captures consumed position and schedules recovery when source capabilities allow it.
pub(super) fn begin_recovery(
    _config: &PlaybackRuntimeConfig,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
    error: PlaybackControlError,
) {
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    let capabilities = current.recovery_plan.item.source.descriptor().capabilities;
    if !capabilities.reopenable
        || !capabilities.byte_seekable
        || actor.policies.max_recovery_attempts == 0
    {
        super::lifecycle::fail_current_error(actor, state, event_tx, error);
        return;
    }
    let checkpoint_frame = current.consumed_position_frame();
    let checkpoint =
        MediaTime::from_frames(checkpoint_frame, current.pipeline.mix_format.sample_rate);
    let item_id = current.item_id;
    let plan = current.recovery_plan.clone();
    let _ = current.output.pause();

    if let Some(pending) = actor.pending_preparation.take() {
        pending.cancellation.cancel();
    }
    actor.crossfade = None;
    actor.force_transition = false;
    if let Some(pending) = actor.pending_seek.take() {
        let _ = pending.response.send(Err(PlaybackControlError::Closed));
    }
    set_state(state, PlaybackState::Recovering, event_tx);
    actor.next_preparation_id = actor.next_preparation_id.wrapping_add(1);
    actor.pending_recovery = Some(RecoveryPreparation {
        plan,
        id: actor.next_preparation_id,
        generation: actor.generation,
        purpose: PreparationPurpose::Recovery {
            item_id,
            checkpoint,
            attempt: 1,
        },
        cancellation: stellatune_audio_core::source::SourceCancellation::default(),
    });
}

/// Promotes a prepared successor or drains and completes the current item.
pub(super) fn promote_or_end(
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
    config: &PlaybackRuntimeConfig,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    actor.force_transition = false;
    let Some(mut ended) = actor.current.take() else {
        return;
    };
    let ended_item_id = ended.item_id;
    let Some(mut next) = actor.next.take() else {
        if actor.next.pending().is_some() {
            actor.current = Some(ended);
            set_state(state, PlaybackState::Buffering, event_tx);
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id: ended_item_id,
                active: true,
            });
            return;
        }
        match ended.output.drain() {
            Ok(false) => {
                actor.current = Some(ended);
                return;
            },
            Err(error) => {
                actor.current = Some(ended);
                super::lifecycle::fail_current_error(actor, state, event_tx, error);
                return;
            },
            Ok(true) => {},
        }
        ended.output.shutdown();
        set_state(state, PlaybackState::Idle, event_tx);
        let _ = event_tx.send(PlaybackEvent::PlaybackEnded {
            item_id: ended_item_id,
        });
        return;
    };

    let normalized = normalize_prepared_for_mix(&mut next, ended.pipeline.mix_format).is_ok();
    let current_key = ended
        .sink_factory
        .compatibility_key(ended.output_format)
        .ok();
    let next_key = next.plan.sink.compatibility_key(next.output_format).ok();
    let compatible = normalized
        && ended.pipeline.mix_format == next.pipeline.mix_format
        && ended.output_format == next.output_format
        && current_key.is_some()
        && current_key == next_key;
    if compatible {
        let clock = ended.output.clock();
        let next_base = clock.consumed_frames.saturating_add(clock.buffered_frames);
        let next_item_id = next.plan.item.id;
        if let Err(error) = ended.output.mark_boundary(next_item_id) {
            actor.current = Some(ended);
            fail_current_error(actor, state, event_tx, error);
            return;
        }
        ended.pipeline.decoder.reset();
        for transform in &mut ended.pipeline.pre_mix_transforms {
            transform.reset();
        }
        if let Some(normalizer) = ended.pipeline.normalizer.as_mut() {
            normalizer.reset();
        }
        let fade_in_frames = transition_fade_in_frames(ended.transition);
        let output = ended.output;
        let promoted = activate_with_output(
            next,
            output,
            ended.post_mix_transforms,
            next_base,
            false,
            fade_in_frames,
        );
        actor.current = Some(promoted);
        set_state(state, PlaybackState::Playing, event_tx);
    } else {
        match ended.output.drain() {
            Ok(false) => {
                actor.current = Some(ended);
                actor.next = super::state::NextTrack::Ready(Box::new(next));
                return;
            },
            Err(error) => {
                actor.current = Some(ended);
                fail_current_error(actor, state, event_tx, error);
                return;
            },
            Ok(true) => {},
        }
        ended.output.shutdown();
        match activate(next, config, actor.output_gain, &actor.output_workers) {
            Ok(promoted) => {
                let item_id = promoted.item_id;
                let mut promoted = promoted;
                promoted.fade_in_frames = transition_fade_in_frames(ended.transition);
                let _ = promoted.output.resume();
                actor.current = Some(promoted);
                set_state(state, PlaybackState::Playing, event_tx);
                let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
            },
            Err(error) => {
                actor.current = Some(ended);
                fail_current_error(actor, state, event_tx, error);
            },
        }
    }
}

/// Progress made by one turn of ordered pipeline draining.
pub(super) enum DrainTurn {
    Produced(AudioBlock),
    Pending,
    Complete,
}

/// Advances the current pipeline through one drain stage or output block.
pub(super) fn drain_current_once(
    current: &mut ActiveTrack,
) -> Result<DrainTurn, PlaybackControlError> {
    loop {
        match current.drain_phase {
            DrainPhase::Decoding => current.drain_phase = DrainPhase::PreMix(0),
            DrainPhase::PreMix(index) if index >= current.pipeline.pre_mix_transforms.len() => {
                current.drain_phase = DrainPhase::Normalizer;
            },
            DrainPhase::PreMix(index) => {
                let mut block =
                    AudioBlock::new(current.pipeline.pre_mix_transforms[index].output_format);
                block.timeline.start_frame = current.pipeline.produced_audible_frame;
                block.timeline.epoch = current.output.epoch();
                match current.pipeline.pre_mix_transforms[index].drain(&mut block)? {
                    DrainStatus::Complete => {
                        current.drain_phase = DrainPhase::PreMix(index + 1);
                    },
                    DrainStatus::Produced => {
                        block.format = current.pipeline.pre_mix_transforms[index].output_format;
                        process_transform_chain(
                            &mut current.pipeline.pre_mix_transforms[index + 1..],
                            &mut block,
                        )?;
                        if block.samples.is_empty() {
                            return Ok(DrainTurn::Pending);
                        }
                        if let Some(normalizer) = current.pipeline.normalizer.as_mut() {
                            normalizer.process(&mut block)?;
                        }
                        apply_track_transition_gain(current, &mut block);
                        current.pipeline.produced_audible_frame = current
                            .pipeline
                            .produced_audible_frame
                            .saturating_add(block.frames() as u64);
                        process_transform_chain(&mut current.post_mix_transforms, &mut block)?;
                        return Ok(if block.samples.is_empty() {
                            DrainTurn::Pending
                        } else {
                            DrainTurn::Produced(block)
                        });
                    },
                }
            },
            DrainPhase::Normalizer => {
                let Some(normalizer) = current.pipeline.normalizer.as_mut() else {
                    current.drain_phase = DrainPhase::PostMix(0);
                    continue;
                };
                let mut block = AudioBlock::new(current.pipeline.mix_format);
                block.timeline.start_frame = current.pipeline.produced_audible_frame;
                block.timeline.epoch = current.output.epoch();
                if !normalizer.drain(&mut block)? {
                    current.drain_phase = DrainPhase::PostMix(0);
                    continue;
                }
                apply_track_transition_gain(current, &mut block);
                current.pipeline.produced_audible_frame = current
                    .pipeline
                    .produced_audible_frame
                    .saturating_add(block.frames() as u64);
                process_transform_chain(&mut current.post_mix_transforms, &mut block)?;
                return Ok(if block.samples.is_empty() {
                    DrainTurn::Pending
                } else {
                    DrainTurn::Produced(block)
                });
            },
            DrainPhase::PostMix(index) if index >= current.post_mix_transforms.len() => {
                current.drain_phase = DrainPhase::Complete;
            },
            DrainPhase::PostMix(index) => {
                let mut block = AudioBlock::new(current.post_mix_transforms[index].output_format);
                block.timeline.start_frame = current.pipeline.produced_audible_frame;
                block.timeline.epoch = current.output.epoch();
                match current.post_mix_transforms[index].drain(&mut block)? {
                    DrainStatus::Complete => {
                        current.drain_phase = DrainPhase::PostMix(index + 1);
                    },
                    DrainStatus::Produced => {
                        block.format = current.post_mix_transforms[index].output_format;
                        process_transform_chain(
                            &mut current.post_mix_transforms[index + 1..],
                            &mut block,
                        )?;
                        return Ok(if block.samples.is_empty() {
                            DrainTurn::Pending
                        } else {
                            DrainTurn::Produced(block)
                        });
                    },
                }
            },
            DrainPhase::Complete => return Ok(DrainTurn::Complete),
        }
    }
}

/// Advances the secondary crossfade pipeline by one decoded block.
pub(super) fn decode_secondary_block(
    next: &mut SecondaryTrack,
    block_frames: usize,
    epoch: u64,
) -> Result<TrackBlockStatus, PlaybackControlError> {
    next.pipeline.decode(block_frames, epoch)
}

/// Publishes sink-consumed position when its cadence advances or when forced.
pub(super) fn emit_position_if_due(
    current: &mut ActiveTrack,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    force: bool,
) {
    let position_frame = current.consumed_position_frame();
    if !current.boundary_announced {
        return;
    }
    let report_interval = u64::from(current.pipeline.mix_format.sample_rate.max(1)) / 20;
    if !force
        && position_frame
            < current
                .last_reported_position_frame
                .saturating_add(report_interval.max(1))
    {
        return;
    }
    current.last_reported_position_frame = position_frame;
    let _ = event_tx.send(PlaybackEvent::Position {
        item_id: current.item_id,
        position: MediaTime::from_frames(position_frame, current.pipeline.mix_format.sample_rate),
    });
}
