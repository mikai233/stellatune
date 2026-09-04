use std::sync::Arc;

use stellatune_audio_core::{
    decoder::DecoderFactory, playback::PlaybackItem, sink::SinkFactory, source::SourceCapabilities,
    transform::TransformFactory,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionPolicy {
    Gapless,
    FadeOutIn {
        fade_out_frames: u64,
        fade_in_frames: u64,
        curve: GainCurve,
    },
    Crossfade {
        duration_frames: u64,
        curve: CrossfadeCurve,
        fallback: CrossfadeFallback,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainCurve {
    Linear,
    EqualPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossfadeCurve {
    Linear,
    EqualPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossfadeFallback {
    Gapless,
    FadeOutIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackPolicies {
    pub transition: TransitionPolicy,
    pub max_decoder_fallbacks: usize,
    pub max_recovery_attempts: usize,
    pub recovery_backoff_ms: u64,
    pub seek_fade_frames: u64,
}

impl Default for PlaybackPolicies {
    fn default() -> Self {
        Self {
            transition: TransitionPolicy::Gapless,
            max_decoder_fallbacks: 3,
            max_recovery_attempts: 2,
            recovery_backoff_ms: 50,
            seek_fade_frames: 240,
        }
    }
}

#[derive(Clone)]
pub struct StageRegistrySnapshot {
    pub decoders: Vec<Arc<dyn DecoderFactory>>,
    pub transforms: Vec<Arc<dyn TransformFactory>>,
    pub sink: Arc<dyn SinkFactory>,
}

#[derive(Clone)]
pub struct PlaybackRequest {
    pub item: PlaybackItem,
    pub policies: PlaybackPolicies,
}

#[derive(Clone)]
pub struct ExecutablePlaybackPlan {
    pub item: PlaybackItem,
    pub decoder_candidates: Vec<Arc<dyn DecoderFactory>>,
    pub transforms: Vec<Arc<dyn TransformFactory>>,
    pub sink: Arc<dyn SinkFactory>,
    pub policies: PlaybackPolicies,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlannerError {
    #[error("no decoder supports the source media hints")]
    NoDecoder,
    #[error("required decoder `{decoder}` is incompatible with the source")]
    RequiredDecoderIncompatible { decoder: String },
}

#[derive(Debug, Default)]
pub struct PipelinePlanner;

impl PipelinePlanner {
    pub fn plan(
        &self,
        request: PlaybackRequest,
        registry: &StageRegistrySnapshot,
    ) -> Result<ExecutablePlaybackPlan, PlannerError> {
        let descriptor = request.item.source.descriptor();
        let decoder_candidates = if let Some(required) = &request.item.required_decoder {
            if !decoder_matches(required.as_ref(), &descriptor.media) {
                return Err(PlannerError::RequiredDecoderIncompatible {
                    decoder: required.descriptor().id.as_str().to_owned(),
                });
            }
            vec![Arc::clone(required)]
        } else {
            let mut candidates = registry
                .decoders
                .iter()
                .filter(|factory| decoder_matches(factory.as_ref(), &descriptor.media))
                .cloned()
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| {
                right
                    .descriptor()
                    .priority
                    .cmp(&left.descriptor().priority)
                    .then_with(|| {
                        left.descriptor()
                            .id
                            .as_str()
                            .cmp(right.descriptor().id.as_str())
                    })
            });
            candidates
        };
        if decoder_candidates.is_empty() {
            return Err(PlannerError::NoDecoder);
        }

        let mut transforms = registry.transforms.clone();
        transforms.sort_by(|left, right| {
            left.descriptor()
                .placement
                .cmp(&right.descriptor().placement)
                .then_with(|| {
                    left.descriptor()
                        .id
                        .as_str()
                        .cmp(right.descriptor().id.as_str())
                })
        });

        Ok(ExecutablePlaybackPlan {
            item: request.item,
            decoder_candidates,
            transforms,
            sink: Arc::clone(&registry.sink),
            policies: request.policies,
        })
    }
}

pub(crate) fn can_fallback(capabilities: SourceCapabilities, candidate_index: usize) -> bool {
    candidate_index == 0 || capabilities.reopenable
}

fn decoder_matches(
    factory: &dyn DecoderFactory,
    media: &stellatune_audio_core::source::MediaHints,
) -> bool {
    let descriptor = factory.descriptor();
    let extension_matches = media.extension.as_ref().is_some_and(|extension| {
        descriptor
            .extensions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension.trim_start_matches('.')))
    });
    let mime_matches = media.mime_type.as_ref().is_some_and(|mime| {
        descriptor
            .mime_types
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(mime))
    });
    extension_matches || mime_matches || (media.extension.is_none() && media.mime_type.is_none())
}
