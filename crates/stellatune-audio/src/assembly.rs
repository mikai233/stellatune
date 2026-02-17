use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use crate::pipeline_graph::{TransformGraphMutation, TransformGraphStage};
use crate::runtime::runner::PipelineRunner;
use crate::runtime::transform::control::SharedMasterGainHotControl;
use crate::runtime::transform::gapless_trim::GaplessTrimStage;
use crate::runtime::transform::master_gain::MasterGainStage;
use crate::runtime::transform::mixer::MixerStage;
use crate::runtime::transform::resampler::ResamplerStage;
use crate::runtime::transform::transition_gain::TransitionGainStage;
use crate::types::{LfeMode, ResampleQuality, SinkLatencyConfig};

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
        sink_latency: SinkLatencyConfig,
        sink_control_timeout: Duration,
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
            sink_latency,
            sink_control_timeout,
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
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use stellatune_audio_core::pipeline::context::{
        AudioBlock, InputRef, PipelineContext, SourceHandle, StreamSpec,
    };
    use stellatune_audio_core::pipeline::error::PipelineError;
    use stellatune_audio_core::pipeline::stages::StageStatus;
    use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
    use stellatune_audio_core::pipeline::stages::sink::SinkStage;
    use stellatune_audio_core::pipeline::stages::source::SourceStage;
    use stellatune_audio_core::pipeline::stages::transform::TransformStage;

    use crate::runtime::runner::{RunnerState, StepResult};
    use crate::runtime::sink_session::{SinkActivationMode, SinkSession};
    use crate::types::{LfeMode, ResampleQuality, SinkLatencyConfig, StopBehavior};

    use super::{
        AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlots, MixerPlan,
        ResamplerPlan, StaticSinkPlan, TransformChain,
    };

    #[derive(Default)]
    struct TestSource;

    impl SourceStage for TestSource {
        fn prepare(
            &mut self,
            _input: &InputRef,
            _ctx: &mut PipelineContext,
        ) -> Result<SourceHandle, PipelineError> {
            Ok(SourceHandle::new(()))
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    struct TestDecoder {
        blocks: VecDeque<Vec<f32>>,
        channels: u16,
        sample_rate: u32,
    }

    impl TestDecoder {
        fn new(blocks: Vec<Vec<f32>>, channels: u16, sample_rate: u32) -> Self {
            Self {
                blocks: blocks.into(),
                channels: channels.max(1),
                sample_rate: sample_rate.max(1),
            }
        }
    }

    impl DecoderStage for TestDecoder {
        fn prepare(
            &mut self,
            _source: &SourceHandle,
            _ctx: &mut PipelineContext,
        ) -> Result<StreamSpec, PipelineError> {
            Ok(StreamSpec {
                sample_rate: self.sample_rate,
                channels: self.channels,
            })
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn next_block(&mut self, out: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
            let Some(samples) = self.blocks.pop_front() else {
                return StageStatus::Eof;
            };
            out.channels = self.channels;
            out.samples = samples;
            StageStatus::Ok
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {
            self.blocks.clear();
        }
    }

    #[derive(Default)]
    struct TestSink;

    impl SinkStage for TestSink {
        fn prepare(
            &mut self,
            _spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn write(&mut self, _block: &AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
            StageStatus::Ok
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    struct CaptureSink {
        written: Arc<Mutex<Vec<AudioBlock>>>,
    }

    impl CaptureSink {
        fn new(written: Arc<Mutex<Vec<AudioBlock>>>) -> Self {
            Self { written }
        }
    }

    impl SinkStage for CaptureSink {
        fn prepare(
            &mut self,
            _spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn write(&mut self, block: &AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
            self.written
                .lock()
                .expect("capture sink mutex poisoned")
                .push(block.clone());
            StageStatus::Ok
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    struct SpecTap {
        seen: Arc<Mutex<Vec<StreamSpec>>>,
    }

    impl SpecTap {
        fn new(seen: Arc<Mutex<Vec<StreamSpec>>>) -> Self {
            Self { seen }
        }
    }

    impl TransformStage for SpecTap {
        fn prepare(
            &mut self,
            spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<StreamSpec, PipelineError> {
            self.seen
                .lock()
                .expect("spec tap mutex poisoned")
                .push(spec);
            Ok(spec)
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn process(&mut self, _block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
            StageStatus::Ok
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    struct FlushTailTap {
        channels: u16,
        pending_samples: Vec<f32>,
        emit_on_empty_process: bool,
    }

    impl FlushTailTap {
        fn new(pending_samples: Vec<f32>) -> Self {
            Self {
                channels: 1,
                pending_samples,
                emit_on_empty_process: false,
            }
        }
    }

    impl TransformStage for FlushTailTap {
        fn prepare(
            &mut self,
            spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<StreamSpec, PipelineError> {
            self.channels = spec.channels.max(1);
            Ok(spec)
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn process(&mut self, block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
            if block.is_empty() && self.emit_on_empty_process && !self.pending_samples.is_empty() {
                block.channels = self.channels;
                block.samples = std::mem::take(&mut self.pending_samples);
            }
            StageStatus::Ok
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            self.emit_on_empty_process = true;
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {
            self.emit_on_empty_process = false;
            self.pending_samples.clear();
        }
    }

    struct KeyedNoopTransform {
        key: &'static str,
    }

    impl KeyedNoopTransform {
        fn new(key: &'static str) -> Self {
            Self { key }
        }
    }

    impl TransformStage for KeyedNoopTransform {
        fn stage_key(&self) -> Option<&str> {
            Some(self.key)
        }

        fn prepare(
            &mut self,
            spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<StreamSpec, PipelineError> {
            Ok(spec)
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn process(&mut self, _block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
            StageStatus::Ok
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    #[test]
    fn pre_post_transform_chain_wraps_mixer_and_resampler() {
        let pre_seen = Arc::new(Mutex::new(Vec::new()));
        let post_seen = Arc::new(Mutex::new(Vec::new()));

        let assembled = AssembledPipeline::from_parts(
            AssembledDecodePipeline {
                source: Box::new(TestSource),
                decoder: Box::new(TestDecoder::new(vec![vec![0.0, 1.0, 2.0, 3.0]], 1, 48_000)),
                transforms: Vec::new(),
                transform_chain: TransformChain {
                    pre_mix: vec![Box::new(SpecTap::new(Arc::clone(&pre_seen)))],
                    post_mix: vec![Box::new(SpecTap::new(Arc::clone(&post_seen)))],
                },
                mixer: Some(MixerPlan::new(2, LfeMode::Mute)),
                resampler: Some(ResamplerPlan::new(24_000, ResampleQuality::Balanced)),
                builtin_slots: BuiltinTransformSlots {
                    gapless_trim: false,
                    ..BuiltinTransformSlots::default()
                },
            },
            Box::new(StaticSinkPlan::new(vec![Box::new(TestSink)])),
        );

        let mut runner = assembled
            .into_runner(
                SinkLatencyConfig::default(),
                Duration::from_millis(100),
                None,
            )
            .expect("into_runner failed");
        let mut sink_session =
            SinkSession::new(SinkLatencyConfig::default(), Duration::from_millis(100));
        let mut ctx = PipelineContext::default();
        runner
            .prepare_decode(&InputRef::TrackToken("track-a".to_string()), &mut ctx)
            .expect("prepare_decode failed");
        runner
            .activate_sink(
                &mut sink_session,
                &ctx,
                SinkActivationMode::ImmediateCutover,
            )
            .expect("activate_sink failed");
        runner.set_state(RunnerState::Playing);
        let result = runner
            .step(&mut sink_session, &mut ctx)
            .expect("step failed");
        assert!(matches!(result, StepResult::Produced { .. }));

        let pre = pre_seen.lock().expect("pre tap mutex poisoned");
        let post = post_seen.lock().expect("post tap mutex poisoned");
        assert_eq!(
            pre.as_slice(),
            &[StreamSpec {
                sample_rate: 48_000,
                channels: 1
            }]
        );
        assert_eq!(
            post.as_slice(),
            &[StreamSpec {
                sample_rate: 24_000,
                channels: 2
            }]
        );
    }

    #[test]
    fn stop_drain_flushes_transform_tail_into_sink() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let assembled = AssembledPipeline::from_parts(
            AssembledDecodePipeline {
                source: Box::new(TestSource),
                decoder: Box::new(TestDecoder::new(Vec::new(), 1, 48_000)),
                transforms: vec![Box::new(FlushTailTap::new(vec![0.25, 0.5]))],
                transform_chain: TransformChain::default(),
                mixer: None,
                resampler: None,
                builtin_slots: BuiltinTransformSlots {
                    gapless_trim: false,
                    ..BuiltinTransformSlots::default()
                },
            },
            Box::new(StaticSinkPlan::new(vec![Box::new(CaptureSink::new(
                Arc::clone(&captured),
            ))])),
        );

        let mut runner = assembled
            .into_runner(
                SinkLatencyConfig::default(),
                Duration::from_millis(100),
                None,
            )
            .expect("into_runner failed");
        let mut sink_session =
            SinkSession::new(SinkLatencyConfig::default(), Duration::from_millis(100));
        let mut ctx = PipelineContext::default();
        runner
            .prepare_decode(&InputRef::TrackToken("track-a".to_string()), &mut ctx)
            .expect("prepare_decode failed");
        runner
            .activate_sink(
                &mut sink_session,
                &ctx,
                SinkActivationMode::ImmediateCutover,
            )
            .expect("activate_sink failed");
        runner
            .stop_with_behavior(StopBehavior::DrainSink, &mut sink_session, &mut ctx)
            .expect("stop_with_behavior failed");

        let written = captured.lock().expect("capture sink mutex poisoned");
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].channels, 1);
        assert_eq!(written[0].samples, vec![0.25, 0.5]);
    }

    #[test]
    fn duplicate_stage_keys_are_rejected() {
        let assembled = AssembledPipeline::from_parts(
            AssembledDecodePipeline {
                source: Box::new(TestSource),
                decoder: Box::new(TestDecoder::new(vec![vec![0.0, 1.0]], 1, 48_000)),
                transforms: vec![
                    Box::new(KeyedNoopTransform::new("external.dup")),
                    Box::new(KeyedNoopTransform::new("external.dup")),
                ],
                transform_chain: TransformChain::default(),
                mixer: None,
                resampler: None,
                builtin_slots: BuiltinTransformSlots {
                    gapless_trim: false,
                    transition_gain: false,
                    master_gain: false,
                },
            },
            Box::new(StaticSinkPlan::new(vec![Box::new(TestSink)])),
        );

        let result = assembled.into_runner(
            SinkLatencyConfig::default(),
            Duration::from_millis(100),
            None,
        );
        match result {
            Ok(_) => panic!("expected duplicate stage key validation failure"),
            Err(PipelineError::StageFailure(message)) => {
                assert!(message.contains("duplicate transform stage key"));
            },
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
}
