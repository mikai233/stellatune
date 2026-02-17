use std::any::Any;

use stellatune_audio_core::pipeline::context::{
    AudioBlock, MasterGainCurve, PipelineContext, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::runtime::transform::control::{MasterGainControl, SharedMasterGainHotControl};

#[derive(Debug)]
pub(crate) struct MasterGainStage {
    level: f32,
    sample_rate: u32,
    channels: usize,
    current_gain: f32,
    target_gain: f32,
    ramp_remaining_frames: usize,
    curve: MasterGainCurve,
    hot_control: Option<SharedMasterGainHotControl>,
    last_seen_hot_version: u64,
}

impl Default for MasterGainStage {
    fn default() -> Self {
        Self {
            level: 1.0,
            sample_rate: 1,
            channels: 1,
            current_gain: 1.0,
            target_gain: 1.0,
            ramp_remaining_frames: 0,
            curve: MasterGainCurve::AudioTaper,
            hot_control: None,
            last_seen_hot_version: 0,
        }
    }
}

impl MasterGainStage {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_hot_control(hot_control: SharedMasterGainHotControl) -> Self {
        let mut stage = Self::new();
        stage.hot_control = Some(hot_control);
        stage
    }

    fn apply_target_gain(&mut self, target_gain: f32, ramp_ms: u32) {
        let target_gain = target_gain.clamp(0.0, 1.0);
        if ramp_ms == 0 || (self.current_gain - target_gain).abs() <= f32::EPSILON {
            self.current_gain = target_gain;
            self.target_gain = target_gain;
            self.ramp_remaining_frames = 0;
            return;
        }

        let frames = ((self.sample_rate as u64 * ramp_ms as u64).div_ceil(1000)).max(1) as usize;
        self.target_gain = target_gain;
        self.ramp_remaining_frames = frames;
    }

    fn next_frame_gain(&mut self) -> f32 {
        if self.ramp_remaining_frames == 0 {
            self.current_gain = self.target_gain;
            return self.current_gain;
        }

        let remaining = self.ramp_remaining_frames as f32;
        self.current_gain += (self.target_gain - self.current_gain) / remaining;
        self.ramp_remaining_frames = self.ramp_remaining_frames.saturating_sub(1);
        if self.ramp_remaining_frames == 0 {
            self.current_gain = self.target_gain;
        }
        self.current_gain.clamp(0.0, 1.0)
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
            let target_gain = self.curve.level_to_gain(self.level).clamp(0.0, 1.0);
            self.apply_target_gain(target_gain, control.ramp_ms);
            return Ok(true);
        }
        Ok(false)
    }

    fn prepare(
        &mut self,
        spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.sample_rate = spec.sample_rate.max(1);
        self.channels = spec.channels.max(1) as usize;
        self.level = self.level.clamp(0.0, 1.0);
        let gain = self.curve.level_to_gain(self.level).clamp(0.0, 1.0);
        self.current_gain = gain;
        self.target_gain = gain;
        self.ramp_remaining_frames = 0;
        Ok(spec)
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        let Some(hot_control) = self.hot_control.as_ref() else {
            return Ok(());
        };
        let version = hot_control.version();
        if version == self.last_seen_hot_version {
            return Ok(());
        }
        let state = hot_control.snapshot();
        if let Some(curve) = state.curve {
            self.curve = curve;
        }
        self.level = state.level.clamp(0.0, 1.0);
        let target_gain = self.curve.level_to_gain(self.level).clamp(0.0, 1.0);
        self.apply_target_gain(target_gain, state.ramp_ms);
        self.last_seen_hot_version = version;
        Ok(())
    }

    fn process(&mut self, block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        if block.is_empty() {
            return StageStatus::Ok;
        }
        if self.ramp_remaining_frames == 0 && self.current_gain <= 0.0 {
            block.samples.fill(0.0);
            return StageStatus::Ok;
        }
        if self.ramp_remaining_frames == 0 && (self.current_gain - 1.0).abs() < f32::EPSILON {
            return StageStatus::Ok;
        }

        let channels = usize::from(block.channels.max(1));
        let frames = block.samples.len() / channels;
        for frame in 0..frames {
            let gain = self.next_frame_gain();
            let base = frame * channels;
            for ch in 0..channels {
                block.samples[base + ch] *= gain;
            }
        }
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.current_gain = self.target_gain;
        self.ramp_remaining_frames = 0;
    }
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
            .apply_control(&MasterGainControl::new(0.5, 0), &mut ctx)
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
                &MasterGainControl::with_curve(0.5, 0, MasterGainCurve::Linear),
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

    #[test]
    fn ramps_gain_over_multiple_frames() {
        let mut stage = MasterGainStage::new();
        let mut ctx = PipelineContext::default();
        stage
            .prepare(
                StreamSpec {
                    sample_rate: 1000,
                    channels: 1,
                },
                &mut ctx,
            )
            .expect("prepare failed");

        stage
            .apply_control(&MasterGainControl::new(0.0, 4), &mut ctx)
            .expect("apply_control failed");
        stage
            .sync_runtime_control(&mut ctx)
            .expect("sync_runtime_control failed");

        let mut first = mono_block(&[1.0, 1.0]);
        assert_eq!(stage.process(&mut first, &mut ctx), StageStatus::Ok);
        assert!((first.samples[0] - 0.75).abs() < 1e-6);
        assert!((first.samples[1] - 0.5).abs() < 1e-6);

        let mut second = mono_block(&[1.0, 1.0]);
        assert_eq!(stage.process(&mut second, &mut ctx), StageStatus::Ok);
        assert!((second.samples[0] - 0.25).abs() < 1e-6);
        assert!((second.samples[1] - 0.0).abs() < 1e-6);
    }
}
