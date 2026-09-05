//! Asynchronous source acquisition followed by cancellable blocking stage setup.
//! Sources use the caller's long-lived executor; decoder work runs off the actor.

use super::{
    normalizer::PcmNormalizer,
    pipeline::{ConfiguredTransform, TrackPipeline},
    state::{PreparationPurpose, PreparationResult, PreparedTrack},
};
use crate::planner::{ExecutablePlaybackPlan, can_fallback};
use std::time::{Duration, Instant};
use stellatune_audio_core::{
    decoder::DecoderSeekStatus,
    error::{FailureStage, PlaybackControlError},
    playback::MediaTime,
    source::{SourceCancellation, SourceOpenPurpose, SourceOpenRequest},
    transform::TransformPlacement,
};

/// Prepares one item on the existing async executor and the blocking stage pool.
/// Source open, fallback, backoff, and recovery seek share one cancellation token
/// and deadline. Blocking stage calls must return before cancellation is observed.
pub(super) async fn prepare_off_turn(
    plan: ExecutablePlaybackPlan,
    id: u64,
    generation: u64,
    purpose: SourceOpenPurpose,
    preparation_purpose: PreparationPurpose,
    cancellation: SourceCancellation,
    deadline: Instant,
) -> PreparationResult {
    let item_id = plan.item.id;
    let recovery = match preparation_purpose {
        PreparationPurpose::Recovery {
            checkpoint,
            attempt,
            ..
        } => Some((checkpoint, attempt)),
        _ => None,
    };
    let work = async {
        check_active(&cancellation, deadline)?;
        if let Some((_, attempt)) = recovery {
            let delay = plan
                .policies
                .recovery_backoff_ms
                .saturating_mul(attempt.saturating_sub(1) as u64);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        prepare_track(
            plan,
            purpose,
            recovery.map(|(checkpoint, _)| checkpoint),
            cancellation.clone(),
            deadline,
        )
        .await
    };
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(PlaybackControlError::Closed),
        _ = tokio::time::sleep_until(deadline.into()) => { cancellation.cancel(); Err(PlaybackControlError::CommandTimeout { operation: "preparation" }) },
        result = work => result,
    }.map_err(|error| error.with_context(Some(item_id), generation));
    PreparationResult {
        id,
        generation,
        purpose: preparation_purpose,
        result,
    }
}

fn check_active(
    cancellation: &SourceCancellation,
    deadline: Instant,
) -> Result<(), PlaybackControlError> {
    if cancellation.is_cancelled() {
        return Err(PlaybackControlError::Closed);
    }
    if Instant::now() >= deadline {
        return Err(PlaybackControlError::CommandTimeout {
            operation: "preparation",
        });
    }
    Ok(())
}

async fn prepare_track(
    plan: ExecutablePlaybackPlan,
    purpose: SourceOpenPurpose,
    resume_position: Option<MediaTime>,
    cancellation: SourceCancellation,
    deadline: Instant,
) -> Result<PreparedTrack, PlaybackControlError> {
    let capabilities = plan.item.source.descriptor().capabilities;
    let limit = plan
        .policies
        .max_decoder_fallbacks
        .max(1)
        .min(plan.decoder_candidates.len());
    let mut last_error = None;
    for index in 0..limit {
        check_active(&cancellation, deadline)?;
        if !can_fallback(capabilities, index) {
            break;
        }
        let source = plan
            .item
            .source
            .open(SourceOpenRequest {
                purpose,
                deadline: Some(deadline),
                cancellation: cancellation.clone(),
            })
            .await
            .map_err(PlaybackControlError::source)?;
        check_active(&cancellation, deadline)?;
        let plan = plan.clone();
        let cancellation = cancellation.clone();
        let result = tokio::task::spawn_blocking(move || {
            check_active(&cancellation, deadline)?;
            let factory = plan.decoder_candidates[index].clone();
            let hints = plan.item.source.descriptor().media;
            let mut decoder = factory.create().map_err(|error| {
                PlaybackControlError::factory(
                    FailureStage::Decoder,
                    factory.descriptor().id.clone(),
                    error,
                )
            })?;
            let info = decoder.open(source, &hints).map_err(|error| {
                PlaybackControlError::decoder(error, factory.descriptor().id.clone())
            })?;
            check_active(&cancellation, deadline)?;
            let decoded_format = info.format;
            let mut mix_format = decoded_format;
            let mut pre_mix_transforms = Vec::new();
            let mut post_mix_factories = Vec::new();
            for transform_factory in &plan.transforms {
                check_active(&cancellation, deadline)?;
                match transform_factory.descriptor().placement {
                    TransformPlacement::PreMix => {},
                    TransformPlacement::PostMix => {
                        post_mix_factories.push(transform_factory);
                        continue;
                    },
                }
                let mut transform = transform_factory.create().map_err(|error| {
                    PlaybackControlError::factory(
                        FailureStage::Transform,
                        transform_factory.descriptor().id.clone(),
                        error,
                    )
                })?;
                mix_format = transform.configure(mix_format).map_err(|error| {
                    PlaybackControlError::transform(
                        error,
                        transform_factory.descriptor().id.clone(),
                    )
                })?;
                pre_mix_transforms.push(ConfiguredTransform::new(
                    transform,
                    mix_format,
                    transform_factory.descriptor().id.clone(),
                ));
            }
            let normalizer_input_format = mix_format;
            let preferred_mix_format = plan.sink.preferred_format(mix_format).map_err(|error| {
                PlaybackControlError::factory(FailureStage::Sink, plan.sink.id().clone(), error)
            })?;
            preferred_mix_format.validate().map_err(|message| {
                PlaybackControlError::failed(FailureStage::Sink, message.to_owned())
            })?;
            let normalizer = if preferred_mix_format == mix_format {
                None
            } else {
                let source = mix_format;
                mix_format = preferred_mix_format;
                Some(PcmNormalizer::new(source, mix_format)?)
            };
            let mut output_format = mix_format;
            let mut post_mix_transforms = Vec::with_capacity(post_mix_factories.len());
            for transform_factory in post_mix_factories {
                check_active(&cancellation, deadline)?;
                let mut transform = transform_factory.create().map_err(|error| {
                    PlaybackControlError::factory(
                        FailureStage::Transform,
                        transform_factory.descriptor().id.clone(),
                        error,
                    )
                })?;
                output_format = transform.configure(output_format).map_err(|error| {
                    PlaybackControlError::transform(
                        error,
                        transform_factory.descriptor().id.clone(),
                    )
                })?;
                post_mix_transforms.push(ConfiguredTransform::new(
                    transform,
                    output_format,
                    transform_factory.descriptor().id.clone(),
                ));
            }
            let trim_head_frames = info
                .gapless_trim
                .map(|trim| u64::from(trim.head_frames))
                .unwrap_or(0);
            let trim_tail_frames = info
                .gapless_trim
                .map(|trim| u64::from(trim.tail_frames))
                .unwrap_or(0);
            let duration_frames = info.duration_frames.map(|duration| {
                let decoded_frames =
                    duration.saturating_sub(trim_head_frames.saturating_add(trim_tail_frames));
                MediaTime::from_frames(decoded_frames, decoded_format.sample_rate)
                    .to_frames(mix_format.sample_rate)
            });
            let (initial_decoded_frame, initial_audible_frame) = if let Some(position) =
                resume_position
            {
                if !capabilities.byte_seekable {
                    return Err(PlaybackControlError::Unsupported);
                }
                let target = position
                    .to_frames(decoded_format.sample_rate)
                    .saturating_add(trim_head_frames);
                let actual = match decoder.start_seek(target) {
                    Ok(DecoderSeekStatus::Complete(result)) => result.actual_frame,
                    Ok(DecoderSeekStatus::Pending) => loop {
                        check_active(&cancellation, deadline)?;
                        match decoder.continue_seek() {
                            Ok(DecoderSeekStatus::Complete(result)) => break result.actual_frame,
                            Ok(DecoderSeekStatus::Pending) => {
                                std::thread::sleep(Duration::from_millis(1))
                            },
                            Err(error) => {
                                return Err(PlaybackControlError::decoder(
                                    error,
                                    factory.descriptor().id.clone(),
                                ));
                            },
                        }
                    },
                    Err(error) => {
                        return Err(PlaybackControlError::decoder(
                            error,
                            factory.descriptor().id.clone(),
                        ));
                    },
                };
                let audible_decoded = actual.saturating_sub(trim_head_frames);
                let audible_mix =
                    MediaTime::from_frames(audible_decoded, decoded_format.sample_rate)
                        .to_frames(mix_format.sample_rate);
                (actual, audible_mix)
            } else {
                (0, 0)
            };
            Ok(PreparedTrack {
                plan,
                post_mix_transforms,
                output_format,
                pipeline: TrackPipeline {
                    decoder,
                    decoder_id: factory.descriptor().id.clone(),
                    pre_mix_transforms,
                    decoded_format,
                    mix_format,
                    normalizer,
                    normalizer_input_format,
                    duration_frames,
                    trim_head_frames,
                    trim_tail_frames,
                    raw_duration_frames: info.duration_frames,
                    tail_buffer: Vec::new(),
                    decoded_frame: initial_decoded_frame,
                    produced_audible_frame: initial_audible_frame,
                },
            })
        })
        .await
        .map_err(|error| PlaybackControlError::failed(FailureStage::Runtime, error.to_string()))?;
        match result {
            Ok(prepared) => return Ok(prepared),
            Err(PlaybackControlError::Failed(failure))
                if failure.stage == FailureStage::Decoder =>
            {
                last_error = Some(PlaybackControlError::Failed(failure));
            },
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        PlaybackControlError::failed(FailureStage::Decoder, "no decoder candidate")
    }))
}
