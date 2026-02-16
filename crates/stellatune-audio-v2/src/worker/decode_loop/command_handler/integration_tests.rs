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

use crate::assembly::{
    AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlots, DspTransformChain,
    PipelineAssembler, PipelinePlan, PipelineRuntime, ResamplerPlan, StaticSinkPlan,
};
use crate::runtime::transform::control::{TRANSITION_GAIN_STAGE_KEY, TransitionGainControl};
use crate::types::{
    DspChainItem, DspChainSpec, DspChainStage, EngineConfig, LfeMode, PauseBehavior, PlayerState,
    ResampleQuality, StopBehavior,
};
use crate::worker::decode_loop::DecodeLoopEventCallback;
use crate::worker::decode_loop::command::DecodeLoopCommand;
use crate::worker::decode_loop::command_handler::handle_command;
use crate::worker::decode_loop::loop_state::DecodeLoopState;

#[derive(Clone)]
struct TestAssembler {
    applied_dsp_chains: Arc<Mutex<Vec<DspChainSpec>>>,
    pipeline_config: HarnessPipelineConfig,
}

impl TestAssembler {
    fn new(
        applied_dsp_chains: Arc<Mutex<Vec<DspChainSpec>>>,
        pipeline_config: HarnessPipelineConfig,
    ) -> Self {
        Self {
            applied_dsp_chains,
            pipeline_config,
        }
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
            applied_dsp_chains: Arc::clone(&self.applied_dsp_chains),
            pipeline_config: self.pipeline_config.clone(),
        })
    }
}

#[derive(Debug)]
struct TestPlan {
    track_token: String,
}

struct TestRuntime {
    applied_dsp_chains: Arc<Mutex<Vec<DspChainSpec>>>,
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
        if let Some(probe) = self.pipeline_config.probe_stage.as_ref() {
            transforms.push(Box::new(ProbeControlStage::new(
                probe.stage_key.clone(),
                Arc::clone(&probe.apply_count),
            )));
        }
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
                dsp_chain: DspTransformChain::default(),
                mixer: None,
                resampler,
                builtin_slots: BuiltinTransformSlots {
                    gapless_trim: self.pipeline_config.gapless_trim_spec.is_some(),
                    transition_gain: true,
                    master_gain: false,
                },
            },
            Box::new(StaticSinkPlan::new(vec![Box::new(TestSink::default())])),
        ))
    }

    fn apply_dsp_chain(&mut self, spec: DspChainSpec) -> Result<(), PipelineError> {
        self.applied_dsp_chains
            .lock()
            .expect("applied dsp chain log mutex poisoned")
            .push(spec);
        Ok(())
    }
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

#[derive(Clone)]
struct ProbeStageConfig {
    stage_key: String,
    apply_count: Arc<AtomicUsize>,
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
    callback: DecodeLoopEventCallback,
    runtime: Box<dyn PipelineRuntime>,
    state: DecodeLoopState,
    transition_requests: Arc<Mutex<Vec<GainTransitionRequest>>>,
    applied_dsp_chains: Arc<Mutex<Vec<DspChainSpec>>>,
}

#[derive(Clone)]
struct HarnessPipelineConfig {
    decoder_sample_rate: u32,
    resampler_target_sample_rate: Option<u32>,
    gapless_trim_spec: Option<GaplessTrimSpec>,
    probe_stage: Option<ProbeStageConfig>,
}

impl Default for HarnessPipelineConfig {
    fn default() -> Self {
        Self {
            decoder_sample_rate: 1000,
            resampler_target_sample_rate: None,
            gapless_trim_spec: None,
            probe_stage: None,
        }
    }
}

impl TestHarness {
    fn new() -> Self {
        Self::with_pipeline_config(HarnessPipelineConfig::default())
    }

    fn with_pipeline_config(pipeline_config: HarnessPipelineConfig) -> Self {
        let transition_requests = Arc::new(Mutex::new(Vec::new()));
        let applied_dsp_chains = Arc::new(Mutex::new(Vec::new()));
        let assembler: Arc<dyn PipelineAssembler> = Arc::new(TestAssembler::new(
            Arc::clone(&applied_dsp_chains),
            pipeline_config,
        ));
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
            state: DecodeLoopState::new(
                config.sink_latency,
                config.sink_recovery,
                config.gain_transition,
                config.sink_control_timeout,
            ),
            transition_requests,
            applied_dsp_chains,
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

    fn open(&mut self, track_token: &str, start_playing: bool) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::Open {
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
        if result.is_ok() {
            if let Some(active_runner) = self.state.runner.as_mut() {
                active_runner
                    .set_transition_request_log_sink(Arc::clone(&self.transition_requests));
            }
        }
        result
    }

    fn seek(&mut self, position_ms: i64) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::Seek {
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

    fn pause(&mut self, behavior: PauseBehavior) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::Pause { behavior, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx.recv().expect("pause response channel closed")
    }

    fn stop(&mut self, behavior: StopBehavior) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::Stop { behavior, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx.recv().expect("stop response channel closed")
    }

    fn set_master_gain_level(&mut self, level: f32) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::SetMasterGainLevel { level, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx
            .recv()
            .expect("set_master_gain_level response channel closed")
    }

    fn set_dsp_chain(&mut self, spec: DspChainSpec) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::SetDspChain { spec, resp_tx },
            &self.assembler,
            &self.callback,
            self.runtime.as_mut(),
            &mut self.state,
        );
        assert!(!should_break);
        resp_rx
            .recv()
            .expect("set_dsp_chain response channel closed")
    }

    fn set_lfe_mode(&mut self, mode: LfeMode) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::SetLfeMode { mode, resp_tx },
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

    fn set_resample_quality(&mut self, quality: ResampleQuality) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::SetResampleQuality { quality, resp_tx },
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

    fn apply_stage_control<T>(&mut self, stage_key: &str, control: T) -> Result<(), String>
    where
        T: Any + Send + 'static,
    {
        let (resp_tx, resp_rx) = bounded(1);
        let should_break = handle_command(
            DecodeLoopCommand::ApplyStageControl {
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

    fn applied_dsp_chains(&self) -> Vec<DspChainSpec> {
        self.applied_dsp_chains
            .lock()
            .expect("applied dsp chain log mutex poisoned")
            .clone()
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
fn master_gain_command_persists_across_context_resets() {
    let mut harness = TestHarness::new();
    harness
        .set_master_gain_level(0.5)
        .expect("set master gain should succeed");

    assert!((harness.state.master_gain_level - 0.5).abs() < f32::EPSILON);
    harness
        .open("track-a", true)
        .expect("open command should succeed");
    assert!((harness.state.master_gain_level - 0.5).abs() < f32::EPSILON);

    harness
        .stop(StopBehavior::Immediate)
        .expect("stop command should succeed");
    assert!((harness.state.master_gain_level - 0.5).abs() < f32::EPSILON);
}

#[test]
fn seek_command_uses_resampled_remaining_frames_hint_for_near_eof_fade_out() {
    let mut pipeline_config = HarnessPipelineConfig::default();
    pipeline_config.resampler_target_sample_rate = Some(2_000);
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
    let mut pipeline_config = HarnessPipelineConfig::default();
    pipeline_config.resampler_target_sample_rate = Some(2_000);
    pipeline_config.gapless_trim_spec = Some(GaplessTrimSpec {
        head_frames: 0,
        tail_frames: 1,
    });
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
fn set_dsp_chain_forwards_typed_spec_to_runtime() {
    let mut harness = TestHarness::new();
    let spec = DspChainSpec {
        items: vec![
            DspChainItem {
                plugin_id: "plugin-a".to_string(),
                type_id: "eq".to_string(),
                config_json: "{\"gain\":1.2}".to_string(),
                stage: DspChainStage::PreMix,
            },
            DspChainItem {
                plugin_id: "plugin-b".to_string(),
                type_id: "limiter".to_string(),
                config_json: "{\"threshold\":-1}".to_string(),
                stage: DspChainStage::PostMix,
            },
        ],
    };

    harness
        .set_dsp_chain(spec.clone())
        .expect("set_dsp_chain should succeed");

    let applied = harness.applied_dsp_chains();
    assert_eq!(applied, vec![spec]);
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
    assert!(result.is_err());
    assert!(
        result
            .expect_err("expected missing stage key error")
            .contains("stage key")
    );
}

#[test]
fn apply_stage_control_persists_and_replays_across_runner_rebuilds() {
    let apply_count = Arc::new(AtomicUsize::new(0));
    let mut pipeline_config = HarnessPipelineConfig::default();
    pipeline_config.probe_stage = Some(ProbeStageConfig {
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
