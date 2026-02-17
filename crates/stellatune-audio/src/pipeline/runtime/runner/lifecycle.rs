//! Runner lifecycle transitions and preparation guards.

use stellatune_audio_core::pipeline::context::{InputRef, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::config::engine::{PauseBehavior, StopBehavior};
use crate::pipeline::assembly::SinkPlan;
use crate::pipeline::runtime::runner::{PipelineRunner, RunnerState};
use crate::pipeline::runtime::sink_session::{SinkActivationMode, SinkSession};

impl PipelineRunner {
    #[allow(dead_code)]
    pub(crate) fn new(
        source: Box<dyn SourceStage>,
        decoder: Box<dyn DecoderStage>,
        transforms: Vec<Box<dyn TransformStage>>,
        sink_plan: Box<dyn SinkPlan>,
        supports_transition_gain: bool,
        supports_gapless_trim: bool,
    ) -> Result<Self, PipelineError> {
        let sink_route_fingerprint = sink_plan.route_fingerprint();
        let transform_control_routes = Self::build_transform_control_routes(&transforms)?;
        Ok(Self {
            source,
            decoder,
            transforms,
            supports_transition_gain,
            supports_gapless_trim,
            sink_plan: Some(sink_plan),
            sink_route_fingerprint,
            pending_sink_block: None,
            source_handle: None,
            decoder_spec: None,
            output_spec: None,
            decoder_gapless_trim_spec: None,
            playable_remaining_frames_hint: None,
            transform_control_routes,
            #[cfg(test)]
            transition_request_log_sink: None,
            state: RunnerState::Stopped,
        })
    }

    /// Prepares decode/transform stages and caches the output stream spec used by sink activation.
    pub(crate) fn prepare_decode(
        &mut self,
        input: &InputRef,
        ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        if self.source_handle.is_some() || self.output_spec.is_some() {
            return Err(PipelineError::StageFailure(
                "decode already prepared".to_string(),
            ));
        }
        let source_handle = self.source.prepare(input, ctx)?;
        let decoder_spec = self.decoder.prepare(&source_handle, ctx)?.validate()?;
        let decoder_gapless_trim_spec =
            Self::normalize_gapless_trim_spec(self.decoder.current_gapless_trim_spec());
        let mut spec = decoder_spec;
        for transform in &mut self.transforms {
            spec = transform.prepare(spec, ctx)?.validate()?;
        }

        self.source_handle = Some(source_handle);
        self.decoder_spec = Some(decoder_spec);
        self.output_spec = Some(spec);
        self.decoder_gapless_trim_spec = decoder_gapless_trim_spec;
        self.pending_sink_block = None;
        self.apply_gapless_trim_control(ctx)?;
        self.refresh_playable_remaining_frames_hint();
        Ok(spec)
    }

    /// Activates sink output for the prepared route and reports whether sink state was reused.
    pub(crate) fn activate_sink(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &PipelineContext,
        mode: SinkActivationMode,
    ) -> Result<bool, PipelineError> {
        self.ensure_decode_prepared()?;
        let spec = self.output_spec.ok_or(PipelineError::NotPrepared)?;
        let reused = sink_session.activate(
            spec,
            self.sink_route_fingerprint,
            &mut self.sink_plan,
            ctx,
            mode,
        )?;
        self.pending_sink_block = None;
        Ok(reused)
    }

    pub(crate) fn drain_sink_for_reuse(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        self.drain(sink_session, ctx)
    }

    pub(crate) fn set_state(&mut self, state: RunnerState) {
        self.state = state;
    }

    pub(crate) fn state(&self) -> RunnerState {
        self.state
    }

    pub(crate) fn has_pending_sink_block(&self) -> bool {
        self.pending_sink_block.is_some()
    }

    pub(crate) fn supports_transition_gain(&self) -> bool {
        self.supports_transition_gain
    }

    pub(crate) fn supports_gapless_trim(&self) -> bool {
        self.supports_gapless_trim
    }

    pub(crate) fn playable_remaining_frames_hint(&self) -> Option<u64> {
        self.playable_remaining_frames_hint
    }

    pub(crate) fn output_sample_rate(&self) -> Option<u32> {
        self.output_spec.map(|spec| spec.sample_rate)
    }

    pub(crate) fn pause(
        &mut self,
        behavior: PauseBehavior,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        if matches!(behavior, PauseBehavior::DrainSink) {
            self.drain(sink_session, ctx)?;
        }
        self.state = RunnerState::Paused;
        Ok(())
    }

    /// Seeks via context handoff after dropping queued sink blocks to avoid stale audio output.
    pub(crate) fn seek(
        &mut self,
        position_ms: i64,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        self.pending_sink_block = None;
        sink_session.drop_queued()?;
        ctx.request_seek(position_ms);
        self.refresh_playable_remaining_frames_hint();
        Ok(())
    }

    pub(crate) fn stop(&mut self, sink_session: &mut SinkSession, ctx: &mut PipelineContext) {
        sink_session.shutdown(false);
        self.stop_decode_only(ctx);
    }

    pub(crate) fn stop_decode_only(&mut self, ctx: &mut PipelineContext) {
        self.pending_sink_block = None;
        self.playable_remaining_frames_hint = None;
        self.decoder_gapless_trim_spec = None;
        for transform in &mut self.transforms {
            transform.stop(ctx);
        }
        self.decoder.stop(ctx);
        self.source.stop(ctx);

        self.source_handle = None;
        self.decoder_spec = None;
        self.output_spec = None;
        self.state = RunnerState::Stopped;
    }

    /// Stops playback with optional sink draining while keeping error semantics uniform.
    pub(crate) fn stop_with_behavior(
        &mut self,
        behavior: StopBehavior,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        if matches!(behavior, StopBehavior::DrainSink) && self.is_decode_prepared() {
            self.drain(sink_session, ctx)?;
        }
        self.stop(sink_session, ctx);
        Ok(())
    }

    /// Ensures decode is prepared and sink session still matches the active route fingerprint.
    pub(crate) fn ensure_sink_prepared(
        &self,
        sink_session: &SinkSession,
    ) -> Result<(), PipelineError> {
        let output_spec = self.output_spec.ok_or(PipelineError::NotPrepared)?;
        if !self.is_decode_prepared()
            || !sink_session.is_active_for(output_spec, self.sink_route_fingerprint)
        {
            return Err(PipelineError::NotPrepared);
        }
        Ok(())
    }

    pub(crate) fn ensure_decode_prepared(&self) -> Result<(), PipelineError> {
        if !self.is_decode_prepared() {
            return Err(PipelineError::NotPrepared);
        }
        Ok(())
    }

    pub(crate) fn is_decode_prepared(&self) -> bool {
        self.source_handle.is_some() && self.output_spec.is_some()
    }
}
