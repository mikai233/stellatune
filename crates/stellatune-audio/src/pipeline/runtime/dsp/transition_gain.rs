use std::any::Any;

use stellatune_audio_core::pipeline::context::{
    AudioBlock, GainTransitionRequest, PipelineContext, StreamSpec, TransitionCurve,
    TransitionTimePolicy,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::pipeline::runtime::dsp::control::TransitionGainControl;

#[derive(Debug)]
pub(crate) struct TransitionGainStage {
    channels: usize,
    sample_rate: u32,
    current_gain: f32,
    transition_from: f32,
    transition_to: f32,
    transition_total_frames: usize,
    transition_remaining_frames: usize,
    curve: TransitionCurve,
}

impl Default for TransitionGainStage {
    fn default() -> Self {
        Self {
            channels: 1,
            sample_rate: 1,
            current_gain: 1.0,
            transition_from: 1.0,
            transition_to: 1.0,
            transition_total_frames: 0,
            transition_remaining_frames: 0,
            curve: TransitionCurve::EqualPower,
        }
    }
}

impl TransitionGainStage {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn configure_transition(&mut self, request: GainTransitionRequest) {
        let target_gain = request.target_gain.clamp(0.0, 1.0);
        if request.ramp_ms == 0 {
            self.current_gain = target_gain;
            self.transition_from = target_gain;
            self.transition_to = target_gain;
            self.transition_total_frames = 0;
            self.transition_remaining_frames = 0;
            return;
        }
        let nominal_frames =
            ((self.sample_rate as u64 * request.ramp_ms as u64).div_ceil(1000)) as usize;
        let nominal_frames = nominal_frames.max(1);
        let effective_frames = match request.time_policy {
            TransitionTimePolicy::Exact => nominal_frames,
            TransitionTimePolicy::FitToAvailable => request
                .available_frames_hint
                .map(|frames| (frames as usize).min(nominal_frames))
                .unwrap_or(nominal_frames),
        };
        if effective_frames == 0 {
            self.current_gain = target_gain;
            self.transition_from = target_gain;
            self.transition_to = target_gain;
            self.transition_total_frames = 0;
            self.transition_remaining_frames = 0;
            return;
        }

        self.transition_from = self.current_gain;
        self.transition_to = target_gain;
        self.transition_total_frames = effective_frames;
        self.transition_remaining_frames = effective_frames;
        self.curve = request.curve;
    }

    fn interpolate_gain(&self, from: f32, to: f32, progress: f32) -> f32 {
        let from = from.clamp(0.0, 1.0);
        let to = to.clamp(0.0, 1.0);
        let progress = progress.clamp(0.0, 1.0);
        match self.curve {
            TransitionCurve::Linear => from + (to - from) * progress,
            TransitionCurve::EqualPower => {
                let from_power = from * from;
                let to_power = to * to;
                let power = from_power + (to_power - from_power) * progress;
                power.max(0.0).sqrt().clamp(0.0, 1.0)
            },
        }
    }

    fn next_frame_gain(&mut self) -> f32 {
        if self.transition_remaining_frames == 0 || self.transition_total_frames == 0 {
            self.current_gain = self.transition_to;
            return self.current_gain;
        }
        let progressed = self
            .transition_total_frames
            .saturating_sub(self.transition_remaining_frames)
            .saturating_add(1);
        let progress = progressed as f32 / self.transition_total_frames as f32;
        let gain = self.interpolate_gain(self.transition_from, self.transition_to, progress);
        self.transition_remaining_frames = self.transition_remaining_frames.saturating_sub(1);
        if self.transition_remaining_frames == 0 {
            self.current_gain = self.transition_to;
        } else {
            self.current_gain = gain;
        }
        self.current_gain
    }

    fn apply_in_place(&mut self, block: &mut AudioBlock) {
        if block.is_empty() {
            return;
        }
        let channels = self.channels.max(1);
        let frames = block.samples.len() / channels;
        for frame in 0..frames {
            let gain = self.next_frame_gain();
            let base = frame * channels;
            for ch in 0..channels {
                block.samples[base + ch] *= gain;
            }
        }
    }
}

impl TransformStage for TransitionGainStage {
    fn stage_key(&self) -> Option<&str> {
        Some(crate::pipeline::runtime::dsp::control::TRANSITION_GAIN_STAGE_KEY)
    }

    fn apply_control(
        &mut self,
        control: &dyn Any,
        _ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        if let Some(control) = control.downcast_ref::<TransitionGainControl>() {
            self.configure_transition(control.request);
            return Ok(true);
        }
        Ok(false)
    }

    fn prepare(
        &mut self,
        spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.channels = spec.channels.max(1) as usize;
        self.sample_rate = spec.sample_rate.max(1);
        self.current_gain = 1.0;
        self.transition_from = 1.0;
        self.transition_to = 1.0;
        self.transition_total_frames = 0;
        self.transition_remaining_frames = 0;
        Ok(spec)
    }

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        let _ = ctx;
        Ok(())
    }

    fn process(&mut self, block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        self.apply_in_place(block);
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.transition_from = self.current_gain;
        self.transition_to = self.current_gain;
        self.transition_total_frames = 0;
        self.transition_remaining_frames = 0;
    }
}

#[cfg(test)]
mod tests {
    use crate::pipeline::runtime::dsp::control::TransitionGainControl;
    use crate::pipeline::runtime::dsp::transition_gain::TransitionGainStage;
    use stellatune_audio_core::pipeline::context::{
        AudioBlock, GainTransitionRequest, PipelineContext, StreamSpec, TransitionCurve,
        TransitionTimePolicy,
    };
    use stellatune_audio_core::pipeline::stages::StageStatus;
    use stellatune_audio_core::pipeline::stages::transform::TransformStage;

    fn mono_block(samples: &[f32]) -> AudioBlock {
        AudioBlock {
            channels: 1,
            samples: samples.to_vec(),
        }
    }

    #[test]
    fn fade_progress_continues_across_blocks() {
        let mut stage = TransitionGainStage::new();
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
            .apply_control(
                &TransitionGainControl::new(GainTransitionRequest {
                    target_gain: 0.0,
                    ramp_ms: 4,
                    available_frames_hint: None,
                    curve: TransitionCurve::Linear,
                    time_policy: TransitionTimePolicy::Exact,
                }),
                &mut ctx,
            )
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

    #[test]
    fn fit_to_available_shortens_fade_when_frames_are_insufficient() {
        let mut stage = TransitionGainStage::new();
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
            .apply_control(
                &TransitionGainControl::new(GainTransitionRequest {
                    target_gain: 0.0,
                    ramp_ms: 100,
                    available_frames_hint: Some(2),
                    curve: TransitionCurve::EqualPower,
                    time_policy: TransitionTimePolicy::FitToAvailable,
                }),
                &mut ctx,
            )
            .expect("apply_control failed");
        stage
            .sync_runtime_control(&mut ctx)
            .expect("sync_runtime_control failed");

        let mut block = mono_block(&[1.0, 1.0, 1.0, 1.0]);
        assert_eq!(stage.process(&mut block, &mut ctx), StageStatus::Ok);
        assert!((block.samples[0] - 0.70710677).abs() < 1e-5);
        assert!((block.samples[1] - 0.0).abs() < 1e-6);
        assert!((block.samples[2] - 0.0).abs() < 1e-6);
        assert!((block.samples[3] - 0.0).abs() < 1e-6);
    }
}
