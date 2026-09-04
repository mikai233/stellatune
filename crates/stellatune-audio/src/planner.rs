//! Deterministic selection and ordering of playback pipeline stages.
//!
//! [`PipelinePlanner`](crate::planner::PipelinePlanner) operates only on typed
//! factories. It does not open sources or create stages. Decoder candidates are
//! ordered by descending priority and then stable stage identifier; transforms
//! are ordered by placement and identifier.

use std::sync::Arc;

use stellatune_audio_core::{
    decoder::DecoderFactory, playback::PlaybackItem, sink::SinkFactory, source::SourceCapabilities,
    transform::TransformFactory,
};
use thiserror::Error;

/// The transition applied when moving from the current item to the next item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionPolicy {
    /// Promotes the prepared next item without applying a gain envelope.
    Gapless,
    /// Fades the current item out before fading the next item in.
    FadeOutIn {
        /// Duration of the current item's fade, in output PCM frames.
        fade_out_frames: u64,
        /// Duration of the next item's fade, in output PCM frames.
        fade_in_frames: u64,
        /// Gain curve used by both fades.
        curve: GainCurve,
    },
    /// Overlaps the end of the current item with the beginning of the next.
    Crossfade {
        /// Desired overlap duration, in output PCM frames.
        duration_frames: u64,
        /// Pair of gain curves applied during the overlap.
        curve: CrossfadeCurve,
        /// Transition used when the two pipelines cannot overlap safely.
        fallback: CrossfadeFallback,
    },
}

/// A one-sided gain-envelope curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainCurve {
    /// Changes gain at a constant rate.
    Linear,
    /// Uses a sine-shaped curve to preserve perceived loudness.
    EqualPower,
}

/// Complementary gain curves used while two tracks overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossfadeCurve {
    /// Fades one track down and the other up at constant rates.
    Linear,
    /// Uses complementary trigonometric gains to preserve perceived power.
    EqualPower,
}

/// The policy used when a requested crossfade cannot be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossfadeFallback {
    /// Promotes the next compatible pipeline without an overlap envelope.
    Gapless,
    /// Falls back to non-overlapping linear fade-out and fade-in envelopes.
    FadeOutIn,
}

/// Runtime policies applied to preparation, transitions, seeking, and recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackPolicies {
    /// The default transition between adjacent items.
    pub transition: TransitionPolicy,
    /// The maximum number of ordered decoder candidates tried during preparation.
    ///
    /// At least one candidate is attempted even when this value is zero.
    pub max_decoder_fallbacks: usize,
    /// The maximum number of recovery preparations for one failure sequence.
    pub max_recovery_attempts: usize,
    /// Base delay in milliseconds multiplied by the recovery attempt index.
    pub recovery_backoff_ms: u64,
    /// Duration of the de-click envelope after a seek, in output PCM frames.
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

/// An immutable set of stage factories available to one runtime.
///
/// Cloning a snapshot clones the factory [`Arc`] values, not the stages they
/// later create.
#[derive(Clone)]
pub struct StageRegistrySnapshot {
    /// Decoder factories considered by [`PipelinePlanner`].
    pub decoders: Vec<Arc<dyn DecoderFactory>>,
    /// Transform factories sorted into pre-mix and post-mix chains.
    pub transforms: Vec<Arc<dyn TransformFactory>>,
    /// The output factory used to negotiate and create the sink.
    pub sink: Arc<dyn SinkFactory>,
}

/// Input to [`PipelinePlanner::plan`].
#[derive(Clone)]
pub struct PlaybackRequest {
    /// The already-materialized item to prepare.
    pub item: PlaybackItem,
    /// The policies captured by the resulting plan.
    pub policies: PlaybackPolicies,
}

/// A deterministic factory plan ready for off-turn preparation.
///
/// The plan contains factories rather than opened sources or live stages, so it
/// can be moved to a blocking preparation task and cloned for recovery.
#[derive(Clone)]
pub struct ExecutablePlaybackPlan {
    /// The item whose source will be opened.
    pub item: PlaybackItem,
    /// Compatible decoder factories in attempted order.
    pub decoder_candidates: Vec<Arc<dyn DecoderFactory>>,
    /// Transform factories in placement and identifier order.
    pub transforms: Vec<Arc<dyn TransformFactory>>,
    /// The selected output factory.
    pub sink: Arc<dyn SinkFactory>,
    /// The policies captured when this plan was created.
    pub policies: PlaybackPolicies,
}

/// An error encountered before source or stage construction begins.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlannerError {
    /// No registered decoder matches the source's extension or media type.
    #[error("no decoder supports the source media hints")]
    NoDecoder,
    /// An explicitly required decoder does not match the source hints.
    #[error("required decoder `{decoder}` is incompatible with the source")]
    RequiredDecoderIncompatible {
        /// The stable identifier of the required decoder.
        decoder: String,
    },
}

/// Builds executable playback plans from items and a stage registry snapshot.
#[derive(Debug, Default)]
pub struct PipelinePlanner;

impl PipelinePlanner {
    /// Selects and deterministically orders the factories for `request`.
    ///
    /// When the item specifies a required decoder, that decoder is the only
    /// candidate. Otherwise matching registry decoders are sorted by descending
    /// priority and stable identifier. Transforms are sorted by placement and
    /// identifier. No source is opened and no stage is created by this method.
    ///
    /// # Errors
    ///
    /// Returns [`PlannerError::NoDecoder`] when no registry decoder matches, or
    /// [`PlannerError::RequiredDecoderIncompatible`] when the item's required
    /// decoder conflicts with its media hints.
    ///
    /// # Examples
    ///
    /// ```
    /// use stellatune_audio::planner::{
    ///     PipelinePlanner, PlaybackRequest, StageRegistrySnapshot,
    /// };
    /// use stellatune_audio_core::playback::PlaybackItem;
    ///
    /// fn make_plan(
    ///     item: PlaybackItem,
    ///     registry: &StageRegistrySnapshot,
    /// ) -> Result<(), stellatune_audio::planner::PlannerError> {
    ///     let request = PlaybackRequest {
    ///         item,
    ///         policies: Default::default(),
    ///     };
    ///     let _plan = PipelinePlanner.plan(request, registry)?;
    ///     Ok(())
    /// }
    /// ```
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
