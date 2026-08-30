use std::time::Duration;
use std::time::Instant;

use stellatune_audio_core::pipeline::context::{InputRef, PipelineContext};

use crate::config::engine::{LfeMode, ResampleQuality};
use crate::config::gain::GainTransitionConfig;
use crate::config::sink::{SinkLatencyConfig, SinkRecoveryConfig};
use crate::error::NoActivePipelineReason;
use crate::pipeline::runtime::dsp::control::SharedMasterGainHotControl;
use crate::pipeline::runtime::runner::PipelineRunner;
use crate::pipeline::runtime::sink_session::SinkSession;

pub(crate) struct PrewarmedNext {
    pub(crate) input: InputRef,
    pub(crate) runner: PipelineRunner,
    pub(crate) ctx: PipelineContext,
}

pub(crate) struct DecodeWorkerState {
    pub(crate) runner: Option<PipelineRunner>,
    pub(crate) ctx: PipelineContext,
    pub(crate) master_gain_hot_control: SharedMasterGainHotControl,
    /// Whether bounded actor turns should continue advancing audio.
    pub(crate) pumping: bool,
    pub(crate) active_input: Option<InputRef>,
    pub(crate) queued_next_input: Option<InputRef>,
    pub(crate) prewarmed_next: Option<PrewarmedNext>,
    pub(crate) last_position_emit_at: Instant,
    pub(crate) sink_recovery: SinkRecoveryConfig,
    pub(crate) gain_transition: GainTransitionConfig,
    pub(crate) sink_session: SinkSession,
    pub(crate) lfe_mode: LfeMode,
    pub(crate) resample_quality: ResampleQuality,
    pub(crate) recovery_attempts: u32,
    pub(crate) recovery_retry_at: Option<Instant>,
    pub(crate) audio_start_sent: bool,
    pub(crate) pipeline_unavailable_reason: Option<NoActivePipelineReason>,
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
            pumping: false,
            active_input: None,
            queued_next_input: None,
            prewarmed_next: None,
            last_position_emit_at: Instant::now(),
            sink_recovery,
            gain_transition,
            sink_session: SinkSession::new(sink_latency, sink_control_timeout),
            lfe_mode: LfeMode::default(),
            resample_quality: ResampleQuality::default(),
            recovery_attempts: 0,
            recovery_retry_at: None,
            audio_start_sent: false,
            pipeline_unavailable_reason: None,
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

    pub(crate) fn clear_pipeline_unavailable_reason(&mut self) {
        self.pipeline_unavailable_reason = None;
    }

    pub(crate) fn set_pipeline_unavailable_reason(&mut self, reason: NoActivePipelineReason) {
        self.pipeline_unavailable_reason = Some(reason);
    }

    pub(crate) fn current_no_active_pipeline_reason(&self) -> NoActivePipelineReason {
        if self.active_input.is_none() {
            return NoActivePipelineReason::NoTrackLoaded;
        }
        self.pipeline_unavailable_reason
            .clone()
            .unwrap_or(NoActivePipelineReason::RunnerMissing)
    }
}
