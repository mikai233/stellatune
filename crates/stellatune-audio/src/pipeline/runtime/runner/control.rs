use std::any::Any;
use std::collections::HashMap;
#[cfg(test)]
use std::sync::{Arc, Mutex};

#[cfg(test)]
use stellatune_audio_core::pipeline::context::GainTransitionRequest;
use stellatune_audio_core::pipeline::context::{GaplessTrimSpec, PipelineContext};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

#[cfg(test)]
use crate::pipeline::runtime::dsp::control::TransitionGainControl;
use crate::pipeline::runtime::dsp::control::{GAPLESS_TRIM_STAGE_KEY, GaplessTrimControl};

use crate::pipeline::runtime::runner::PipelineRunner;
use crate::pipeline::runtime::sink_session::SinkSession;

impl PipelineRunner {
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
            self.decoder_gapless_trim_spec = next_gapless_trim_spec;
            self.apply_gapless_trim_control(ctx)?;
        }
        for transform in &mut self.transforms {
            transform.sync_runtime_control(ctx)?;
        }
        sink_session.sync_runtime_control(ctx)?;
        Ok(())
    }

    pub(crate) fn apply_transform_control_to(
        &mut self,
        stage_key: &str,
        control: &dyn Any,
        ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        self.ensure_decode_prepared()?;
        self.apply_transform_control_internal(stage_key, control, ctx)
    }

    fn apply_transform_control_internal(
        &mut self,
        stage_key: &str,
        control: &dyn Any,
        ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        let Some(target_index) = self.transform_control_routes.get(stage_key).copied() else {
            return Ok(false);
        };
        let transforms_len = self.transforms.len();
        let transform = self.transforms.get_mut(target_index).ok_or_else(|| {
            PipelineError::StageFailure(format!(
                "transform control target out of bounds: key={stage_key}, index={target_index}, len={transforms_len}"
            ))
        })?;
        let handled = transform.apply_control(control, ctx)?;
        if !handled {
            return Err(PipelineError::StageFailure(format!(
                "transform control target rejected control: key={stage_key}, index={target_index}"
            )));
        }
        #[cfg(test)]
        if let Some(control) = control.downcast_ref::<TransitionGainControl>()
            && let Some(sink) = self.transition_request_log_sink.as_ref()
        {
            sink.lock()
                .expect("transition request log sink mutex poisoned")
                .push(control.request);
        }
        Ok(true)
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
        let _ = self.apply_transform_control_internal(GAPLESS_TRIM_STAGE_KEY, &control, ctx)?;
        Ok(())
    }

    pub(crate) fn normalize_gapless_trim_spec(
        spec: Option<GaplessTrimSpec>,
    ) -> Option<GaplessTrimSpec> {
        spec.filter(|v| !v.is_disabled())
    }

    pub(crate) fn build_transform_control_routes(
        transforms: &[Box<dyn TransformStage>],
    ) -> Result<HashMap<String, usize>, PipelineError> {
        let mut routes = HashMap::new();
        for (index, transform) in transforms.iter().enumerate() {
            if let Some(stage_key) = transform.stage_key() {
                let key = stage_key.trim();
                if key.is_empty() {
                    return Err(PipelineError::StageFailure(
                        "transform stage key must not be empty".to_string(),
                    ));
                }
                if routes.insert(key.to_string(), index).is_some() {
                    return Err(PipelineError::StageFailure(format!(
                        "duplicate transform stage key: {key}"
                    )));
                }
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
