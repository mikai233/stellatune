use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, unbounded};
use stellatune_audio_core::pipeline::context::{
    AudioBlock, InputRef, PipelineContext, SourceHandle, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;

use crate::config::engine::EngineConfig;
use crate::pipeline::assembly::{
    AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlots, PipelineAssembler,
    PipelineMutation, PipelinePlan, PipelineRuntime, StaticSinkPlan, TransformChain,
};
use crate::pipeline::runtime::dsp::control::MasterGainHotControl;
use crate::workers::decode::{DecodeWorker, DecodeWorkerEvent, DecodeWorkerEventCallback};

#[derive(Debug)]
struct TestPlan {
    track_token: String,
}

#[derive(Debug, Clone)]
pub(super) enum EnsureAction {
    Succeed,
    Fail(&'static str),
}

#[derive(Default, Debug)]
pub(super) struct RuntimeState {
    track_blocks: HashMap<String, usize>,
    ensure_calls: HashMap<String, usize>,
    ensure_scripts: HashMap<String, VecDeque<EnsureAction>>,
}

impl RuntimeState {
    pub(super) fn set_track_blocks(&mut self, track_token: &str, blocks: usize) {
        self.track_blocks
            .insert(track_token.to_string(), blocks.max(1));
    }

    pub(super) fn set_ensure_script(&mut self, track_token: &str, script: Vec<EnsureAction>) {
        self.ensure_scripts
            .insert(track_token.to_string(), script.into());
    }

    fn track_blocks(&self, track_token: &str) -> usize {
        self.track_blocks
            .get(track_token)
            .copied()
            .unwrap_or(2)
            .max(1)
    }

    fn ensure_count(&self, track_token: &str) -> usize {
        self.ensure_calls.get(track_token).copied().unwrap_or(0)
    }
}

#[derive(Clone)]
struct TestAssembler {
    state: Arc<Mutex<RuntimeState>>,
}

impl PipelineAssembler for TestAssembler {
    fn plan(&self, input: &InputRef) -> Result<Arc<dyn PipelinePlan>, PipelineError> {
        let track_token = match input {
            InputRef::TrackToken(track_token) => track_token.clone(),
        };
        Ok(Arc::new(TestPlan { track_token }))
    }

    fn create_runtime(&self) -> Box<dyn PipelineRuntime> {
        Box::new(TestRuntime {
            state: Arc::clone(&self.state),
        })
    }
}

struct TestRuntime {
    state: Arc<Mutex<RuntimeState>>,
}

impl PipelineRuntime for TestRuntime {
    fn ensure(&mut self, plan: &dyn PipelinePlan) -> Result<AssembledPipeline, PipelineError> {
        let Some(plan) = (plan as &dyn Any).downcast_ref::<TestPlan>() else {
            return Err(PipelineError::StageFailure(
                "unexpected test plan type".to_string(),
            ));
        };

        let mut state = self.state.lock().expect("runtime state mutex poisoned");
        let track_token = plan.track_token.clone();
        let next_call = state
            .ensure_calls
            .entry(track_token.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        let _ = next_call;

        if let Some(script) = state.ensure_scripts.get_mut(&track_token)
            && let Some(action) = script.pop_front()
        {
            match action {
                EnsureAction::Succeed => {},
                EnsureAction::Fail(message) => {
                    return Err(PipelineError::StageFailure(message.to_string()));
                },
            }
        }

        let blocks = state.track_blocks(&track_token);
        drop(state);

        Ok(AssembledPipeline::from_parts(
            AssembledDecodePipeline {
                source: Box::new(TestSource),
                decoder: Box::new(TestDecoder::new(blocks)),
                transforms: Vec::new(),
                transform_chain: TransformChain::default(),
                mixer: None,
                resampler: None,
                builtin_slots: BuiltinTransformSlots::default(),
            },
            Box::new(StaticSinkPlan::new(vec![Box::new(TestSink)])),
        ))
    }

    fn apply_pipeline_mutation(
        &mut self,
        _mutation: PipelineMutation,
    ) -> Result<(), PipelineError> {
        Ok(())
    }
}

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
    remaining_blocks: usize,
}

impl TestDecoder {
    fn new(remaining_blocks: usize) -> Self {
        Self { remaining_blocks }
    }
}

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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn next_block(&mut self, out: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        if self.remaining_blocks == 0 {
            return StageStatus::Eof;
        }
        self.remaining_blocks = self.remaining_blocks.saturating_sub(1);
        out.channels = 2;
        out.samples.clear();
        out.samples.extend([0.25, 0.25]);
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {}
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

pub(super) struct LoopHarness {
    worker: Option<DecodeWorker>,
    events_rx: Receiver<DecodeWorkerEvent>,
    state: Arc<Mutex<RuntimeState>>,
    command_timeout: Duration,
}

impl LoopHarness {
    pub(super) fn start(runtime_state: RuntimeState) -> Self {
        let state = Arc::new(Mutex::new(runtime_state));
        let assembler: Arc<dyn PipelineAssembler> = Arc::new(TestAssembler {
            state: Arc::clone(&state),
        });
        let (events_tx, events_rx) = unbounded::<DecodeWorkerEvent>();
        let callback: DecodeWorkerEventCallback = Arc::new(move |event| {
            let _ = events_tx.send(event);
        });
        let config = EngineConfig {
            decode_playing_pending_block_sleep: Duration::from_micros(250),
            decode_playing_idle_sleep: Duration::from_millis(1),
            decode_idle_sleep: Duration::from_millis(1),
            ..EngineConfig::default()
        };
        let worker = DecodeWorker::start(
            assembler,
            config,
            callback,
            Arc::new(MasterGainHotControl::default()),
        );
        Self {
            worker: Some(worker),
            events_rx,
            state,
            command_timeout: Duration::from_secs(2),
        }
    }

    pub(super) fn open(&self, track_token: &str, start_playing: bool) -> Result<(), String> {
        self.worker_ref()
            .open(track_token.to_string(), start_playing, self.command_timeout)
    }

    pub(super) fn queue_next(&self, track_token: &str) -> Result<(), String> {
        self.worker_ref()
            .queue_next(track_token.to_string(), self.command_timeout)
    }

    pub(super) fn play(&self) -> Result<(), String> {
        self.worker_ref().play(self.command_timeout)
    }

    pub(super) fn ensure_count(&self, track_token: &str) -> usize {
        self.state
            .lock()
            .expect("runtime state mutex poisoned")
            .ensure_count(track_token)
    }

    pub(super) fn wait_for_track_changed(
        &self,
        track_token: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(format!("timed out waiting for track '{track_token}'"));
            }
            let wait = deadline.saturating_duration_since(now);
            match self.events_rx.recv_timeout(wait) {
                Ok(DecodeWorkerEvent::TrackChanged {
                    track_token: current,
                }) if current == track_token => {
                    return Ok(());
                },
                Ok(DecodeWorkerEvent::Error(message)) => {
                    return Err(format!(
                        "unexpected decode worker error while waiting for track '{track_token}': {message}"
                    ));
                },
                Ok(_) => {},
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("timed out waiting for track '{track_token}'"));
                },
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("event channel disconnected".to_string());
                },
            }
        }
    }

    pub(super) fn shutdown(mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.shutdown(Duration::from_secs(2));
        }
    }

    fn worker_ref(&self) -> &DecodeWorker {
        self.worker.as_ref().expect("decode worker not available")
    }
}

impl Drop for LoopHarness {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.shutdown(Duration::from_millis(200));
        }
    }
}
