use std::sync::{Arc, Mutex};
use std::time::Duration;

use stellatune_audio_core::pipeline::context::{
    AudioBlock, InputRef, PipelineContext, SourceHandle, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;
use stellatune_audio_core::pipeline::stages::{Stage, StageFlow};

use crate::config::engine::EngineConfig;
use crate::config::sink::SinkRecoveryConfig;
use crate::pipeline::assembly::{
    AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlots, PipelineAssembler,
    PipelineBlueprint, PipelineMutation, PipelineRuntime, StaticSinkPlan, TransformChain,
};
use crate::pipeline::runtime::dsp::control::MasterGainHotControl;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

#[derive(Debug, Clone)]
struct TestPlan;

pub(super) struct SuccessAssembler;

impl PipelineAssembler for SuccessAssembler {
    fn build_blueprint(
        &self,
        _input: &InputRef,
    ) -> Result<Arc<dyn PipelineBlueprint>, PipelineError> {
        Ok(Arc::new(TestPlan))
    }

    fn apply_pipeline_mutation(
        &self,
        current: Option<&dyn PipelineBlueprint>,
        _mutation: PipelineMutation,
    ) -> Result<Arc<dyn PipelineBlueprint>, PipelineError> {
        if let Some(current) = current {
            return Ok(Arc::new(
                (current as &dyn std::any::Any)
                    .downcast_ref::<TestPlan>()
                    .expect("unexpected test blueprint type")
                    .clone(),
            ));
        }
        Ok(Arc::new(TestPlan))
    }

    fn create_runtime(&self) -> Box<dyn PipelineRuntime> {
        Box::new(SuccessRuntime)
    }
}

pub(super) struct FailingAssembler;

impl PipelineAssembler for FailingAssembler {
    fn build_blueprint(
        &self,
        _input: &InputRef,
    ) -> Result<Arc<dyn PipelineBlueprint>, PipelineError> {
        Err(PipelineError::StageFailure("blueprint failed".to_string()))
    }

    fn apply_pipeline_mutation(
        &self,
        current: Option<&dyn PipelineBlueprint>,
        _mutation: PipelineMutation,
    ) -> Result<Arc<dyn PipelineBlueprint>, PipelineError> {
        if let Some(current) = current {
            return Ok(Arc::new(
                (current as &dyn std::any::Any)
                    .downcast_ref::<TestPlan>()
                    .expect("unexpected test blueprint type")
                    .clone(),
            ));
        }
        Ok(Arc::new(TestPlan))
    }

    fn create_runtime(&self) -> Box<dyn PipelineRuntime> {
        Box::new(FailingRuntime)
    }
}

pub(super) struct SuccessRuntime;

impl PipelineRuntime for SuccessRuntime {
    fn assemble(
        &mut self,
        _blueprint: &dyn PipelineBlueprint,
    ) -> Result<AssembledPipeline, PipelineError> {
        Ok(test_pipeline())
    }
}

pub(super) struct FailingRuntime;

impl PipelineRuntime for FailingRuntime {
    fn assemble(
        &mut self,
        _blueprint: &dyn PipelineBlueprint,
    ) -> Result<AssembledPipeline, PipelineError> {
        Err(PipelineError::StageFailure("assemble failed".to_string()))
    }
}

#[derive(Default)]
struct TestSource;

impl Stage for TestSource {}

impl SourceStage for TestSource {
    fn prepare(
        &mut self,
        _input: &InputRef,
        _ctx: &mut PipelineContext,
    ) -> Result<SourceHandle, PipelineError> {
        Ok(SourceHandle::new(()))
    }
    fn stop(&mut self, _ctx: &mut PipelineContext) {}
}

#[derive(Default)]
struct TestDecoder;

impl Stage for TestDecoder {}

impl DecoderStage for TestDecoder {
    fn prepare(
        &mut self,
        _source: &SourceHandle,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        Ok(StreamSpec {
            sample_rate: 48_000,
            channels: 2,
        })
    }
    fn next_block(
        &mut self,
        _out: &mut AudioBlock,
        _ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError> {
        Ok(StageFlow::Eof)
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {}
}

#[derive(Default)]
struct TestSink;

impl Stage for TestSink {}

impl SinkStage for TestSink {
    fn prepare(
        &mut self,
        _spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        Ok(())
    }
    fn write(
        &mut self,
        _block: &AudioBlock,
        _ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError> {
        Ok(StageFlow::Continue)
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {}
}

fn test_pipeline() -> AssembledPipeline {
    AssembledPipeline::from_parts(
        AssembledDecodePipeline {
            source: Box::new(TestSource),
            decoder: Box::new(TestDecoder),
            transforms: Vec::new(),
            transform_chain: TransformChain::default(),
            mixer: None,
            resampler: None,
            builtin_slots: BuiltinTransformSlots::default(),
        },
        Box::new(StaticSinkPlan::new(vec![Box::new(TestSink)])),
    )
}

pub(super) fn test_config() -> EngineConfig {
    EngineConfig {
        sink_recovery: SinkRecoveryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(20),
            max_backoff: Duration::from_millis(80),
        },
        ..EngineConfig::default()
    }
}

pub(super) fn test_state(config: &EngineConfig) -> DecodeWorkerState {
    DecodeWorkerState::new(
        config.sink_latency,
        config.sink_recovery,
        config.gain_transition,
        config.sink_control_timeout,
        Arc::new(MasterGainHotControl::default()),
    )
}

pub(super) fn event_callback() -> (
    DecodeWorkerEventCallback,
    Arc<Mutex<Vec<DecodeWorkerEvent>>>,
) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_for_callback = Arc::clone(&events);
    let callback: DecodeWorkerEventCallback = Arc::new(move |event| {
        events_for_callback
            .lock()
            .expect("events mutex poisoned")
            .push(event);
    });
    (callback, events)
}

pub(super) fn shutdown_state(mut state: DecodeWorkerState) {
    if let Some(mut runner) = state.runner.take() {
        runner.stop(&mut state.sink_session, &mut state.ctx);
    } else {
        state.sink_session.shutdown(false);
    }
}
