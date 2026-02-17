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

use crate::config::engine::{LfeMode, ResampleQuality, StopBehavior};
use crate::config::sink::SinkLatencyConfig;
use crate::pipeline::runtime::runner::{RunnerState, StepResult};
use crate::pipeline::runtime::sink_session::{SinkActivationMode, SinkSession};

use crate::pipeline::assembly::{
    AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlots, MixerPlan, ResamplerPlan,
    StaticSinkPlan, TransformChain,
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    let mut runner = assembled.into_runner(None).expect("into_runner failed");
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

    let mut runner = assembled.into_runner(None).expect("into_runner failed");
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

    let result = assembled.into_runner(None);
    match result {
        Ok(_) => panic!("expected duplicate stage key validation failure"),
        Err(PipelineError::StageFailure(message)) => {
            assert!(message.contains("duplicate transform stage key"));
        },
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}
