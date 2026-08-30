//! Strongly typed playback intent and executable pipeline plans.
//!
//! These types form the boundary between source negotiation and native stage
//! construction. A source may describe where encoded media is located and what
//! it requires, but only the core planner can choose DSP and output stages.

use std::collections::BTreeMap;

use serde_json::Value;
use stellatune_audio_core::pipeline::context::InputRef;

use crate::config::engine::{LfeMode, ResampleQuality};
use crate::pipeline::assembly::{
    AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlots, TransformChain,
};
use crate::pipeline::capability::{
    CapabilityKind, CapabilityRegistry, ExecutionBackend, RegistryError,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use thiserror::Error;

/// Stable identifier of a registered source, decoder, transform, or output.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageId(String);

impl StageId {
    /// Creates an identifier after trimming surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns an error when the identifier is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err("stage id cannot be empty");
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated configuration passed to a selected stage factory.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StageConfig(Value);

impl StageConfig {
    /// Wraps a JSON value after the owning registry has validated its schema.
    pub fn validated(value: Value) -> Self {
        Self(value)
    }

    /// Borrows the validated configuration value.
    pub fn value(&self) -> &Value {
        &self.0
    }
}

/// User/API playback intent before source negotiation and capability selection.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackRequest {
    /// Input selected by the caller.
    pub input: InputRef,
    /// Ordered DSP choices requested by the caller.
    pub transforms: Vec<TransformSelection>,
    /// Output route requested by the caller.
    pub output: OutputSelection,
    /// Core playback policies.
    pub policies: PlaybackPolicies,
    /// Optional native channel-layout adaptation selected by the core.
    pub mixer: Option<crate::pipeline::assembly::MixerPlan>,
    /// Optional native sample-rate conversion selected by the core.
    pub resampler: Option<crate::pipeline::assembly::ResamplerPlan>,
}

/// Locator returned by a source resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceLocator {
    /// Local file path consumed directly by the Rust source stage.
    File { path: String },
    /// HTTP resource consumed directly by the Rust HTTP source stage.
    Http {
        /// Resource URL.
        url: String,
        /// Stable request headers supplied by negotiation.
        headers: BTreeMap<String, String>,
    },
}

/// Media hints that help the core select and probe a decoder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaHints {
    /// Normalized extension without a leading dot.
    pub extension: Option<String>,
    /// MIME type reported by the source.
    pub mime_type: Option<String>,
    /// Optional encoded byte length.
    pub content_length: Option<u64>,
}

/// Source behavior available to playback and recovery policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceCapabilities {
    /// Whether the source can seek to an absolute media position.
    pub seekable: bool,
    /// Whether the locator represents a live stream.
    pub live: bool,
}

/// Constraints supplied by source negotiation without selecting an output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceRequirements {
    /// Optional required decoder capability identifier.
    pub decoder: Option<StageId>,
}

/// Declarative result returned by a builtin or TypeScript source resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePlan {
    /// Encoded media location; bytes never travel through TypeScript RPC.
    pub locator: SourceLocator,
    /// Decoder selection hints.
    pub media: MediaHints,
    /// Seek/live behavior.
    pub capabilities: SourceCapabilities,
    /// Hard source constraints enforced by the core planner.
    pub requirements: SourceRequirements,
}

/// Selected source stage and its validated configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceSelection {
    /// Registered source stage identifier.
    pub stage_id: StageId,
    /// Stage-specific validated configuration.
    pub config: StageConfig,
}

/// Selected decoder stage and its validated configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct DecoderSelection {
    /// Registered decoder stage identifier.
    pub stage_id: StageId,
    /// Stage-specific validated configuration.
    pub config: StageConfig,
}

/// Selected transform stage and its validated configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformSelection {
    /// Registered transform stage identifier.
    pub stage_id: StageId,
    /// Stage-specific validated configuration.
    pub config: StageConfig,
    /// Stable insertion point around native mixer/resampler stages.
    pub placement: TransformPlacement,
}

/// Strongly typed transform position without a mutable runtime graph.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransformPlacement {
    /// Runs before native channel mixing and resampling.
    PreMix,
    /// Runs after native mixing/resampling in the primary DSP chain.
    #[default]
    Main,
    /// Runs after the primary DSP chain and before transition/master gain.
    PostMix,
}

/// Output route selected by the user or host policy.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputSelection {
    /// Registered output stage identifier.
    pub stage_id: StageId,
    /// Stage-specific validated configuration.
    pub config: StageConfig,
}

/// Playback policies owned by the Rust core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackPolicies {
    /// Whether native gapless trimming is enabled.
    pub gapless: bool,
    /// Whether native transition gain ramps are enabled.
    pub transition_gain: bool,
    /// Whether native master gain is enabled.
    pub master_gain: bool,
    /// Mixer low-frequency routing policy.
    pub lfe_mode: LfeMode,
    /// Resampler quality policy.
    pub resample_quality: ResampleQuality,
}

impl Default for PlaybackPolicies {
    fn default() -> Self {
        Self {
            gapless: true,
            transition_gain: true,
            master_gain: true,
            lfe_mode: LfeMode::default(),
            resample_quality: ResampleQuality::default(),
        }
    }
}

/// Immutable, core-authored plan used to build native playback stages.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutablePlaybackPlan {
    /// Selected source stage.
    pub source: SourceSelection,
    /// Selected decoder stage.
    pub decoder: DecoderSelection,
    /// Ordered selected transforms.
    pub transforms: Vec<TransformSelection>,
    /// Selected output stage.
    pub output: OutputSelection,
    /// Policies applied while building and driving the session.
    pub policies: PlaybackPolicies,
    /// Native channel-layout adaptation.
    pub mixer: Option<crate::pipeline::assembly::MixerPlan>,
    /// Native sample-rate conversion.
    pub resampler: Option<crate::pipeline::assembly::ResamplerPlan>,
}

/// State to restore after a complete playback-session rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackCheckpoint {
    /// Track identity that was active at suspension.
    pub input: InputRef,
    /// Position actually consumed by the sink, in milliseconds.
    pub consumed_position_ms: i64,
    /// Whether playback should resume automatically after rebuilding.
    pub resume_playing: bool,
}

/// Deterministic planning failures before any stage is constructed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanningError {
    /// Registry lookup or validation failed.
    #[error(transparent)]
    Registry(#[from] RegistryError),
    /// No decoder matches the negotiated media hints.
    #[error("no decoder matches extension {extension:?}")]
    NoDecoder {
        /// Normalized extension used for selection.
        extension: Option<String>,
    },
    /// A capability exists but cannot fill the requested slot.
    #[error("capability '{id}' cannot fill {expected:?}")]
    InvalidSelection {
        /// Invalid capability identifier.
        id: String,
        /// Required capability slot.
        expected: CapabilityKind,
    },
}

/// Converts playback intent and source negotiation into an immutable executable plan.
pub struct PipelinePlanner {
    file_source: StageId,
    http_source: StageId,
}

impl PipelinePlanner {
    /// Creates a planner with the registered builtin file and HTTP source IDs.
    pub fn new(file_source: StageId, http_source: StageId) -> Self {
        Self {
            file_source,
            http_source,
        }
    }

    /// Selects and validates all PCM data-plane capabilities.
    pub fn plan(
        &self,
        request: PlaybackRequest,
        source_plan: SourcePlan,
        registry: &CapabilityRegistry,
    ) -> Result<ExecutablePlaybackPlan, PlanningError> {
        let (source_id, source_config) = match &source_plan.locator {
            SourceLocator::File { path } => (
                self.file_source.clone(),
                StageConfig::validated(serde_json::json!({ "path": path })),
            ),
            SourceLocator::Http { url, headers } => (
                self.http_source.clone(),
                StageConfig::validated(serde_json::json!({ "url": url, "headers": headers })),
            ),
        };
        validate_kind(registry, &source_id, CapabilityKind::Source)?;

        let decoder_id = match source_plan.requirements.decoder {
            Some(id) => {
                validate_kind(registry, &id, CapabilityKind::Decoder)?;
                id
            },
            None => registry
                .decoder_candidates(source_plan.media.extension.as_deref())
                .first()
                .map(|descriptor| descriptor.id.clone())
                .ok_or_else(|| PlanningError::NoDecoder {
                    extension: source_plan.media.extension.clone(),
                })?,
        };

        for transform in &request.transforms {
            validate_kind(registry, &transform.stage_id, CapabilityKind::Transform)?;
        }
        validate_kind(registry, &request.output.stage_id, CapabilityKind::Sink)?;

        Ok(ExecutablePlaybackPlan {
            source: SourceSelection {
                stage_id: source_id,
                config: source_config,
            },
            decoder: DecoderSelection {
                stage_id: decoder_id,
                config: StageConfig::default(),
            },
            transforms: request.transforms,
            output: request.output,
            policies: request.policies,
            mixer: request.mixer,
            resampler: request.resampler,
        })
    }
}

/// Constructs one native pipeline exclusively from an executable plan and registry factories.
pub struct PipelineBuilder;

impl PipelineBuilder {
    /// Builds native stages; no plugin/runtime-specific payload is downcast.
    pub fn build(
        plan: &ExecutablePlaybackPlan,
        registry: &CapabilityRegistry,
    ) -> Result<AssembledPipeline, PipelineError> {
        let source = registry
            .source_factory(&plan.source.stage_id)
            .map_err(registry_pipeline_error)?
            .create(&plan.source.config)?;
        let decoder = registry
            .decoder_factory(&plan.decoder.stage_id)
            .map_err(registry_pipeline_error)?
            .create(&plan.decoder.config)?;
        let mut transforms = Vec::new();
        let mut transform_chain = TransformChain::default();
        for selection in &plan.transforms {
            let stage = registry
                .transform_factory(&selection.stage_id)
                .map_err(registry_pipeline_error)?
                .create(&selection.config)?;
            match selection.placement {
                TransformPlacement::PreMix => transform_chain.pre_mix.push(stage),
                TransformPlacement::Main => transforms.push(stage),
                TransformPlacement::PostMix => transform_chain.post_mix.push(stage),
            }
        }
        let sink_plan = registry
            .output_factory(&plan.output.stage_id)
            .map_err(registry_pipeline_error)?
            .create(&plan.output.config)?;
        Ok(AssembledPipeline::from_parts(
            AssembledDecodePipeline {
                source,
                decoder,
                transforms,
                transform_chain,
                mixer: plan.mixer,
                resampler: plan.resampler,
                builtin_slots: BuiltinTransformSlots {
                    gapless_trim: plan.policies.gapless,
                    transition_gain: plan.policies.transition_gain,
                    master_gain: plan.policies.master_gain,
                },
            },
            sink_plan,
        ))
    }
}

fn validate_kind(
    registry: &CapabilityRegistry,
    id: &StageId,
    expected: CapabilityKind,
) -> Result<(), PlanningError> {
    let descriptor = registry
        .descriptor(id)
        .ok_or_else(|| RegistryError::Missing {
            kind: expected,
            id: id.as_str().to_string(),
        })?;
    if descriptor.kind != expected
        || (descriptor.backend == ExecutionBackend::TypeScriptProcess
            && matches!(
                expected,
                CapabilityKind::Source
                    | CapabilityKind::Decoder
                    | CapabilityKind::Transform
                    | CapabilityKind::Sink
            ))
    {
        return Err(PlanningError::InvalidSelection {
            id: id.as_str().to_string(),
            expected,
        });
    }
    Ok(())
}

fn registry_pipeline_error(error: RegistryError) -> PipelineError {
    PipelineError::StageFailure(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stellatune_audio_core::pipeline::{
        error::PipelineError,
        stages::{decoder::DecoderStage, source::SourceStage},
    };

    use super::{
        OutputSelection, PipelinePlanner, PlaybackPolicies, PlaybackRequest, SourceCapabilities,
        SourceLocator, SourcePlan, SourceRequirements, StageConfig, StageId,
    };
    use crate::pipeline::assembly::SinkPlan;
    use crate::pipeline::capability::{
        CapabilityDescriptor, CapabilityKind, CapabilityRegistry, DecoderFactory, ExecutionBackend,
        OutputFactory, SourceFactory,
    };

    fn unused_source_factory(_: &StageConfig) -> Result<Box<dyn SourceStage>, PipelineError> {
        Err(PipelineError::StageFailure(
            "factory must not run in this test".to_string(),
        ))
    }

    fn unused_decoder_factory(_: &StageConfig) -> Result<Box<dyn DecoderStage>, PipelineError> {
        Err(PipelineError::StageFailure(
            "factory must not run in this test".to_string(),
        ))
    }

    fn unused_output_factory(_: &StageConfig) -> Result<Box<dyn SinkPlan>, PipelineError> {
        Err(PipelineError::StageFailure(
            "factory must not run in this test".to_string(),
        ))
    }

    #[test]
    fn stage_id_rejects_empty_values_and_normalizes_whitespace() {
        assert!(StageId::new("  ").is_err());
        assert_eq!(
            StageId::new(" builtin.decoder ").unwrap().as_str(),
            "builtin.decoder"
        );
    }

    #[test]
    fn source_plan_keeps_media_transport_outside_plugin_runtime() {
        let plan = SourcePlan {
            locator: SourceLocator::Http {
                url: "https://example.test/audio.flac".to_string(),
                headers: [("authorization".to_string(), "token".to_string())]
                    .into_iter()
                    .collect(),
            },
            media: Default::default(),
            capabilities: Default::default(),
            requirements: Default::default(),
        };
        assert!(matches!(plan.locator, SourceLocator::Http { .. }));
    }

    #[test]
    fn planner_selects_highest_priority_decoder_and_preserves_user_output() {
        let file_id = StageId::new("builtin.file").unwrap();
        let http_id = StageId::new("builtin.http").unwrap();
        let output_id = StageId::new("builtin.output").unwrap();
        let mut registry = CapabilityRegistry::new();
        registry
            .register_source(
                CapabilityDescriptor::new(
                    file_id.clone(),
                    CapabilityKind::Source,
                    ExecutionBackend::BuiltinRust,
                ),
                Arc::new(unused_source_factory) as Arc<dyn SourceFactory>,
            )
            .unwrap();
        for (id, priority) in [("decoder.low", 1), ("decoder.high", 9)] {
            registry
                .register_decoder(
                    CapabilityDescriptor::new(
                        StageId::new(id).unwrap(),
                        CapabilityKind::Decoder,
                        ExecutionBackend::BuiltinRust,
                    )
                    .with_priority(priority)
                    .with_extensions(["flac".to_string()]),
                    Arc::new(unused_decoder_factory) as Arc<dyn DecoderFactory>,
                )
                .unwrap();
        }
        registry
            .register_output(
                CapabilityDescriptor::new(
                    output_id.clone(),
                    CapabilityKind::Sink,
                    ExecutionBackend::BuiltinRust,
                ),
                Arc::new(unused_output_factory) as Arc<dyn OutputFactory>,
            )
            .unwrap();
        let plan = PipelinePlanner::new(file_id, http_id)
            .plan(
                PlaybackRequest {
                    input: stellatune_audio_core::pipeline::context::InputRef::TrackToken(
                        "track".to_string(),
                    ),
                    transforms: Vec::new(),
                    output: OutputSelection {
                        stage_id: output_id.clone(),
                        config: StageConfig::default(),
                    },
                    policies: PlaybackPolicies::default(),
                    mixer: None,
                    resampler: None,
                },
                SourcePlan {
                    locator: SourceLocator::File {
                        path: "track.flac".to_string(),
                    },
                    media: super::MediaHints {
                        extension: Some("flac".to_string()),
                        ..Default::default()
                    },
                    capabilities: SourceCapabilities::default(),
                    requirements: SourceRequirements::default(),
                },
                &registry,
            )
            .unwrap();
        assert_eq!(plan.decoder.stage_id.as_str(), "decoder.high");
        assert_eq!(plan.output.stage_id, output_id);
    }
}
