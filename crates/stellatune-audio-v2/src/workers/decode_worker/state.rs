use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use stellatune_audio_core::pipeline::context::{InputRef, PipelineContext};

use crate::assembly::PipelinePlan;
use crate::runtime::runner::PipelineRunner;
use crate::runtime::sink_session::SinkSession;
use crate::runtime::transform::control::SharedMasterGainHotControl;
use crate::types::{
    GainTransitionConfig, LfeMode, PlayerState, ResampleQuality, SinkLatencyConfig,
    SinkRecoveryConfig,
};

pub(crate) struct PrewarmedNext {
    pub(crate) input: InputRef,
    pub(crate) runner: PipelineRunner,
    pub(crate) ctx: PipelineContext,
}

pub(crate) struct DecodeWorkerState {
    pub(crate) runner: Option<PipelineRunner>,
    pub(crate) ctx: PipelineContext,
    pub(crate) master_gain_hot_control: SharedMasterGainHotControl,
    pub(crate) state: PlayerState,
    pub(crate) active_input: Option<InputRef>,
    pub(crate) queued_next_input: Option<InputRef>,
    pub(crate) prewarmed_next: Option<PrewarmedNext>,
    pub(crate) pinned_plan: Option<Arc<dyn PipelinePlan>>,
    pub(crate) last_position_emit_at: Instant,
    pub(crate) sink_latency: SinkLatencyConfig,
    pub(crate) sink_recovery: SinkRecoveryConfig,
    pub(crate) gain_transition: GainTransitionConfig,
    pub(crate) sink_control_timeout: Duration,
    pub(crate) sink_session: SinkSession,
    pub(crate) lfe_mode: LfeMode,
    pub(crate) resample_quality: ResampleQuality,
    pub(crate) persisted_stage_controls: HashMap<String, Box<dyn Any + Send>>,
    pub(crate) recovery_attempts: u32,
    pub(crate) recovery_retry_at: Option<Instant>,
}

impl DecodeWorkerState {
    pub(crate) fn new(
        sink_latency: SinkLatencyConfig,
        sink_recovery: SinkRecoveryConfig,
        gain_transition: GainTransitionConfig,
        sink_control_timeout: Duration,
        master_gain_hot_control: SharedMasterGainHotControl,
    ) -> Self {
        let ctx = PipelineContext::default();
        Self {
            runner: None,
            ctx,
            master_gain_hot_control,
            state: PlayerState::Stopped,
            active_input: None,
            queued_next_input: None,
            prewarmed_next: None,
            pinned_plan: None,
            last_position_emit_at: Instant::now(),
            sink_latency,
            sink_recovery,
            gain_transition,
            sink_control_timeout,
            sink_session: SinkSession::new(sink_latency, sink_control_timeout),
            lfe_mode: LfeMode::default(),
            resample_quality: ResampleQuality::default(),
            persisted_stage_controls: HashMap::new(),
            recovery_attempts: 0,
            recovery_retry_at: None,
        }
    }

    pub(crate) fn reset_context(&mut self) {
        self.ctx = self.fresh_context();
    }

    pub(crate) fn fresh_context(&self) -> PipelineContext {
        PipelineContext::default()
    }

    pub(crate) fn set_lfe_mode(&mut self, mode: LfeMode) {
        self.lfe_mode = mode;
    }

    pub(crate) fn set_resample_quality(&mut self, quality: ResampleQuality) {
        self.resample_quality = quality;
    }
}
