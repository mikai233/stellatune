use std::any::Any;

use stellatune_audio_core::pipeline::context::{
    AudioBlock, MasterGainCurve, PipelineContext, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::runtime::transform::control::MasterGainControl;

#[derive(Debug)]
pub(crate) struct MasterGainStage {
    level: f32,
    gain: f32,
    curve: MasterGainCurve,
}

impl Default for MasterGainStage {
    fn default() -> Self {
        Self {
            level: 1.0,
            gain: 1.0,
            curve: MasterGainCurve::AudioTaper,
        }
    }
}

impl MasterGainStage {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl TransformStage for MasterGainStage {
    fn stage_key(&self) -> Option<&str> {
        Some(crate::runtime::transform::control::MASTER_GAIN_STAGE_KEY)
    }

    fn apply_control(
        &mut self,
        control: &dyn Any,
        _ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        if let Some(control) = control.downcast_ref::<MasterGainControl>() {
            if let Some(curve) = control.curve {
                self.curve = curve;
            }
            self.level = control.level.clamp(0.0, 1.0);
            self.gain = self.curve.level_to_gain(self.level).clamp(0.0, 1.0);
            return Ok(true);
        }
        Ok(false)
    }

    fn prepare(
        &mut self,
        spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.level = self.level.clamp(0.0, 1.0);
        self.gain = self.curve.level_to_gain(self.level).clamp(0.0, 1.0);
        Ok(spec)
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn process(&mut self, block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        if block.is_empty() {
            return StageStatus::Ok;
        }
        if self.gain <= 0.0 {
            block.samples.fill(0.0);
            return StageStatus::Ok;
        }
        if (self.gain - 1.0).abs() < f32::EPSILON {
            return StageStatus::Ok;
        }
        for sample in &mut block.samples {
            *sample *= self.gain;
        }
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono_block(samples: &[f32]) -> AudioBlock {
        AudioBlock {
            channels: 1,
            samples: samples.to_vec(),
        }
    }

    #[test]
    fn applies_audio_taper_gain_from_requested_level() {
        let mut stage = MasterGainStage::new();
        let mut ctx = PipelineContext::default();
        stage
            .prepare(
                StreamSpec {
                    sample_rate: 48_000,
                    channels: 1,
                },
                &mut ctx,
            )
            .expect("prepare failed");

        stage
            .apply_control(&MasterGainControl::new(0.5), &mut ctx)
            .expect("apply_control failed");
        stage
            .sync_runtime_control(&mut ctx)
            .expect("sync_runtime_control failed");

        let mut block = mono_block(&[1.0]);
        assert_eq!(stage.process(&mut block, &mut ctx), StageStatus::Ok);
        assert!((block.samples[0] - 0.17782794).abs() < 1e-6);
    }

    #[test]
    fn supports_linear_curve_when_requested() {
        let mut stage = MasterGainStage::new();
        let mut ctx = PipelineContext::default();
        stage
            .prepare(
                StreamSpec {
                    sample_rate: 48_000,
                    channels: 1,
                },
                &mut ctx,
            )
            .expect("prepare failed");

        stage
            .apply_control(
                &MasterGainControl::with_curve(0.5, MasterGainCurve::Linear),
                &mut ctx,
            )
            .expect("apply_control failed");
        stage
            .sync_runtime_control(&mut ctx)
            .expect("sync_runtime_control failed");

        let mut block = mono_block(&[1.0, 0.5]);
        assert_eq!(stage.process(&mut block, &mut ctx), StageStatus::Ok);
        assert!((block.samples[0] - 0.5).abs() < 1e-6);
        assert!((block.samples[1] - 0.25).abs() < 1e-6);
    }
}
