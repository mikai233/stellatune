//! Typed runtime controls for the active native pipeline.

use std::collections::HashMap;

use stellatune_audio_core::pipeline::context::{
    GainTransitionRequest, GaplessTrimSpec, MasterGainCurve, PipelineContext,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::pipeline::runtime::dsp::control::{
    GAPLESS_TRIM_STAGE_KEY, MASTER_GAIN_STAGE_KEY, TRANSITION_GAIN_STAGE_KEY,
};
use crate::pipeline::runtime::runner::PipelineRunner;
use crate::pipeline::runtime::sink_session::SinkSession;

impl PipelineRunner {
    pub(crate) fn refresh_runtime_state(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        self.source.refresh_runtime_state(ctx)?;
        self.decoder.refresh_runtime_state(ctx)?;
        let next_gapless_trim_spec =
            Self::normalize_gapless_trim_spec(self.decoder.current_gapless_trim_spec());
        if next_gapless_trim_spec != self.decoder_gapless_trim_spec {
            self.decoder_gapless_trim_spec = next_gapless_trim_spec;
            self.apply_gapless_trim_control(ctx)?;
        }
        for transform in &mut self.transforms {
            transform.refresh_runtime_state(ctx)?;
        }
        sink_session.refresh_runtime_state(ctx)?;
        Ok(())
    }

    pub(crate) fn set_master_gain(
        &mut self,
        level: f32,
        ramp_ms: u32,
        curve: Option<MasterGainCurve>,
    ) -> Result<(), PipelineError> {
        let transform = self.transform_by_key_mut(MASTER_GAIN_STAGE_KEY)?;
        if !transform.set_master_gain(level, ramp_ms, curve)? {
            return Err(PipelineError::StageFailure(
                "master gain stage rejected typed control".to_string(),
            ));
        }
        Ok(())
    }

    pub(crate) fn set_transition_gain(
        &mut self,
        request: GainTransitionRequest,
    ) -> Result<(), PipelineError> {
        let transform = self.transform_by_key_mut(TRANSITION_GAIN_STAGE_KEY)?;
        if !transform.set_transition_gain(request)? {
            return Err(PipelineError::StageFailure(
                "transition gain stage rejected typed control".to_string(),
            ));
        }
        Ok(())
    }

    pub(crate) fn apply_gapless_trim_control(
        &mut self,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        if !self.supports_gapless_trim {
            return Ok(());
        }
        let spec = self.decoder_gapless_trim_spec;
        let position_ms = ctx.position_ms.max(0);
        let transform = self.transform_by_key_mut(GAPLESS_TRIM_STAGE_KEY)?;
        if !transform.set_gapless_trim(spec, position_ms)? {
            return Err(PipelineError::StageFailure(
                "gapless trim stage rejected typed control".to_string(),
            ));
        }
        Ok(())
    }

    fn transform_by_key_mut(
        &mut self,
        key: &str,
    ) -> Result<&mut Box<dyn TransformStage>, PipelineError> {
        self.ensure_decode_prepared()?;
        let index = self
            .transform_control_routes
            .get(key)
            .copied()
            .ok_or_else(|| {
                PipelineError::StageFailure(format!("required transform stage is missing: {key}"))
            })?;
        let transforms_len = self.transforms.len();
        self.transforms.get_mut(index).ok_or_else(|| {
            PipelineError::StageFailure(format!(
                "transform route out of bounds: key={key}, index={index}, len={transforms_len}"
            ))
        })
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
        self.playable_remaining_frames_hint = self
            .decoder
            .estimated_remaining_frames()
            .map(|frames| self.scale_decoder_frames_to_output_domain(frames));
    }

    pub(crate) fn normalize_gapless_trim_spec(
        spec: Option<GaplessTrimSpec>,
    ) -> Option<GaplessTrimSpec> {
        spec.filter(|value| !value.is_disabled())
    }

    pub(crate) fn build_transform_control_routes(
        transforms: &[Box<dyn TransformStage>],
    ) -> Result<HashMap<String, usize>, PipelineError> {
        let mut routes = HashMap::new();
        for (index, transform) in transforms.iter().enumerate() {
            let key = transform.key().trim();
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
        Ok(routes)
    }
}
