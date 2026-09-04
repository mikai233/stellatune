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

use stellatune_audio_core::{
    decoder::{DecodeStatus, DecoderStage},
    error::{PlaybackControlError, TransformError},
    format::{AudioBlock, PcmFormat},
    playback::MediaTime,
    transform::{DrainStatus, TransformStage, TransformStatus},
};
use tokio::sync::broadcast;

use super::actor::advance_generation;
use super::event::{PlaybackEvent, PlaybackState};
use super::lifecycle::{fail_current, fail_promoted, set_state};
use super::normalizer::PcmNormalizer;
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
) -> Result<ActiveTrack, PlaybackControlError> {
    let recovery_plan = prepared.plan.clone();
    let sink_factory = Arc::clone(&prepared.plan.sink);
    let output = SinkWorker::start(
        Arc::clone(&sink_factory),
        prepared.output_format,
        config.pcm_ring_blocks,
        output_gain,
    )?;
    Ok(ActiveTrack {
        recovery_plan,
        item_id: prepared.plan.item.id,
        decoder: prepared.decoder,
        pre_mix_transforms: prepared.pre_mix_transforms,
        pre_mix_formats: prepared.pre_mix_formats,
        post_mix_transforms: prepared.post_mix_transforms,
        post_mix_formats: prepared.post_mix_formats,
        decoded_format: prepared.decoded_format,
        mix_format: prepared.mix_format,
        output_format: prepared.output_format,
        normalizer: prepared.normalizer,
        duration_frames: prepared.duration_frames,
        trim_head_frames: prepared.trim_head_frames,
        trim_tail_frames: prepared.trim_tail_frames,
        raw_duration_frames: prepared.raw_duration_frames,
        tail_buffer: Vec::new(),
        decoded_frame: prepared.initial_decoded_frame,
        produced_audible_frame: prepared.initial_audible_frame,
        position_base_frame: prepared.initial_audible_frame,
        last_reported_position_frame: prepared.initial_audible_frame,
        epoch: 0,
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
    if let Some(message) = actor
        .current
        .as_ref()
        .and_then(|current| current.output.try_failure())
    {
        begin_recovery(config, event_tx, actor, state, "sink", message);
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
    while let Some(item_id) = current.output.try_boundary() {
        if item_id == current.item_id && !current.boundary_announced {
            current.boundary_announced = true;
            let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
        }
    }
    emit_position_if_due(current, event_tx, false);
    if current
        .forced_end_frame
        .is_some_and(|end| current.produced_audible_frame >= end)
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
                    "sink",
                    "sink worker closed".to_owned(),
                );
                return;
            },
        }
    }
    let mut block = AudioBlock::new(current.decoded_format);
    block.timeline.start_frame = current.decoded_frame;
    block.timeline.epoch = current.epoch;
    block
        .samples
        .reserve(config.block_frames.saturating_mul(usize::from(
            current.decoded_format.channel_layout.channel_count(),
        )));
    match current.decoder.decode(&mut block) {
        Ok(DecodeStatus::Produced { frames }) => {
            if frames == 0 || block.samples.is_empty() {
                return;
            }
            let raw_frames = block.frames() as u64;
            let raw_start = current.decoded_frame;
            current.decoded_frame = current.decoded_frame.saturating_add(raw_frames);
            trim_gapless_block(current, &mut block, raw_start);
            if block.samples.is_empty() {
                return;
            }
            block.timeline.start_frame = current.produced_audible_frame;
            if let Err(error) = process_transform_chain(
                &mut current.pre_mix_transforms,
                &current.pre_mix_formats,
                &mut block,
            ) {
                fail_current(actor, state, event_tx, "transform", error.to_string());
                return;
            }
            if block.samples.is_empty() {
                return;
            }
            if let Some(normalizer) = current.normalizer.as_mut()
                && let Err(error) = normalizer.process(&mut block)
            {
                fail_current(actor, state, event_tx, "normalizer", error.to_string());
                return;
            }
            apply_track_transition_gain(current, &mut block);
            current.produced_audible_frame = current
                .produced_audible_frame
                .saturating_add(block.frames() as u64);
            if let Err(error) = process_transform_chain(
                &mut current.post_mix_transforms,
                &current.post_mix_formats,
                &mut block,
            ) {
                fail_current(actor, state, event_tx, "transform", error.to_string());
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
                        "sink",
                        "sink worker closed".to_owned(),
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
        Ok(DecodeStatus::Pending) => {
            let item_id = current.item_id;
            set_state(state, PlaybackState::Buffering, event_tx);
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id,
                active: true,
            });
        },
        Ok(DecodeStatus::EndOfStream) => match drain_current_once(current) {
            Ok(DrainTurn::Produced(block)) => match current.output.try_write(block) {
                Ok(()) => {},
                Err(PendingWrite::Full(block)) => current.pending_block = Some(block),
                Err(PendingWrite::Closed) => begin_recovery(
                    config,
                    event_tx,
                    actor,
                    state,
                    "sink",
                    "sink worker closed".to_owned(),
                ),
            },
            Ok(DrainTurn::Pending) => {},
            Ok(DrainTurn::Complete) => promote_or_end(actor, state, config, event_tx),
            Err(error) => fail_current(actor, state, event_tx, "transform", error.to_string()),
        },
        Err(stellatune_audio_core::error::DecodeError::Io(error)) => {
            begin_recovery(config, event_tx, actor, state, "decoder", error.to_string())
        },
        Err(error) => fail_current(actor, state, event_tx, "decoder", error.to_string()),
    }
}

/// Captures consumed position and schedules recovery when source capabilities allow it.
pub(super) fn begin_recovery(
    _config: &PlaybackRuntimeConfig,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut PlaybackSession,
    state: &mut PlaybackState,
    stage: &'static str,
    message: String,
) {
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    let capabilities = current.recovery_plan.item.source.descriptor().capabilities;
    if !capabilities.reopenable
        || !capabilities.byte_seekable
        || actor.policies.max_recovery_attempts == 0
    {
        fail_current(actor, state, event_tx, stage, message);
        return;
    }
    let clock = current.output.clock();
    let checkpoint_frame = current.position_base_frame.saturating_add(
        clock
            .consumed_frames
            .saturating_sub(current.sink_consumed_base_frame),
    );
    let checkpoint = MediaTime::from_frames(checkpoint_frame, current.mix_format.sample_rate);
    let item_id = current.item_id;
    let plan = current.recovery_plan.clone();
    let _ = current.output.pause();

    advance_generation(actor);
    actor.next = None;
    actor.next_preparing = false;
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
            resume_state: PlaybackState::Playing,
            attempt: 1,
        },
        cancellation: actor.preparation_cancellation.clone(),
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
        if actor.next_preparing {
            actor.current = Some(ended);
            set_state(state, PlaybackState::Buffering, event_tx);
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id: ended_item_id,
                active: true,
            });
            return;
        }
        let _ = ended.output.drain();
        ended.output.shutdown();
        set_state(state, PlaybackState::Idle, event_tx);
        let _ = event_tx.send(PlaybackEvent::PlaybackEnded {
            item_id: ended_item_id,
        });
        return;
    };

    let normalized = normalize_prepared_for_mix(&mut next, ended.mix_format).is_ok();
    let current_key = ended
        .sink_factory
        .compatibility_key(ended.output_format)
        .ok();
    let next_key = next.plan.sink.compatibility_key(next.output_format).ok();
    let compatible = normalized
        && ended.mix_format == next.mix_format
        && ended.output_format == next.output_format
        && current_key.is_some()
        && current_key == next_key;
    if compatible {
        let clock = ended.output.clock();
        let next_base = clock.consumed_frames.saturating_add(clock.buffered_frames);
        let next_item_id = next.plan.item.id;
        if ended.output.mark_boundary(next_item_id).is_err() {
            ended.output.shutdown();
            fail_promoted(
                actor,
                state,
                event_tx,
                "failed to queue item boundary".to_owned(),
            );
            return;
        }
        ended.decoder.reset();
        for transform in &mut ended.pre_mix_transforms {
            transform.reset();
        }
        if let Some(normalizer) = ended.normalizer.as_mut() {
            normalizer.reset();
        }
        let fade_in_frames = transition_fade_in_frames(ended.transition);
        let output = ended.output;
        let promoted = activate_with_output(
            next,
            output,
            ended.post_mix_transforms,
            ended.post_mix_formats,
            next_base,
            false,
            fade_in_frames,
        );
        actor.current = Some(promoted);
        set_state(state, PlaybackState::Playing, event_tx);
    } else {
        let _ = ended.output.drain();
        ended.output.shutdown();
        match activate(next, config, actor.output_gain) {
            Ok(promoted) => {
                let item_id = promoted.item_id;
                let mut promoted = promoted;
                promoted.fade_in_frames = transition_fade_in_frames(ended.transition);
                let _ = promoted.output.resume();
                actor.current = Some(promoted);
                set_state(state, PlaybackState::Playing, event_tx);
                let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
            },
            Err(error) => fail_promoted(actor, state, event_tx, error.to_string()),
        }
    }
}

/// Output of one bounded secondary-track decode operation.
pub(super) enum TrackBlockStatus {
    Data(AudioBlock),
    Pending,
    EndOfStream,
}

/// Decodes, gapless-trims, transforms, and normalizes one track block.
#[allow(clippy::too_many_arguments)]
pub(super) fn decode_track_block(
    decoder: &mut dyn DecoderStage,
    transforms: &mut [Box<dyn TransformStage>],
    transform_formats: &[PcmFormat],
    normalizer: &mut Option<PcmNormalizer>,
    format: PcmFormat,
    trim_head_frames: u64,
    trim_tail_frames: u64,
    raw_duration_frames: Option<u64>,
    tail_buffer: &mut Vec<f32>,
    decoded_frame: &mut u64,
    produced_audible_frame: &mut u64,
    block_frames: usize,
    epoch: u64,
) -> Result<TrackBlockStatus, PlaybackControlError> {
    let mut block = AudioBlock::new(format);
    block.timeline.start_frame = *decoded_frame;
    block.timeline.epoch = epoch;
    block
        .samples
        .reserve(block_frames.saturating_mul(usize::from(format.channel_layout.channel_count())));
    match decoder
        .decode(&mut block)
        .map_err(|error| PlaybackControlError::failed("decoder", error.to_string()))?
    {
        DecodeStatus::Produced { frames } if frames > 0 && !block.samples.is_empty() => {
            let raw_start = *decoded_frame;
            *decoded_frame = decoded_frame.saturating_add(block.frames() as u64);
            trim_gapless_samples(
                &mut block,
                raw_start,
                trim_head_frames,
                trim_tail_frames,
                raw_duration_frames,
                tail_buffer,
            );
            if block.samples.is_empty() {
                return Ok(TrackBlockStatus::Pending);
            }
            block.timeline.start_frame = *produced_audible_frame;
            process_transform_chain(transforms, transform_formats, &mut block)
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            if block.samples.is_empty() {
                return Ok(TrackBlockStatus::Pending);
            }
            if let Some(normalizer) = normalizer.as_mut() {
                normalizer.process(&mut block)?;
            }
            if block.samples.is_empty() {
                return Ok(TrackBlockStatus::Pending);
            }
            *produced_audible_frame = produced_audible_frame.saturating_add(block.frames() as u64);
            Ok(TrackBlockStatus::Data(block))
        },
        DecodeStatus::Produced { .. } | DecodeStatus::Pending => Ok(TrackBlockStatus::Pending),
        DecodeStatus::EndOfStream => Ok(TrackBlockStatus::EndOfStream),
    }
}

/// Runs a block through an ordered transform suffix until output or buffering.
pub(super) fn process_transform_chain(
    transforms: &mut [Box<dyn TransformStage>],
    formats: &[PcmFormat],
    block: &mut AudioBlock,
) -> Result<(), TransformError> {
    debug_assert_eq!(transforms.len(), formats.len());
    for (transform, output_format) in transforms.iter_mut().zip(formats) {
        match transform.process(block)? {
            TransformStatus::Produced => block.format = *output_format,
            TransformStatus::Buffered => {
                block.samples.clear();
                return Ok(());
            },
        }
    }
    Ok(())
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
            DrainPhase::PreMix(index) if index >= current.pre_mix_transforms.len() => {
                current.drain_phase = DrainPhase::Normalizer;
            },
            DrainPhase::PreMix(index) => {
                let mut block = AudioBlock::new(current.pre_mix_formats[index]);
                block.timeline.start_frame = current.produced_audible_frame;
                block.timeline.epoch = current.epoch;
                match current.pre_mix_transforms[index]
                    .drain(&mut block)
                    .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?
                {
                    DrainStatus::Complete => {
                        current.drain_phase = DrainPhase::PreMix(index + 1);
                    },
                    DrainStatus::Produced => {
                        block.format = current.pre_mix_formats[index];
                        process_transform_chain(
                            &mut current.pre_mix_transforms[index + 1..],
                            &current.pre_mix_formats[index + 1..],
                            &mut block,
                        )
                        .map_err(|error| {
                            PlaybackControlError::failed("transform", error.to_string())
                        })?;
                        if block.samples.is_empty() {
                            return Ok(DrainTurn::Pending);
                        }
                        if let Some(normalizer) = current.normalizer.as_mut() {
                            normalizer.process(&mut block)?;
                        }
                        apply_track_transition_gain(current, &mut block);
                        current.produced_audible_frame = current
                            .produced_audible_frame
                            .saturating_add(block.frames() as u64);
                        process_transform_chain(
                            &mut current.post_mix_transforms,
                            &current.post_mix_formats,
                            &mut block,
                        )
                        .map_err(|error| {
                            PlaybackControlError::failed("transform", error.to_string())
                        })?;
                        return Ok(if block.samples.is_empty() {
                            DrainTurn::Pending
                        } else {
                            DrainTurn::Produced(block)
                        });
                    },
                }
            },
            DrainPhase::Normalizer => {
                let Some(normalizer) = current.normalizer.as_mut() else {
                    current.drain_phase = DrainPhase::PostMix(0);
                    continue;
                };
                let mut block = AudioBlock::new(current.mix_format);
                block.timeline.start_frame = current.produced_audible_frame;
                block.timeline.epoch = current.epoch;
                if !normalizer.drain(&mut block)? {
                    current.drain_phase = DrainPhase::PostMix(0);
                    continue;
                }
                apply_track_transition_gain(current, &mut block);
                current.produced_audible_frame = current
                    .produced_audible_frame
                    .saturating_add(block.frames() as u64);
                process_transform_chain(
                    &mut current.post_mix_transforms,
                    &current.post_mix_formats,
                    &mut block,
                )
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
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
                let mut block = AudioBlock::new(current.post_mix_formats[index]);
                block.timeline.start_frame = current.produced_audible_frame;
                block.timeline.epoch = current.epoch;
                match current.post_mix_transforms[index]
                    .drain(&mut block)
                    .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?
                {
                    DrainStatus::Complete => {
                        current.drain_phase = DrainPhase::PostMix(index + 1);
                    },
                    DrainStatus::Produced => {
                        block.format = current.post_mix_formats[index];
                        process_transform_chain(
                            &mut current.post_mix_transforms[index + 1..],
                            &current.post_mix_formats[index + 1..],
                            &mut block,
                        )
                        .map_err(|error| {
                            PlaybackControlError::failed("transform", error.to_string())
                        })?;
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
    decode_track_block(
        next.decoder.as_mut(),
        &mut next.pre_mix_transforms,
        &next.pre_mix_formats,
        &mut next.normalizer,
        next.decoded_format,
        next.trim_head_frames,
        next.trim_tail_frames,
        next.raw_duration_frames,
        &mut next.tail_buffer,
        &mut next.decoded_frame,
        &mut next.produced_audible_frame,
        block_frames,
        epoch,
    )
}

/// Applies the active track's gapless head and tail trim to a decoded block.
pub(super) fn trim_gapless_block(
    current: &mut ActiveTrack,
    block: &mut AudioBlock,
    raw_start: u64,
) {
    trim_gapless_samples(
        block,
        raw_start,
        current.trim_head_frames,
        current.trim_tail_frames,
        current.raw_duration_frames,
        &mut current.tail_buffer,
    );
}

/// Removes encoder delay and withholds possible tail padding from raw PCM.
pub(super) fn trim_gapless_samples(
    block: &mut AudioBlock,
    raw_start: u64,
    trim_head_frames: u64,
    trim_tail_frames: u64,
    raw_duration_frames: Option<u64>,
    tail_buffer: &mut Vec<f32>,
) {
    let channels = usize::from(block.format.channel_layout.channel_count());
    let raw_end = raw_start.saturating_add(block.frames() as u64);
    let keep_start = raw_start.max(trim_head_frames);
    let known_keep_end =
        raw_duration_frames.map(|duration| duration.saturating_sub(trim_tail_frames));
    let keep_end = known_keep_end.map_or(raw_end, |end| raw_end.min(end));
    if keep_end <= keep_start {
        block.samples.clear();
        return;
    }
    let drop_head_frames = keep_start.saturating_sub(raw_start) as usize;
    let keep_frames = keep_end.saturating_sub(keep_start) as usize;
    let start_sample = drop_head_frames.saturating_mul(channels);
    let end_sample = start_sample.saturating_add(keep_frames.saturating_mul(channels));
    if start_sample > 0 || end_sample < block.samples.len() {
        block.samples = block.samples[start_sample..end_sample].to_vec();
    }

    if raw_duration_frames.is_none() && trim_tail_frames > 0 {
        tail_buffer.extend_from_slice(&block.samples);
        let held_samples = (trim_tail_frames as usize).saturating_mul(channels);
        if tail_buffer.len() <= held_samples {
            block.samples.clear();
        } else {
            let emit_samples = tail_buffer.len().saturating_sub(held_samples);
            block.samples = tail_buffer.drain(..emit_samples).collect();
        }
    }
}

/// Publishes sink-consumed position when its cadence advances or when forced.
pub(super) fn emit_position_if_due(
    current: &mut ActiveTrack,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    force: bool,
) {
    let position_frame = current.position_base_frame.saturating_add(
        current
            .output
            .clock()
            .consumed_frames
            .saturating_sub(current.sink_consumed_base_frame),
    );
    if !current.boundary_announced {
        return;
    }
    let report_interval = u64::from(current.mix_format.sample_rate.max(1)) / 20;
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
        position: MediaTime::from_frames(position_frame, current.mix_format.sample_rate),
    });
}
