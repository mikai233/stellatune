//! Runtime-control synchronization and transform-control routing helpers.
//!
//! # Why This Layer Exists
//!
//! Runtime controls are sourced from multiple places:
//! - actor commands (for explicit stage control),
//! - hot control snapshots (for gain and trim metadata),
//! - stage-local runtime updates during stepping.
//!
//! Centralizing this in the runner prevents drift between source/decoder/transform
//! state and sink-visible context. The control path is also where stage-key routing
//! is validated so dispatch can remain stable even if transform ordering changes.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
use stellatune_audio_core::pipeline::context::GainTransitionRequest;
use stellatune_audio_core::pipeline::context::{GaplessTrimSpec, PipelineContext};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;
use stellatune_audio_core::pipeline::stages::{
    StageControlResult, StageDispatchResult, StageTarget,
};

#[cfg(test)]
use crate::pipeline::runtime::dsp::control::TransitionGainControl;
use crate::pipeline::runtime::dsp::control::{GAPLESS_TRIM_STAGE_KEY, GaplessTrimControl};

use crate::pipeline::runtime::runner::PipelineRunner;
use crate::pipeline::runtime::sink_session::SinkSession;

impl PipelineRunner {
    /// Synchronizes runtime control across all active stages and updates gapless trim state.
    ///
    /// This is the control-plane checkpoint called from playback stepping. Any
    /// drift between decoder-provided trim metadata and transform trim state is
    /// reconciled here before new audio is produced.
    pub(crate) fn sync_runtime_control(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        self.source.sync_runtime_control(ctx)?;
        self.decoder.sync_runtime_control(ctx)?;
        let next_gapless_trim_spec =
            Self::normalize_gapless_trim_spec(self.decoder.current_gapless_trim_spec());
        if next_gapless_trim_spec != self.decoder_gapless_trim_spec {
            // Decoder trim metadata can change after seek/reopen; keep trim transform in sync.
            self.decoder_gapless_trim_spec = next_gapless_trim_spec;
            self.apply_gapless_trim_control(ctx)?;
        }
        for transform in &mut self.transforms {
            transform.sync_runtime_control(ctx)?;
        }
        sink_session.sync_runtime_control(ctx)?;
        Ok(())
    }

    pub(crate) fn apply_stage_control_to(
        &mut self,
        target: &StageTarget,
        control: Arc<dyn Any + Send + Sync>,
        sink_session: Option<&SinkSession>,
        ctx: &mut PipelineContext,
    ) -> Result<StageDispatchResult, PipelineError> {
        self.ensure_decode_prepared()?;
        self.apply_stage_control_internal(target, control, sink_session, ctx)
    }

    /// Applies typed control to a routed stage target with strict mismatch checks.
    ///
    /// The function is intentionally strict: once a target resolves to a stage,
    /// rejection of the payload is treated as a contract error instead of a soft no-op.
    fn apply_stage_control_internal(
        &mut self,
        target: &StageTarget,
        control: Arc<dyn Any + Send + Sync>,
        sink_session: Option<&SinkSession>,
        ctx: &mut PipelineContext,
    ) -> Result<StageDispatchResult, PipelineError> {
        let handled = match target {
            StageTarget::Source => self.source.apply_control(control.as_ref(), ctx)?,
            StageTarget::Decoder => self.decoder.apply_control(control.as_ref(), ctx)?,
            StageTarget::Transform(_) => {
                let Some(target_index) = self.transform_control_routes.get(target).copied() else {
                    return Ok(StageDispatchResult::StageNotFound);
                };
                let transforms_len = self.transforms.len();
                let transform = self.transforms.get_mut(target_index).ok_or_else(|| {
                    PipelineError::StageFailure(format!(
                        "stage control target out of bounds: target={target}, index={target_index}, len={transforms_len}"
                    ))
                })?;
                transform.apply_control(control.as_ref(), ctx)?
            },
            StageTarget::Sink(stage_key) => {
                let Some(sink_session) = sink_session else {
                    return Ok(StageDispatchResult::StageNotFound);
                };
                return sink_session.apply_stage_control(stage_key, control);
            },
        };
        if handled == StageControlResult::Ignored {
            return Err(PipelineError::StageFailure(format!(
                "stage control target rejected control: target={target}"
            )));
        }
        #[cfg(test)]
        if let Some(control) = control.as_ref().downcast_ref::<TransitionGainControl>()
            && let Some(sink) = self.transition_request_log_sink.as_ref()
        {
            sink.lock()
                .expect("transition request log sink mutex poisoned")
                .push(control.request);
        }
        Ok(StageDispatchResult::Applied)
    }

    fn scale_decoder_frames_to_output_domain(&self, frames: u64) -> u64 {
        let Some(decoder_spec) = self.decoder_spec else {
            return frames;
        };
        let Some(output_spec) = self.output_spec else {
            return frames;
        };
        let decoder_rate = decoder_spec.sample_rate.max(1) as u128;
        let output_rate = output_spec.sample_rate.max(1) as u128;
        if decoder_rate == output_rate {
            return frames;
        }
        let scaled = (frames as u128).saturating_mul(output_rate) / decoder_rate;
        scaled.min(u64::MAX as u128) as u64
    }

    /// Refreshes playable-remaining hint in output-rate domain, including gapless tail trimming.
    ///
    /// The hint is approximate and is used for policy decisions and transition timing,
    /// not as an exact playback position source.
    pub(crate) fn refresh_playable_remaining_frames_hint(&mut self) {
        let tail_frames = if self.supports_gapless_trim() {
            self.decoder_gapless_trim_spec
                .map(|v| v.tail_frames as u64)
                .unwrap_or(0)
        } else {
            0
        };
        let hint = self.decoder.estimated_remaining_frames().map(|frames| {
            let playable_decoder_frames = frames.saturating_sub(tail_frames);
            self.scale_decoder_frames_to_output_domain(playable_decoder_frames)
        });
        self.playable_remaining_frames_hint = hint;
    }

    pub(crate) fn apply_gapless_trim_control(
        &mut self,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        if !self.supports_gapless_trim {
            return Ok(());
        }
        let control = GaplessTrimControl::new(self.decoder_gapless_trim_spec, ctx.position_ms);
        let _ = self.apply_stage_control_internal(
            &StageTarget::transform(GAPLESS_TRIM_STAGE_KEY),
            Arc::new(control),
            None,
            ctx,
        )?;
        Ok(())
    }

    pub(crate) fn normalize_gapless_trim_spec(
        spec: Option<GaplessTrimSpec>,
    ) -> Option<GaplessTrimSpec> {
        spec.filter(|v| !v.is_disabled())
    }

    /// Builds and validates stage-key routes used by typed transform controls.
    ///
    /// Route construction is done once during runner creation to keep control
    /// dispatch O(1) during playback.
    pub(crate) fn build_transform_control_routes(
        transforms: &[Box<dyn TransformStage>],
    ) -> Result<HashMap<StageTarget, usize>, PipelineError> {
        let mut routes = HashMap::new();
        for (index, transform) in transforms.iter().enumerate() {
            let key = transform.key().trim();
            if key.is_empty() {
                return Err(PipelineError::StageFailure(
                    "transform stage key must not be empty".to_string(),
                ));
            }
            // Reject collisions early so control dispatch never becomes ambiguous.
            if routes.insert(StageTarget::transform(key), index).is_some() {
                return Err(PipelineError::StageFailure(format!(
                    "duplicate transform stage key: {key}"
                )));
            }
        }
        Ok(routes)
    }

    #[cfg(test)]
    pub(crate) fn set_transition_request_log_sink(
        &mut self,
        sink: Arc<Mutex<Vec<GainTransitionRequest>>>,
    ) {
        self.transition_request_log_sink = Some(sink);
    }
}
