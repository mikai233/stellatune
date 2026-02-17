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

pub struct AssembledDecodePipeline {
    pub source: Box<dyn SourceStage>,
    pub decoder: Box<dyn DecoderStage>,
    pub transforms: Vec<Box<dyn TransformStage>>,
    pub transform_chain: TransformChain,
    pub mixer: Option<MixerPlan>,
    pub resampler: Option<ResamplerPlan>,
    pub builtin_slots: BuiltinTransformSlots,
}

impl AssembledDecodePipeline {
    pub fn with_gapless_trim(mut self, enabled: bool) -> Self {
        self.builtin_slots.gapless_trim = enabled;
        self
    }

    pub fn with_transition_gain(mut self, enabled: bool) -> Self {
        self.builtin_slots.transition_gain = enabled;
        self
    }

    pub fn with_master_gain(mut self, enabled: bool) -> Self {
        self.builtin_slots.master_gain = enabled;
        self
    }

    pub fn with_mixer(mut self, mixer: Option<MixerPlan>) -> Self {
        self.mixer = mixer;
        self
    }

    pub fn with_resampler(mut self, resampler: Option<ResamplerPlan>) -> Self {
        self.resampler = resampler;
        self
    }

    pub fn with_transform_chain(mut self, transform_chain: TransformChain) -> Self {
        self.transform_chain = transform_chain;
        self
    }

    pub fn push_pre_mix_transform(&mut self, transform: Box<dyn TransformStage>) {
        self.transform_chain.pre_mix.push(transform);
    }

    pub fn push_post_mix_transform(&mut self, transform: Box<dyn TransformStage>) {
        self.transform_chain.post_mix.push(transform);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinTransformSlots {
    pub gapless_trim: bool,
    pub transition_gain: bool,
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

pub struct TransformChain {
    pub pre_mix: Vec<Box<dyn TransformStage>>,
    pub post_mix: Vec<Box<dyn TransformStage>>,
}

impl Default for TransformChain {
    fn default() -> Self {
        Self {
            pre_mix: Vec::new(),
            post_mix: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixerPlan {
    pub target_channels: u16,
    pub lfe_mode: LfeMode,
}

impl MixerPlan {
    pub fn new(target_channels: u16, lfe_mode: LfeMode) -> Self {
        Self {
            target_channels: target_channels.max(1),
            lfe_mode,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResamplerPlan {
    pub target_sample_rate: u32,
    pub quality: ResampleQuality,
}

impl ResamplerPlan {
    pub fn new(target_sample_rate: u32, quality: ResampleQuality) -> Self {
        Self {
            target_sample_rate: target_sample_rate.max(1),
            quality,
        }
    }
}

pub trait SinkPlan: Send {
    fn route_fingerprint(&self) -> u64;
    fn into_sinks(self: Box<Self>) -> Result<Vec<Box<dyn SinkStage>>, PipelineError>;
}

pub struct StaticSinkPlan {
    sinks: Vec<Box<dyn SinkStage>>,
    route_fingerprint: u64,
}

impl StaticSinkPlan {
    pub fn new(sinks: Vec<Box<dyn SinkStage>>) -> Self {
        Self {
            sinks,
            route_fingerprint: 0,
        }
    }

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

pub struct AssembledPipeline {
    pub decode: AssembledDecodePipeline,
    pub sink_plan: Box<dyn SinkPlan>,
}

impl AssembledPipeline {
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

pub trait PipelinePlan: Any + Send + Sync {}

impl<T> PipelinePlan for T where T: Any + Send + Sync {}

pub type PipelineStagePayload = Arc<dyn Any + Send + Sync>;

#[derive(Clone)]
pub struct OpaqueTransformStageSpec {
    pub stage_key: String,
    pub payload: PipelineStagePayload,
}

impl OpaqueTransformStageSpec {
    pub fn new(stage_key: impl Into<String>, payload: PipelineStagePayload) -> Self {
        Self {
            stage_key: stage_key.into(),
            payload,
        }
    }

    pub fn with_payload<T>(stage_key: impl Into<String>, payload: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        Self::new(stage_key, Arc::new(payload))
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinTransformSlot {
    GaplessTrim,
    TransitionGain,
    MasterGain,
}

#[derive(Debug, Clone)]
pub enum PipelineMutation {
    MutateTransformGraph {
        mutation: TransformGraphMutation<OpaqueTransformStageSpec>,
    },
    SetMixerPlan {
        mixer: Option<MixerPlan>,
    },
    SetResamplerPlan {
        resampler: Option<ResamplerPlan>,
    },
    SetBuiltinTransformSlot {
        slot: BuiltinTransformSlot,
        enabled: bool,
    },
}

pub trait PipelineRuntime: Send {
    fn ensure(&mut self, plan: &dyn PipelinePlan) -> Result<AssembledPipeline, PipelineError>;
    fn apply_pipeline_mutation(&mut self, mutation: PipelineMutation) -> Result<(), PipelineError>;
    fn reset(&mut self) {}
}

pub trait PipelineAssembler: Send + Sync {
    fn plan(&self, input: &InputRef) -> Result<Arc<dyn PipelinePlan>, PipelineError>;
    fn create_runtime(&self) -> Box<dyn PipelineRuntime>;
}

#[cfg(test)]
#[path = "../tests/pipeline/assembly.rs"]
mod tests;
