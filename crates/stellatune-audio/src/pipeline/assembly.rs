//! Pipeline assembly contracts used by the runtime.
//!
//! This module defines immutable plans and mutable runtime hooks used to build,
//! mutate, and instantiate decode/sink pipeline graphs.

use std::any::Any;
use std::sync::Arc;

use crate::config::engine::{LfeMode, ResampleQuality};
use crate::pipeline::graph::{TransformGraphMutation, TransformGraphStage};
use crate::pipeline::runtime::dsp::control::SharedMasterGainHotControl;
use crate::pipeline::runtime::dsp::gapless_trim::GaplessTrimStage;
use crate::pipeline::runtime::dsp::master_gain::MasterGainStage;
use crate::pipeline::runtime::dsp::mixer::MixerStage;
use crate::pipeline::runtime::dsp::resampler::ResamplerStage;
use crate::pipeline::runtime::dsp::transition_gain::TransitionGainStage;
use crate::pipeline::runtime::runner::PipelineRunner;

use stellatune_audio_core::pipeline::context::InputRef;
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

/// Fully assembled decode-side stages and built-in transform toggles.
pub struct AssembledDecodePipeline {
    /// Source stage implementation.
    pub source: Box<dyn SourceStage>,
    /// Decoder stage implementation.
    pub decoder: Box<dyn DecoderStage>,
    /// Main transform stage list.
    pub transforms: Vec<Box<dyn TransformStage>>,
    /// Pre/post mixer transform chain.
    pub transform_chain: TransformChain,
    /// Optional mixer plan.
    pub mixer: Option<MixerPlan>,
    /// Optional resampler plan.
    pub resampler: Option<ResamplerPlan>,
    /// Built-in transform slot enablement.
    pub builtin_slots: BuiltinTransformSlots,
}

impl AssembledDecodePipeline {
    /// Enables or disables the built-in gapless trim stage.
    pub fn with_gapless_trim(mut self, enabled: bool) -> Self {
        self.builtin_slots.gapless_trim = enabled;
        self
    }

    /// Enables or disables the built-in transition gain stage.
    pub fn with_transition_gain(mut self, enabled: bool) -> Self {
        self.builtin_slots.transition_gain = enabled;
        self
    }

    /// Enables or disables the built-in master gain stage.
    pub fn with_master_gain(mut self, enabled: bool) -> Self {
        self.builtin_slots.master_gain = enabled;
        self
    }

    /// Replaces the mixer plan.
    pub fn with_mixer(mut self, mixer: Option<MixerPlan>) -> Self {
        self.mixer = mixer;
        self
    }

    /// Replaces the resampler plan.
    pub fn with_resampler(mut self, resampler: Option<ResamplerPlan>) -> Self {
        self.resampler = resampler;
        self
    }

    /// Replaces the pre/post transform chain.
    pub fn with_transform_chain(mut self, transform_chain: TransformChain) -> Self {
        self.transform_chain = transform_chain;
        self
    }

    /// Appends a transform to the pre-mix chain.
    pub fn push_pre_mix_transform(&mut self, transform: Box<dyn TransformStage>) {
        self.transform_chain.pre_mix.push(transform);
    }

    /// Appends a transform to the post-mix chain.
    pub fn push_post_mix_transform(&mut self, transform: Box<dyn TransformStage>) {
        self.transform_chain.post_mix.push(transform);
    }
}

/// Enable flags for built-in transform stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinTransformSlots {
    /// Enables gapless head/tail trim.
    pub gapless_trim: bool,
    /// Enables transition gain ramping.
    pub transition_gain: bool,
    /// Enables master gain stage.
    pub master_gain: bool,
}

impl Default for BuiltinTransformSlots {
    fn default() -> Self {
        Self {
            gapless_trim: true,
            transition_gain: true,
            master_gain: true,
        }
    }
}

/// Transform stages inserted around mixer/resampler boundaries.
#[derive(Default)]
pub struct TransformChain {
    /// Transform stages applied before mixer/resampler.
    pub pre_mix: Vec<Box<dyn TransformStage>>,
    /// Transform stages applied after mixer/resampler.
    pub post_mix: Vec<Box<dyn TransformStage>>,
}

/// Mixer configuration for channel-layout adaptation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixerPlan {
    /// Target output channel count.
    pub target_channels: u16,
    /// LFE routing mode.
    pub lfe_mode: LfeMode,
}

impl MixerPlan {
    /// Creates a mixer plan with clamped channel count.
    pub fn new(target_channels: u16, lfe_mode: LfeMode) -> Self {
        Self {
            target_channels: target_channels.max(1),
            lfe_mode,
        }
    }
}

/// Resampler configuration for output sample-rate conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResamplerPlan {
    /// Desired output sample rate.
    pub target_sample_rate: u32,
    /// Quality preset used by the resampler.
    pub quality: ResampleQuality,
}

impl ResamplerPlan {
    /// Creates a resampler plan with clamped sample rate.
    pub fn new(target_sample_rate: u32, quality: ResampleQuality) -> Self {
        Self {
            target_sample_rate: target_sample_rate.max(1),
            quality,
        }
    }
}

/// Sink construction contract used by assembled pipelines.
pub trait SinkPlan: Send {
    /// Returns a stable sink-route identity fingerprint.
    fn route_fingerprint(&self) -> u64;
    /// Consumes the plan and returns sink stage instances.
    fn into_sinks(self: Box<Self>) -> Result<Vec<Box<dyn SinkStage>>, PipelineError>;
}

/// Static sink plan backed by a fixed sink list.
pub struct StaticSinkPlan {
    sinks: Vec<Box<dyn SinkStage>>,
    route_fingerprint: u64,
}

impl StaticSinkPlan {
    /// Creates a static sink plan with default route fingerprint.
    pub fn new(sinks: Vec<Box<dyn SinkStage>>) -> Self {
        Self {
            sinks,
            route_fingerprint: 0,
        }
    }

    /// Creates a static sink plan with explicit route fingerprint.
    pub fn with_route_fingerprint(sinks: Vec<Box<dyn SinkStage>>, route_fingerprint: u64) -> Self {
        Self {
            sinks,
            route_fingerprint,
        }
    }
}

impl SinkPlan for StaticSinkPlan {
    fn route_fingerprint(&self) -> u64 {
        self.route_fingerprint
    }

    fn into_sinks(self: Box<Self>) -> Result<Vec<Box<dyn SinkStage>>, PipelineError> {
        Ok(self.sinks)
    }
}

/// Fully assembled decode and sink plans for runtime instantiation.
pub struct AssembledPipeline {
    /// Decode-side assembly.
    pub decode: AssembledDecodePipeline,
    /// Sink-side assembly.
    pub sink_plan: Box<dyn SinkPlan>,
}

impl AssembledPipeline {
    /// Builds a pipeline from static decode and sink stages.
    pub fn from_static(
        source: Box<dyn SourceStage>,
        decoder: Box<dyn DecoderStage>,
        transforms: Vec<Box<dyn TransformStage>>,
        sinks: Vec<Box<dyn SinkStage>>,
    ) -> Self {
        Self {
            decode: AssembledDecodePipeline {
                source,
                decoder,
                transforms,
                transform_chain: TransformChain::default(),
                mixer: None,
                resampler: None,
                builtin_slots: BuiltinTransformSlots::default(),
            },
            sink_plan: Box::new(StaticSinkPlan::new(sinks)),
        }
    }

    /// Builds a pipeline from pre-assembled decode and sink plans.
    pub fn from_parts(decode: AssembledDecodePipeline, sink_plan: Box<dyn SinkPlan>) -> Self {
        Self { decode, sink_plan }
    }

    pub(crate) fn into_runner(
        self,
        master_gain_hot_control: Option<SharedMasterGainHotControl>,
    ) -> Result<PipelineRunner, PipelineError> {
        let AssembledDecodePipeline {
            source,
            decoder,
            mut transforms,
            mut transform_chain,
            mixer,
            resampler,
            builtin_slots,
        } = self.decode;
        let mut final_transforms: Vec<Box<dyn TransformStage>> = Vec::new();
        if builtin_slots.gapless_trim {
            final_transforms.push(Box::new(GaplessTrimStage::new()));
        }
        final_transforms.append(&mut transform_chain.pre_mix);
        if let Some(plan) = mixer {
            final_transforms.push(Box::new(MixerStage::new(plan)));
        }
        if let Some(plan) = resampler {
            final_transforms.push(Box::new(ResamplerStage::new(plan)));
        }
        final_transforms.append(&mut transforms);
        final_transforms.append(&mut transform_chain.post_mix);
        if builtin_slots.transition_gain {
            final_transforms.push(Box::new(TransitionGainStage::new()));
        }
        if builtin_slots.master_gain {
            let stage = match master_gain_hot_control {
                Some(ref hot_control) => MasterGainStage::with_hot_control(Arc::clone(hot_control)),
                None => MasterGainStage::new(),
            };
            final_transforms.push(Box::new(stage));
        }
        PipelineRunner::new(
            source,
            decoder,
            final_transforms,
            self.sink_plan,
            builtin_slots.transition_gain,
            builtin_slots.gapless_trim,
        )
    }
}

/// Opaque marker trait for pipeline plans accepted by runtime implementations.
pub trait PipelinePlan: Any + Send + Sync {}

impl<T> PipelinePlan for T where T: Any + Send + Sync {}

/// Type-erased payload used by opaque transform stage specifications.
pub type PipelineStagePayload = Arc<dyn Any + Send + Sync>;

/// Serializable-like transform stage spec with opaque typed payload.
#[derive(Clone)]
pub struct OpaqueTransformStageSpec {
    /// Stage key used for routing and mutation targeting.
    pub stage_key: String,
    /// Type-erased stage payload.
    pub payload: PipelineStagePayload,
}

impl OpaqueTransformStageSpec {
    /// Creates a new opaque stage spec.
    pub fn new(stage_key: impl Into<String>, payload: PipelineStagePayload) -> Self {
        Self {
            stage_key: stage_key.into(),
            payload,
        }
    }

    /// Creates a stage spec from a strongly-typed payload.
    pub fn with_payload<T>(stage_key: impl Into<String>, payload: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        Self::new(stage_key, Arc::new(payload))
    }

    /// Returns a typed payload reference if the payload type matches `T`.
    pub fn payload_ref<T: Any>(&self) -> Option<&T> {
        self.payload.as_ref().downcast_ref::<T>()
    }
}

impl std::fmt::Debug for OpaqueTransformStageSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpaqueTransformStageSpec")
            .field("stage_key", &self.stage_key)
            .finish_non_exhaustive()
    }
}

impl TransformGraphStage for OpaqueTransformStageSpec {
    fn stage_key(&self) -> &str {
        &self.stage_key
    }
}

/// Selects one of the built-in transform slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinTransformSlot {
    /// Gapless trim slot.
    GaplessTrim,
    /// Transition gain slot.
    TransitionGain,
    /// Master gain slot.
    MasterGain,
}

/// Runtime mutation operations supported by pipeline runtimes.
#[derive(Debug, Clone)]
pub enum PipelineMutation {
    /// Applies a graph mutation on plugin/opaque transforms.
    MutateTransformGraph {
        /// Graph mutation payload.
        mutation: TransformGraphMutation<OpaqueTransformStageSpec>,
    },
    /// Replaces mixer plan.
    SetMixerPlan {
        /// New mixer plan.
        mixer: Option<MixerPlan>,
    },
    /// Replaces resampler plan.
    SetResamplerPlan {
        /// New resampler plan.
        resampler: Option<ResamplerPlan>,
    },
    /// Enables or disables one built-in transform slot.
    SetBuiltinTransformSlot {
        /// Slot to update.
        slot: BuiltinTransformSlot,
        /// Whether the slot should be enabled.
        enabled: bool,
    },
}

/// Mutable runtime contract for applying plans and mutations.
pub trait PipelineRuntime: Send {
    /// Ensures a concrete assembled pipeline for the provided plan.
    fn ensure(&mut self, plan: &dyn PipelinePlan) -> Result<AssembledPipeline, PipelineError>;
    /// Applies an in-place runtime mutation.
    fn apply_pipeline_mutation(&mut self, mutation: PipelineMutation) -> Result<(), PipelineError>;
    /// Resets runtime-local planning state.
    fn reset(&mut self) {}
}

/// Planner contract used by engine startup and decode worker open flows.
pub trait PipelineAssembler: Send + Sync {
    /// Builds a plan for the specified input reference.
    fn plan(&self, input: &InputRef) -> Result<Arc<dyn PipelinePlan>, PipelineError>;
    /// Creates a fresh runtime instance used to materialize plans.
    fn create_runtime(&self) -> Box<dyn PipelineRuntime>;
}

#[cfg(test)]
#[path = "../tests/pipeline/assembly.rs"]
mod tests;
