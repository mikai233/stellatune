use std::collections::VecDeque;

use stellatune_audio_core::pipeline::context::{
    AudioBlock, GaplessTrimSpec, PipelineContext, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::pipeline::runtime::dsp::control::GaplessTrimControl;

const GAPLESS_ENTRY_DECLICK_MS: usize = 2;

#[derive(Debug, Default)]
pub(crate) struct GaplessTrimStage {
    spec: Option<GaplessTrimSpec>,
    channels: usize,
    sample_rate: u32,
    initial_head_samples: usize,
    head_samples_remaining: usize,
    tail_hold_samples: usize,
    tail_buffer: VecDeque<f32>,
    pending_output: VecDeque<f32>,
    entry_ramp_total_frames: usize,
    entry_ramp_applied_frames: usize,
    entry_ramp_active: bool,
}

impl GaplessTrimStage {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn normalized_spec(spec: Option<GaplessTrimSpec>) -> Option<GaplessTrimSpec> {
        spec.filter(|v| !v.is_disabled())
    }

    fn configure(
        &mut self,
        spec: StreamSpec,
        gapless_spec: Option<GaplessTrimSpec>,
        position_ms: i64,
    ) {
        self.channels = spec.channels.max(1) as usize;
        self.sample_rate = spec.sample_rate.max(1);
        self.spec = Self::normalized_spec(gapless_spec);
        self.initial_head_samples = self
            .spec
            .map(|v| (v.head_frames as usize).saturating_mul(self.channels))
            .unwrap_or(0);
        self.tail_hold_samples = self
            .spec
            .map(|v| (v.tail_frames as usize).saturating_mul(self.channels))
            .unwrap_or(0);
        self.entry_ramp_total_frames = ((self.sample_rate as usize) * GAPLESS_ENTRY_DECLICK_MS)
            .saturating_div(1000)
            .max(1);
        self.reset_for_seek(position_ms);
    }

    fn reset_for_seek(&mut self, position_ms: i64) {
        self.pending_output.clear();
        self.tail_buffer.clear();
        self.entry_ramp_applied_frames = 0;
        if position_ms <= 0 {
            self.head_samples_remaining = self.initial_head_samples;
            self.entry_ramp_active = self.initial_head_samples > 0;
        } else {
            self.head_samples_remaining = 0;
            self.entry_ramp_active = false;
        }
    }

    fn apply_entry_ramp_in_place(&mut self, samples: &mut [f32]) {
        if !self.entry_ramp_active || samples.is_empty() {
            return;
        }
        let channels = self.channels.max(1);
        let frames = samples.len() / channels;
        if frames == 0 {
            return;
        }
        let remaining = self
            .entry_ramp_total_frames
            .saturating_sub(self.entry_ramp_applied_frames);
        if remaining == 0 {
            self.entry_ramp_active = false;
            return;
        }
        let apply_frames = remaining.min(frames);
        for frame in 0..apply_frames {
            let progress_frame = self.entry_ramp_applied_frames + frame + 1;
            let t = (progress_frame as f32 / self.entry_ramp_total_frames as f32).clamp(0.0, 1.0);
            let gain = t.sqrt();
            let base = frame * channels;
            for ch in 0..channels {
                samples[base + ch] *= gain;
            }
        }
        self.entry_ramp_applied_frames =
            self.entry_ramp_applied_frames.saturating_add(apply_frames);
        if self.entry_ramp_applied_frames >= self.entry_ramp_total_frames {
            self.entry_ramp_active = false;
        }
    }

    fn push_decoded_samples(&mut self, mut samples: Vec<f32>) {
        if self.spec.is_none() {
            self.pending_output.extend(samples);
            return;
        }

        if self.head_samples_remaining > 0 {
            let trim = self.head_samples_remaining.min(samples.len());
            if trim == samples.len() {
                samples.clear();
            } else if trim > 0 {
                samples = samples.split_off(trim);
            }
            self.head_samples_remaining = self.head_samples_remaining.saturating_sub(trim);
        }
        if samples.is_empty() {
            return;
        }

        self.apply_entry_ramp_in_place(&mut samples);
        if self.tail_hold_samples == 0 {
            self.pending_output.extend(samples);
            return;
        }

        self.tail_buffer.extend(samples);
        let releasable = self
            .tail_buffer
            .len()
            .saturating_sub(self.tail_hold_samples);
        if releasable > 0 {
            self.pending_output
                .extend(self.tail_buffer.drain(..releasable));
        }
    }

    fn drain_pending_into_block(&mut self, block: &mut AudioBlock) {
        let take = self.pending_output.len();
        block.samples.clear();
        block.samples.extend(self.pending_output.drain(..take));
    }
}

impl TransformStage for GaplessTrimStage {
    fn stage_key(&self) -> Option<&str> {
        Some(crate::pipeline::runtime::dsp::control::GAPLESS_TRIM_STAGE_KEY)
    }

    fn apply_control(
        &mut self,
        control: &dyn std::any::Any,
        _ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        if let Some(control) = control.downcast_ref::<GaplessTrimControl>() {
            let spec = StreamSpec {
                sample_rate: self.sample_rate.max(1),
                channels: self.channels.max(1) as u16,
            };
            self.configure(spec, control.spec, control.position_ms);
            return Ok(true);
        }
        Ok(false)
    }

    fn prepare(
        &mut self,
        spec: StreamSpec,
        ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.configure(spec, self.spec, ctx.position_ms);
        Ok(spec)
    }

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        if let Some(seek_ms) = ctx.pending_seek_ms {
            self.reset_for_seek(seek_ms);
        }
        Ok(())
    }

    fn process(&mut self, block: &mut AudioBlock, ctx: &mut PipelineContext) -> StageStatus {
        let _ = ctx;
        if block.is_empty() {
            return StageStatus::Ok;
        }
        let incoming = std::mem::take(&mut block.samples);
        self.push_decoded_samples(incoming);
        self.drain_pending_into_block(block);
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        self.pending_output.clear();
        self.tail_buffer.clear();
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.pending_output.clear();
        self.tail_buffer.clear();
        self.head_samples_remaining = 0;
        self.entry_ramp_applied_frames = 0;
        self.entry_ramp_active = false;
    }
}

#[cfg(test)]
mod tests {
    use crate::pipeline::runtime::dsp::control::GaplessTrimControl;
    use crate::pipeline::runtime::dsp::gapless_trim::GaplessTrimStage;
    use stellatune_audio_core::pipeline::context::{
        AudioBlock, GaplessTrimSpec, PipelineContext, StreamSpec,
    };
    use stellatune_audio_core::pipeline::stages::StageStatus;
    use stellatune_audio_core::pipeline::stages::transform::TransformStage;

    fn block(samples: &[f32]) -> AudioBlock {
        AudioBlock {
            channels: 1,
            samples: samples.to_vec(),
        }
    }

    #[test]
    fn trims_head_and_holds_tail_samples() {
        let mut stage = GaplessTrimStage::new();
        let mut ctx = PipelineContext::default();
        let _ = stage
            .prepare(
                StreamSpec {
                    sample_rate: 1,
                    channels: 1,
                },
                &mut ctx,
            )
            .expect("prepare failed");
        stage
            .apply_control(
                &GaplessTrimControl::new(
                    Some(GaplessTrimSpec {
                        head_frames: 2,
                        tail_frames: 2,
                    }),
                    0,
                ),
                &mut ctx,
            )
            .expect("apply_control failed");

        let mut first = block(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(stage.process(&mut first, &mut ctx), StageStatus::Ok);
        assert_eq!(first.samples, vec![2.0, 3.0]);

        let mut second = block(&[6.0, 7.0, 8.0]);
        assert_eq!(stage.process(&mut second, &mut ctx), StageStatus::Ok);
        assert_eq!(second.samples, vec![4.0, 5.0, 6.0]);
    }

    #[test]
    fn seek_to_zero_reenables_head_trim() {
        let mut stage = GaplessTrimStage::new();
        let mut ctx = PipelineContext::default();
        let _ = stage
            .prepare(
                StreamSpec {
                    sample_rate: 1,
                    channels: 1,
                },
                &mut ctx,
            )
            .expect("prepare failed");
        stage
            .apply_control(
                &GaplessTrimControl::new(
                    Some(GaplessTrimSpec {
                        head_frames: 1,
                        tail_frames: 0,
                    }),
                    0,
                ),
                &mut ctx,
            )
            .expect("apply_control failed");

        let mut a = block(&[0.0, 1.0]);
        assert_eq!(stage.process(&mut a, &mut ctx), StageStatus::Ok);
        assert_eq!(a.samples, vec![1.0]);

        ctx.pending_seek_ms = Some(500);
        stage
            .sync_runtime_control(&mut ctx)
            .expect("sync_runtime_control failed");
        let mut b = block(&[10.0, 11.0]);
        assert_eq!(stage.process(&mut b, &mut ctx), StageStatus::Ok);
        assert_eq!(b.samples, vec![10.0, 11.0]);

        ctx.pending_seek_ms = Some(0);
        stage
            .sync_runtime_control(&mut ctx)
            .expect("sync_runtime_control failed");
        let mut c = block(&[20.0, 21.0]);
        assert_eq!(stage.process(&mut c, &mut ctx), StageStatus::Ok);
        assert_eq!(c.samples, vec![21.0]);
    }
}
