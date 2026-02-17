use std::any::Any;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam_channel::bounded;
use stellatune_audio_core::pipeline::context::{
    AudioBlock, GainTransitionRequest, GaplessTrimSpec, InputRef, PipelineContext, SourceHandle,
    StreamSpec, TransitionCurve, TransitionTimePolicy,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;

use crate::config::engine::{
    EngineConfig, LfeMode, PauseBehavior, PlayerState, ResampleQuality, StopBehavior,
};
use crate::error::DecodeError;
use crate::pipeline::assembly::{
    AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlots, OpaqueTransformStageSpec,
    PipelineAssembler, PipelineMutation, PipelinePlan, PipelineRuntime, ResamplerPlan,
    StaticSinkPlan, TransformChain,
};
use crate::pipeline::graph::{
    TransformGraph, TransformGraphMutation, TransformGraphStage, TransformPosition,
    TransformSegment,
};
use crate::pipeline::runtime::dsp::control::MasterGainHotControl;
use crate::pipeline::runtime::dsp::control::{TRANSITION_GAIN_STAGE_KEY, TransitionGainControl};
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::command::DecodeWorkerCommand;
use crate::workers::decode::handlers::handle_command;
use crate::workers::decode::state::DecodeWorkerState;

#[derive(Clone)]
struct TestAssembler {
    pipeline_config: HarnessPipelineConfig,
}

impl TestAssembler {
    fn new(pipeline_config: HarnessPipelineConfig) -> Self {
        Self { pipeline_config }
    }
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
            pipeline_config: self.pipeline_config.clone(),
        })
    }
}

#[derive(Debug)]
struct TestPlan {
    track_token: String,
}

struct TestRuntime {
    pipeline_config: HarnessPipelineConfig,
}

impl PipelineRuntime for TestRuntime {
    fn ensure(&mut self, plan: &dyn PipelinePlan) -> Result<AssembledPipeline, PipelineError> {
        let Some(plan) = (plan as &dyn Any).downcast_ref::<TestPlan>() else {
            return Err(PipelineError::StageFailure(
                "unexpected pipeline plan type in test runtime".to_string(),
            ));
        };
        let blocks = near_eof_blocks_for_track(&plan.track_token);
        let resampler = self
            .pipeline_config
            .resampler_target_sample_rate
            .map(|target_rate| ResamplerPlan::new(target_rate, ResampleQuality::Balanced));
        let mut transforms: Vec<
            Box<dyn stellatune_audio_core::pipeline::stages::transform::TransformStage>,
        > = Vec::new();
        transforms.extend(
            self.pipeline_config
                .probe_graph
                .main
                .iter()
                .map(ProbeControlStage::from_config)
                .map(|stage| {
                    Box::new(stage)
                        as Box<
                            dyn stellatune_audio_core::pipeline::stages::transform::TransformStage,
                        >
                }),
        );
        let mut transform_chain = TransformChain::default();
        transform_chain.pre_mix.extend(
            self.pipeline_config
                .probe_graph
                .pre_mix
                .iter()
                .map(ProbeControlStage::from_config)
                .map(|stage| {
                    Box::new(stage)
                        as Box<
                            dyn stellatune_audio_core::pipeline::stages::transform::TransformStage,
                        >
                }),
        );
        transform_chain.post_mix.extend(
            self.pipeline_config
                .probe_graph
                .post_mix
                .iter()
                .map(ProbeControlStage::from_config)
                .map(|stage| {
                    Box::new(stage)
                        as Box<
                            dyn stellatune_audio_core::pipeline::stages::transform::TransformStage,
                        >
                }),
        );
        Ok(AssembledPipeline::from_parts(
            AssembledDecodePipeline {
                source: Box::new(TestSource::default()),
                decoder: Box::new(TestDecoder::new(
                    blocks,
                    1,
                    self.pipeline_config.decoder_sample_rate,
                    self.pipeline_config.gapless_trim_spec,
                )),
                transforms,
                transform_chain,
                mixer: None,
                resampler,
                builtin_slots: BuiltinTransformSlots {
                    gapless_trim: self.pipeline_config.gapless_trim_spec.is_some(),
                    transition_gain: true,
                    master_gain: false,
                },
            },
            Box::new(StaticSinkPlan::new(vec![Box::new(TestSink)])),
        ))
    }

    fn apply_pipeline_mutation(&mut self, mutation: PipelineMutation) -> Result<(), PipelineError> {
        match mutation {
            PipelineMutation::SetResamplerPlan { resampler } => {
                self.pipeline_config.resampler_target_sample_rate =
                    resampler.map(|plan| plan.target_sample_rate);
                Ok(())
            },
            PipelineMutation::MutateTransformGraph { mutation } => {
                let typed = decode_probe_graph_mutation(mutation)?;
                self.pipeline_config
                    .probe_graph
                    .apply_mutation(typed)
                    .map_err(|error| PipelineError::StageFailure(error.to_string()))?;
                self.pipeline_config
                    .probe_graph
                    .validate_unique_stage_keys()
                    .map_err(|error| PipelineError::StageFailure(error.to_string()))?;
                Ok(())
            },
            PipelineMutation::SetMixerPlan { .. } => Ok(()),
            PipelineMutation::SetBuiltinTransformSlot { .. } => Ok(()),
        }
    }
}

fn decode_probe_graph_mutation(
    mutation: TransformGraphMutation<OpaqueTransformStageSpec>,
) -> Result<TransformGraphMutation<ProbeStageConfig>, PipelineError> {
    match mutation {
        TransformGraphMutation::Insert {
            segment,
            position,
            stage,
        } => Ok(TransformGraphMutation::Insert {
            segment,
            position,
            stage: decode_probe_stage(stage)?,
        }),
        TransformGraphMutation::Replace {
            target_stage_key,
            stage,
        } => Ok(TransformGraphMutation::Replace {
            target_stage_key,
            stage: decode_probe_stage(stage)?,
        }),
        TransformGraphMutation::Remove { target_stage_key } => {
            Ok(TransformGraphMutation::Remove { target_stage_key })
        },
        TransformGraphMutation::Move {
            target_stage_key,
            segment,
            position,
        } => Ok(TransformGraphMutation::Move {
            target_stage_key,
            segment,
            position,
        }),
    }
}

fn decode_probe_stage(stage: OpaqueTransformStageSpec) -> Result<ProbeStageConfig, PipelineError> {
    let mut decoded = stage
        .payload_ref::<ProbeStageConfig>()
        .cloned()
        .ok_or_else(|| {
            PipelineError::StageFailure(format!(
                "expected ProbeStageConfig payload for stage '{}'",
                stage.stage_key
            ))
        })?;
    decoded.stage_key = stage.stage_key;
    Ok(decoded)
}

fn near_eof_blocks_for_track(track_token: &str) -> Vec<Vec<f32>> {
    if track_token == "track-b" {
        return vec![vec![0.5, 0.5, 0.5]];
    }
    vec![vec![1.0, 1.0, 1.0]]
}

#[derive(Default)]
struct TestSource {
    prepared: bool,
}

impl SourceStage for TestSource {
    fn prepare(
        &mut self,
        _input: &InputRef,
        _ctx: &mut PipelineContext,
    ) -> Result<SourceHandle, PipelineError> {
        self.prepared = true;
        Ok(SourceHandle::new(()))
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.prepared = false;
    }
}

struct TestDecoder {
    blocks: VecDeque<Vec<f32>>,
    channels: u16,
    sample_rate: u32,
    gapless_trim_spec: Option<GaplessTrimSpec>,
}

impl TestDecoder {
    fn new(
        blocks: Vec<Vec<f32>>,
        channels: u16,
        sample_rate: u32,
        gapless_trim_spec: Option<GaplessTrimSpec>,
    ) -> Self {
        Self {
            blocks: blocks.into(),
            channels: channels.max(1),
            sample_rate: sample_rate.max(1),
            gapless_trim_spec,
        }
    }

    fn remaining_frames(&self) -> u64 {
        let channels = self.channels as usize;
        self.blocks
            .iter()
            .map(|samples| (samples.len() / channels) as u64)
            .sum()
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

    fn current_gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.gapless_trim_spec
    }

    fn estimated_remaining_frames(&self) -> Option<u64> {
        Some(self.remaining_frames())
    }

    fn next_block(&mut self, out: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        let Some(samples) = self.blocks.pop_front() else {
            return StageStatus::Eof;
        };
        out.channels = self.channels;
        out.samples.clear();
        out.samples.extend(samples);
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

#[derive(Debug)]
struct ProbeControl;

#[derive(Debug, Clone)]
struct ProbeStageConfig {
    stage_key: String,
    apply_count: Arc<AtomicUsize>,
}

impl TransformGraphStage for ProbeStageConfig {
    fn stage_key(&self) -> &str {
        &self.stage_key
    }
}

struct ProbeControlStage {
    stage_key: String,
    apply_count: Arc<AtomicUsize>,
}

impl ProbeControlStage {
    fn new(stage_key: String, apply_count: Arc<AtomicUsize>) -> Self {
        Self {
            stage_key,
            apply_count,
        }
    }

    fn from_config(config: &ProbeStageConfig) -> Self {
        Self::new(config.stage_key.clone(), Arc::clone(&config.apply_count))
    }
}

impl stellatune_audio_core::pipeline::stages::transform::TransformStage for ProbeControlStage {
    fn stage_key(&self) -> Option<&str> {
        Some(&self.stage_key)
    }

    fn apply_control(
        &mut self,
        control: &dyn Any,
        _ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        if control.downcast_ref::<ProbeControl>().is_some() {
            self.apply_count.fetch_add(1, Ordering::SeqCst);
            return Ok(true);
        }
        Ok(false)
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

struct TestHarness {
    assembler: Arc<dyn PipelineAssembler>,
    callback: DecodeWorkerEventCallback,
    runtime: Box<dyn PipelineRuntime>,
    state: DecodeWorkerState,
    transition_requests: Arc<Mutex<Vec<GainTransitionRequest>>>,
}

#[derive(Clone)]
struct HarnessPipelineConfig {
    decoder_sample_rate: u32,
    resampler_target_sample_rate: Option<u32>,
    gapless_trim_spec: Option<GaplessTrimSpec>,
    probe_graph: TransformGraph<ProbeStageConfig>,
}

impl Default for HarnessPipelineConfig {
    fn default() -> Self {
        Self {
            decoder_sample_rate: 1000,
            resampler_target_sample_rate: None,
            gapless_trim_spec: None,
            probe_graph: TransformGraph::default(),
        }
    }
}

impl TestHarness {
    fn new() -> Self {
        Self::with_pipeline_config(HarnessPipelineConfig::default())
    }

    fn with_pipeline_config(pipeline_config: HarnessPipelineConfig) -> Self {
        let transition_requests = Arc::new(Mutex::new(Vec::new()));
        let assembler: Arc<dyn PipelineAssembler> = Arc::new(TestAssembler::new(pipeline_config));
        let mut config = EngineConfig::default();
        config.gain_transition.seek_fade_out_ms = 48;
        config.gain_transition.pause_fade_out_ms = 48;
        config.gain_transition.stop_fade_out_ms = 48;
        config.gain_transition.switch_fade_out_ms = 48;
        config.gain_transition.interrupt_max_extra_wait_ms = 4;

        Self {
            runtime: assembler.create_runtime(),
            assembler,
            callback: Arc::new(|_| {}),
            state: DecodeWorkerState::new(
                config.sink_latency,
                config.sink_recovery,
                config.gain_transition,
                config.sink_control_timeout,
                Arc::new(MasterGainHotControl::default()),
            ),
            transition_requests,
        }
    }

    fn clear_transition_requests(&mut self) {
        self.transition_requests
            .lock()
            .expect("transition request log mutex poisoned")
            .clear();
    }

    fn transition_requests(&self) -> Vec<GainTransitionRequest> {
        self.transition_requests
            .lock()
            .expect("transition request log mutex poisoned")
            .clone()
    }

    fn force_full_transition_gain(&mut self) {
        if let Some(active_runner) = self.state.runner.as_mut() {
            let _ = active_runner.apply_transform_control_to(
                TRANSITION_GAIN_STAGE_KEY,
                &TransitionGainControl::new(GainTransitionRequest {
                    target_gain: 1.0,
                    ramp_ms: 0,
                    available_frames_hint: None,
                    curve: TransitionCurve::Linear,
                    time_policy: TransitionTimePolicy::Exact,
                }),
                &mut self.state.ctx,
            );
        }
    }

    fn playable_remaining_frames_hint(&self) -> Option<u64> {
        self.state
            .runner
            .as_ref()
            .and_then(|runner| runner.playable_remaining_frames_hint())
    }

    fn open(&mut self, track_token: &str, start_playing: bool) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::Open {
                input: InputRef::TrackToken(track_token.to_string()),
                start_playing,
                resp_tx,
            },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        let result = resp_rx.recv().expect("open response channel closed");
        if result.is_ok()
            && let Some(active_runner) = self.state.runner.as_mut()
        {
            active_runner.set_transition_request_log_sink(Arc::clone(&self.transition_requests));
        }
        result
    }

    fn seek(&mut self, position_ms: i64) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::Seek {
                position_ms,
                resp_tx,
            },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx.recv().expect("seek response channel closed")
    }

    fn pause(&mut self, behavior: PauseBehavior) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::Pause { behavior, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx.recv().expect("pause response channel closed")
    }

    fn stop(&mut self, behavior: StopBehavior) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::Stop { behavior, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx.recv().expect("stop response channel closed")
    }

    fn set_master_gain_level(&mut self, level: f32) -> Result<(), DecodeError> {
        self.state.master_gain_hot_control.update(level, 0, None);
        Ok(())
    }

    fn set_lfe_mode(&mut self, mode: LfeMode) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::SetLfeMode { mode, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx
            .recv()
            .expect("set_lfe_mode response channel closed")
    }

    fn set_resample_quality(&mut self, quality: ResampleQuality) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::SetResampleQuality { quality, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx
            .recv()
            .expect("set_resample_quality response channel closed")
    }

    fn apply_stage_control<T>(&mut self, stage_key: &str, control: T) -> Result<(), DecodeError>
    where
        T: Any + Send + 'static,
    {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::ApplyStageControl {
                stage_key: stage_key.to_string(),
                control: Box::new(control),
                resp_tx,
            },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx
            .recv()
            .expect("apply_stage_control response channel closed")
    }

    fn apply_pipeline_mutation(&mut self, mutation: PipelineMutation) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::ApplyPipelineMutation { mutation, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx
            .recv()
            .expect("apply_pipeline_mutation response channel closed")
    }

    fn queue_next(&mut self, track_token: &str) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::QueueNext {
                input: InputRef::TrackToken(track_token.to_string()),
                resp_tx,
            },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx.recv().expect("queue_next response channel closed")
    }

    fn play(&mut self) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::Play { resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx.recv().expect("play response channel closed")
    }

    fn apply_pipeline_plan(&mut self, plan: Arc<dyn PipelinePlan>) -> Result<(), DecodeError> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::ApplyPipelinePlan { plan, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx
            .recv()
            .expect("apply_pipeline_plan response channel closed")
    }

    fn shutdown(&mut self) -> bool {
        let (ack_tx, ack_rx) = bounded(1);
        let should_break = handle_command(
            DecodeWorkerCommand::Shutdown { ack_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        ack_rx.recv().expect("shutdown ack channel closed");
        should_break
    }
}

fn assert_has_near_eof_fade_out_request(requests: &[GainTransitionRequest], expected_hint: u64) {
    let request = requests
        .iter()
        .find(|request| request.target_gain <= f32::EPSILON)
        .expect("missing fade-out gain transition request");
    assert_eq!(request.available_frames_hint, Some(expected_hint));
    assert_eq!(request.time_policy, TransitionTimePolicy::FitToAvailable);
}

#[test]
fn seek_command_uses_remaining_frames_hint_for_near_eof_fade_out() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    harness.force_full_transition_gain();
    assert_eq!(harness.playable_remaining_frames_hint(), Some(3));

    harness.clear_transition_requests();
    harness
        .seek(1200)
        .expect("seek command should succeed after fade-out");

    let requests = harness.transition_requests();
    assert_has_near_eof_fade_out_request(&requests, 3);
    assert!(
        requests
            .iter()
            .any(|request| request.target_gain >= 1.0 - f32::EPSILON),
        "seek should request a fade-in transition"
    );
}

#[test]
fn pause_command_uses_remaining_frames_hint_for_near_eof_fade_out() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    harness.force_full_transition_gain();
    assert_eq!(harness.playable_remaining_frames_hint(), Some(3));

    harness.clear_transition_requests();
    harness
        .pause(PauseBehavior::Immediate)
        .expect("pause command should succeed after fade-out");

    let requests = harness.transition_requests();
    assert_has_near_eof_fade_out_request(&requests, 3);
    assert_eq!(harness.state.state, PlayerState::Paused);
}

#[test]
fn stop_command_uses_remaining_frames_hint_for_near_eof_fade_out() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    harness.force_full_transition_gain();
    assert_eq!(harness.playable_remaining_frames_hint(), Some(3));

    harness.clear_transition_requests();
    harness
        .stop(StopBehavior::Immediate)
        .expect("stop command should succeed after fade-out");

    let requests = harness.transition_requests();
    assert_has_near_eof_fade_out_request(&requests, 3);
    assert_eq!(harness.state.state, PlayerState::Stopped);
    assert!(harness.state.runner.is_none());
}

#[test]
fn switch_open_command_uses_remaining_frames_hint_for_near_eof_fade_out() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("first open command should succeed");
    harness.force_full_transition_gain();
    assert_eq!(harness.playable_remaining_frames_hint(), Some(3));

    harness.clear_transition_requests();
    harness
        .open("track-b", true)
        .expect("switch open command should succeed after fade-out");

    let requests = harness.transition_requests();
    assert_has_near_eof_fade_out_request(&requests, 3);
    assert_eq!(
        harness.state.active_input,
        Some(InputRef::TrackToken("track-b".to_string()))
    );
    assert_eq!(harness.state.state, PlayerState::Playing);
}

#[test]
fn master_gain_hot_state_persists_across_context_resets() {
    let mut harness = TestHarness::new();
    harness
        .set_master_gain_level(0.5)
        .expect("set master gain should succeed");

    assert!((harness.state.master_gain_hot_control.snapshot().level - 0.5).abs() < f32::EPSILON);
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    assert!((harness.state.master_gain_hot_control.snapshot().level - 0.5).abs() < f32::EPSILON);

    harness
        .stop(StopBehavior::Immediate)
        .expect("stop command should succeed");
    assert!((harness.state.master_gain_hot_control.snapshot().level - 0.5).abs() < f32::EPSILON);
}

#[test]
fn seek_command_uses_resampled_remaining_frames_hint_for_near_eof_fade_out() {
    let pipeline_config = HarnessPipelineConfig {
        resampler_target_sample_rate: Some(2_000),
        ..HarnessPipelineConfig::default()
    };
    let mut harness = TestHarness::with_pipeline_config(pipeline_config);
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    harness.force_full_transition_gain();
    assert_eq!(harness.playable_remaining_frames_hint(), Some(6));

    harness.clear_transition_requests();
    harness
        .seek(1200)
        .expect("seek command should succeed after fade-out");

    let requests = harness.transition_requests();
    assert_has_near_eof_fade_out_request(&requests, 6);
}

#[test]
fn stop_command_applies_gapless_tail_before_resampled_hint_scaling() {
    let pipeline_config = HarnessPipelineConfig {
        resampler_target_sample_rate: Some(2_000),
        gapless_trim_spec: Some(GaplessTrimSpec {
            head_frames: 0,
            tail_frames: 1,
        }),
        ..HarnessPipelineConfig::default()
    };
    let mut harness = TestHarness::with_pipeline_config(pipeline_config);
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    harness.force_full_transition_gain();
    assert_eq!(harness.playable_remaining_frames_hint(), Some(4));

    harness.clear_transition_requests();
    harness
        .stop(StopBehavior::Immediate)
        .expect("stop command should succeed after fade-out");

    let requests = harness.transition_requests();
    assert_has_near_eof_fade_out_request(&requests, 4);
}

#[test]
fn set_lfe_mode_updates_policy_and_keeps_active_runner_playing() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    assert_eq!(harness.state.state, PlayerState::Playing);
    assert!(harness.state.runner.is_some());

    harness
        .set_lfe_mode(LfeMode::MixToFront)
        .expect("set_lfe_mode should succeed");

    assert_eq!(harness.state.lfe_mode, LfeMode::MixToFront);
    assert_eq!(harness.state.state, PlayerState::Playing);
    assert!(harness.state.runner.is_some());
}

#[test]
fn set_resample_quality_updates_policy_and_keeps_active_runner_playing() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    assert_eq!(harness.state.state, PlayerState::Playing);
    assert!(harness.state.runner.is_some());

    harness
        .set_resample_quality(ResampleQuality::Ultra)
        .expect("set_resample_quality should succeed");

    assert_eq!(harness.state.resample_quality, ResampleQuality::Ultra);
    assert_eq!(harness.state.state, PlayerState::Playing);
    assert!(harness.state.runner.is_some());
}

#[test]
fn apply_stage_control_routes_to_transition_gain_stage() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    harness.clear_transition_requests();

    harness
        .apply_stage_control(
            TRANSITION_GAIN_STAGE_KEY,
            TransitionGainControl::new(GainTransitionRequest {
                target_gain: 0.25,
                ramp_ms: 24,
                available_frames_hint: Some(3),
                curve: TransitionCurve::Linear,
                time_policy: TransitionTimePolicy::FitToAvailable,
            }),
        )
        .expect("apply_stage_control should succeed");

    let requests = harness.transition_requests();
    assert_eq!(requests.len(), 1);
    assert!((requests[0].target_gain - 0.25).abs() < f32::EPSILON);
    assert_eq!(requests[0].ramp_ms, 24);
}

#[test]
fn apply_stage_control_reports_missing_stage_key() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");

    let result = harness.apply_stage_control(
        "custom.unknown.stage",
        TransitionGainControl::new(GainTransitionRequest::default()),
    );
    let error = result.expect_err("expected missing stage key error");
    assert!(matches!(
        error,
        DecodeError::TransformStageNotFound { ref stage_key }
            if stage_key == "custom.unknown.stage"
    ));
}

#[test]
fn apply_stage_control_persists_and_replays_across_runner_rebuilds() {
    let apply_count = Arc::new(AtomicUsize::new(0));
    let mut pipeline_config = HarnessPipelineConfig::default();
    pipeline_config.probe_graph.main.push(ProbeStageConfig {
        stage_key: "external.probe".to_string(),
        apply_count: Arc::clone(&apply_count),
    });
    let mut harness = TestHarness::with_pipeline_config(pipeline_config);

    harness
        .apply_stage_control("external.probe", ProbeControl)
        .expect("apply_stage_control should succeed without active runner");
    assert_eq!(apply_count.load(Ordering::SeqCst), 0);

    harness
        .open("track-a", false)
        .expect("open command should succeed");
    assert_eq!(apply_count.load(Ordering::SeqCst), 1);

    harness
        .set_lfe_mode(LfeMode::MixToFront)
        .expect("set_lfe_mode should trigger runner rebuild");
    assert_eq!(apply_count.load(Ordering::SeqCst), 2);
}

#[test]
fn apply_pipeline_mutation_rebuilds_active_runner_with_updated_runtime_config() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    assert_eq!(harness.state.state, PlayerState::Playing);
    assert_eq!(harness.playable_remaining_frames_hint(), Some(3));

    harness
        .apply_pipeline_mutation(PipelineMutation::SetResamplerPlan {
            resampler: Some(ResamplerPlan::new(2_000, ResampleQuality::Balanced)),
        })
        .expect("apply_pipeline_mutation should succeed");

    assert_eq!(harness.state.state, PlayerState::Playing);
    assert!(harness.state.runner.is_some());
    assert_eq!(harness.playable_remaining_frames_hint(), Some(6));
}

#[test]
fn apply_pipeline_mutation_supports_insert_move_replace_and_remove() {
    let apply_count_a = Arc::new(AtomicUsize::new(0));
    let apply_count_b = Arc::new(AtomicUsize::new(0));
    let apply_count_c = Arc::new(AtomicUsize::new(0));
    let mut pipeline_config = HarnessPipelineConfig::default();
    pipeline_config.probe_graph.main.push(ProbeStageConfig {
        stage_key: "external.a".to_string(),
        apply_count: Arc::clone(&apply_count_a),
    });
    let mut harness = TestHarness::with_pipeline_config(pipeline_config);
    harness
        .open("track-a", true)
        .expect("open command should succeed");

    harness
        .apply_stage_control("external.a", ProbeControl)
        .expect("control apply for stage a should succeed");
    assert_eq!(apply_count_a.load(Ordering::SeqCst), 1);

    harness
        .apply_pipeline_mutation(PipelineMutation::MutateTransformGraph {
            mutation: TransformGraphMutation::Insert {
                segment: TransformSegment::PostMix,
                position: TransformPosition::Back,
                stage: OpaqueTransformStageSpec::with_payload(
                    "external.b",
                    ProbeStageConfig {
                        stage_key: "external.b".to_string(),
                        apply_count: Arc::clone(&apply_count_b),
                    },
                ),
            },
        })
        .expect("insert stage mutation should succeed");
    harness
        .apply_stage_control("external.b", ProbeControl)
        .expect("control apply for inserted stage b should succeed");
    assert_eq!(apply_count_b.load(Ordering::SeqCst), 1);

    harness
        .apply_pipeline_mutation(PipelineMutation::MutateTransformGraph {
            mutation: TransformGraphMutation::Move {
                target_stage_key: "external.b".to_string(),
                segment: TransformSegment::PreMix,
                position: TransformPosition::Front,
            },
        })
        .expect("move stage mutation should succeed");
    harness
        .apply_stage_control("external.b", ProbeControl)
        .expect("control apply for moved stage b should succeed");
    assert_eq!(apply_count_b.load(Ordering::SeqCst), 3);

    harness
        .apply_pipeline_mutation(PipelineMutation::MutateTransformGraph {
            mutation: TransformGraphMutation::Replace {
                target_stage_key: "external.b".to_string(),
                stage: OpaqueTransformStageSpec::with_payload(
                    "external.c",
                    ProbeStageConfig {
                        stage_key: "external.c".to_string(),
                        apply_count: Arc::clone(&apply_count_c),
                    },
                ),
            },
        })
        .expect("replace stage mutation should succeed");
    let removed_b = harness.apply_stage_control("external.b", ProbeControl);
    assert!(removed_b.is_err());
    harness
        .apply_stage_control("external.c", ProbeControl)
        .expect("control apply for replaced stage c should succeed");
    assert_eq!(apply_count_c.load(Ordering::SeqCst), 1);

    harness
        .apply_pipeline_mutation(PipelineMutation::MutateTransformGraph {
            mutation: TransformGraphMutation::Remove {
                target_stage_key: "external.a".to_string(),
            },
        })
        .expect("remove stage mutation should succeed");
    let removed_a = harness.apply_stage_control("external.a", ProbeControl);
    assert!(removed_a.is_err());
}

#[test]
fn play_command_requires_active_pipeline() {
    let mut harness = TestHarness::new();
    let result = harness.play();
    let error = result.expect_err("play without active pipeline should fail");
    assert!(matches!(
        error,
        DecodeError::NoActivePipeline { operation: "play" }
    ));
}

#[test]
fn play_command_resumes_from_paused_state() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", false)
        .expect("open command should succeed");
    assert_eq!(harness.state.state, PlayerState::Paused);

    harness.play().expect("play command should succeed");
    assert_eq!(harness.state.state, PlayerState::Playing);
    assert!(harness.state.runner.is_some());
}

#[test]
fn queue_next_command_sets_queued_and_prewarmed_next() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");

    harness
        .queue_next("track-b")
        .expect("queue_next command should succeed");

    assert_eq!(
        harness.state.queued_next_input,
        Some(InputRef::TrackToken("track-b".to_string()))
    );
    let prewarmed = harness
        .state
        .prewarmed_next
        .as_ref()
        .expect("prewarmed next should exist");
    assert_eq!(prewarmed.input, InputRef::TrackToken("track-b".to_string()));
}

#[test]
fn apply_pipeline_plan_no_active_input_only_pins_plan() {
    let mut harness = TestHarness::new();
    let plan: Arc<dyn PipelinePlan> = Arc::new(TestPlan {
        track_token: "track-a".to_string(),
    });

    harness
        .apply_pipeline_plan(Arc::clone(&plan))
        .expect("apply_pipeline_plan should succeed");

    assert!(harness.state.runner.is_none());
    let pinned = harness
        .state
        .pinned_plan
        .as_ref()
        .expect("pinned plan should exist");
    let pinned_test_plan = (pinned.as_ref() as &dyn Any)
        .downcast_ref::<TestPlan>()
        .expect("pinned plan type should be TestPlan");
    assert_eq!(pinned_test_plan.track_token, "track-a");
}

#[test]
fn apply_pipeline_plan_rebuilds_active_runner_and_keeps_state() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    assert_eq!(harness.state.state, PlayerState::Playing);
    assert!(harness.state.runner.is_some());

    let plan: Arc<dyn PipelinePlan> = Arc::new(TestPlan {
        track_token: "track-a".to_string(),
    });
    harness
        .apply_pipeline_plan(plan)
        .expect("apply_pipeline_plan should succeed");

    assert_eq!(harness.state.state, PlayerState::Playing);
    assert!(harness.state.runner.is_some());
}

#[test]
fn shutdown_command_acknowledges_and_requests_loop_break() {
    let mut harness = TestHarness::new();
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    assert!(harness.state.runner.is_some());

    let should_break = harness.shutdown();
    assert!(should_break);
    assert!(harness.state.runner.is_none());
    assert_eq!(harness.state.state, PlayerState::Stopped);
    assert!(harness.state.active_input.is_none());
    assert!(harness.state.queued_next_input.is_none());
    assert!(harness.state.prewarmed_next.is_none());
}
