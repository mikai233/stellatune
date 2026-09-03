use std::time::Duration;

use crossbeam_channel::Sender;
use stellatune_audio_core::{
    DecoderSeekStatus, MediaTime, PlaybackControlError, SourceCancellation, SourceOpenPurpose,
    SourceOpenRequest, TransformPlacement,
};

use crate::planner::{ExecutablePlaybackPlan, can_fallback};

use super::normalizer::PcmNormalizer;
use super::state::{PreparationKind, PreparationResult, PreparedTrack};
pub(super) fn spawn_preparation(
    plan: ExecutablePlaybackPlan,
    generation: u64,
    purpose: SourceOpenPurpose,
    kind: PreparationKind,
    sender: Sender<PreparationResult>,
    cancellation: SourceCancellation,
) {
    std::thread::spawn(move || {
        let item_id = plan.item.id;
        let recovery = match &kind {
            PreparationKind::Recovery {
                checkpoint,
                attempt,
                ..
            } => Some((*checkpoint, *attempt)),
            _ => None,
        };
        if let Some((_, attempt)) = recovery {
            let backoff_ms = plan
                .policies
                .recovery_backoff_ms
                .saturating_mul(attempt.saturating_sub(1) as u64);
            if backoff_ms > 0 {
                let deadline = std::time::Instant::now() + Duration::from_millis(backoff_ms);
                while std::time::Instant::now() < deadline {
                    if cancellation.is_cancelled() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        let result = if cancellation.is_cancelled() {
            Err(PlaybackControlError::Closed)
        } else {
            prepare_track(
                plan,
                purpose,
                recovery.map(|(checkpoint, _)| checkpoint),
                cancellation,
            )
            .map_err(|error| error.with_context(Some(item_id), generation))
        };
        let _ = sender.send(PreparationResult {
            generation,
            kind,
            result,
        });
    });
}

fn prepare_track(
    plan: ExecutablePlaybackPlan,
    purpose: SourceOpenPurpose,
    resume_position: Option<MediaTime>,
    cancellation: SourceCancellation,
) -> Result<PreparedTrack, PlaybackControlError> {
    let capabilities = plan.item.source.descriptor().capabilities;
    let hints = plan.item.source.descriptor().media;
    let mut last_error = None;
    let fallback_limit = plan
        .policies
        .max_decoder_fallbacks
        .min(plan.decoder_candidates.len());

    for (index, factory) in plan
        .decoder_candidates
        .iter()
        .take(fallback_limit.max(1))
        .enumerate()
    {
        if !can_fallback(capabilities, index) {
            break;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| PlaybackControlError::failed("runtime", error.to_string()))?;
        let source = match runtime.block_on(plan.item.source.open(SourceOpenRequest {
            purpose,
            deadline: None,
            cancellation: cancellation.clone(),
        })) {
            Ok(source) => source,
            Err(error) => {
                last_error = Some(("source", error.to_string()));
                continue;
            },
        };
        let mut decoder = match factory.create() {
            Ok(decoder) => decoder,
            Err(error) => {
                last_error = Some(("decoder", error.to_string()));
                continue;
            },
        };
        let info = match decoder.open(source, &hints) {
            Ok(info) => info,
            Err(error) => {
                last_error = Some(("decoder", error.to_string()));
                continue;
            },
        };
        let decoded_format = info.format;
        let mut mix_format = decoded_format;
        let mut pre_mix_transforms = Vec::new();
        let mut pre_mix_formats = Vec::new();
        let mut post_mix_factories = Vec::new();
        for transform_factory in &plan.transforms {
            match transform_factory.descriptor().placement {
                TransformPlacement::PreMix => {},
                TransformPlacement::PostMix => {
                    post_mix_factories.push(transform_factory);
                    continue;
                },
            }
            let mut transform = transform_factory
                .create()
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            mix_format = transform
                .configure(mix_format)
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            pre_mix_transforms.push(transform);
            pre_mix_formats.push(mix_format);
        }
        let preferred_mix_format = plan
            .sink
            .preferred_format(mix_format)
            .map_err(|error| PlaybackControlError::failed("sink", error.to_string()))?;
        preferred_mix_format
            .validate()
            .map_err(|message| PlaybackControlError::failed("sink", message.to_owned()))?;
        let normalizer = if preferred_mix_format == mix_format {
            None
        } else {
            let source = mix_format;
            mix_format = preferred_mix_format;
            Some(PcmNormalizer::new(source, mix_format)?)
        };
        let mut output_format = mix_format;
        let mut post_mix_transforms = Vec::with_capacity(post_mix_factories.len());
        let mut post_mix_formats = Vec::with_capacity(post_mix_factories.len());
        for transform_factory in post_mix_factories {
            let mut transform = transform_factory
                .create()
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            output_format = transform
                .configure(output_format)
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            post_mix_transforms.push(transform);
            post_mix_formats.push(output_format);
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
        let (initial_decoded_frame, initial_audible_frame) = if let Some(position) = resume_position
        {
            if !capabilities.byte_seekable {
                return Err(PlaybackControlError::Unsupported);
            }
            let target = position
                .to_frames(decoded_format.sample_rate)
                .saturating_add(trim_head_frames);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let actual = match decoder.start_seek(target) {
                Ok(DecoderSeekStatus::Complete(result)) => result.actual_frame,
                Ok(DecoderSeekStatus::Pending) => loop {
                    if std::time::Instant::now() >= deadline {
                        return Err(PlaybackControlError::failed(
                            "decoder",
                            "recovery seek timed out".to_owned(),
                        ));
                    }
                    match decoder.continue_seek() {
                        Ok(DecoderSeekStatus::Complete(result)) => break result.actual_frame,
                        Ok(DecoderSeekStatus::Pending) => std::thread::yield_now(),
                        Err(error) => {
                            return Err(PlaybackControlError::failed("decoder", error.to_string()));
                        },
                    }
                },
                Err(error) => {
                    return Err(PlaybackControlError::failed("decoder", error.to_string()));
                },
            };
            let audible_decoded = actual.saturating_sub(trim_head_frames);
            let audible_mix = MediaTime::from_frames(audible_decoded, decoded_format.sample_rate)
                .to_frames(mix_format.sample_rate);
            (actual, audible_mix)
        } else {
            (0, 0)
        };
        return Ok(PreparedTrack {
            plan,
            decoder,
            pre_mix_transforms,
            pre_mix_formats,
            post_mix_transforms,
            post_mix_formats,
            decoded_format,
            mix_format,
            output_format,
            normalizer,
            duration_frames,
            trim_head_frames,
            trim_tail_frames,
            raw_duration_frames: info.duration_frames,
            initial_decoded_frame,
            initial_audible_frame,
        });
    }
    let (stage, message) = last_error.unwrap_or(("decoder", "no decoder candidate".to_owned()));
    Err(PlaybackControlError::failed(stage, message))
}
