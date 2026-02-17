use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType};
use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::assembly::ResamplerPlan;
use crate::types::ResampleQuality;

const RESAMPLE_CHUNK_FRAMES: usize = 1024;

#[derive(Debug, Clone, Copy)]
struct ResampleParams {
    sinc_len: usize,
    f_cutoff: f32,
    oversampling_factor: usize,
    window: rubato::WindowFunction,
    interpolation: SincInterpolationType,
}

impl ResampleParams {
    fn from_quality(quality: ResampleQuality) -> Self {
        use rubato::WindowFunction;

        match quality {
            ResampleQuality::Fast => Self {
                sinc_len: 64,
                f_cutoff: 0.92,
                oversampling_factor: 64,
                window: WindowFunction::Blackman,
                interpolation: SincInterpolationType::Nearest,
            },
            ResampleQuality::Balanced => Self {
                sinc_len: 128,
                f_cutoff: 0.94,
                oversampling_factor: 128,
                window: WindowFunction::Blackman,
                interpolation: SincInterpolationType::Linear,
            },
            ResampleQuality::High => Self {
                sinc_len: 256,
                f_cutoff: 0.95,
                oversampling_factor: 128,
                window: WindowFunction::BlackmanHarris2,
                interpolation: SincInterpolationType::Linear,
            },
            ResampleQuality::Ultra => Self {
                sinc_len: 512,
                f_cutoff: 0.98,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
                interpolation: SincInterpolationType::Cubic,
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResamplerStage {
    plan: ResamplerPlan,
    channels: usize,
    source_sample_rate: u32,
    target_sample_rate: u32,
    active: bool,
    resampler: Option<Async<f32>>,
}

impl ResamplerStage {
    pub(crate) fn new(plan: ResamplerPlan) -> Self {
        let target_sample_rate = plan.target_sample_rate.max(1);
        Self {
            plan,
            channels: 1,
            source_sample_rate: target_sample_rate,
            target_sample_rate,
            active: false,
            resampler: None,
        }
    }

    fn build_resampler(&self) -> Result<Async<f32>, PipelineError> {
        let ratio = self.target_sample_rate as f64 / self.source_sample_rate as f64;
        let params_spec = ResampleParams::from_quality(self.plan.quality);
        let params = SincInterpolationParameters {
            sinc_len: params_spec.sinc_len,
            f_cutoff: params_spec.f_cutoff,
            oversampling_factor: params_spec.oversampling_factor,
            interpolation: params_spec.interpolation,
            window: params_spec.window,
        };

        Async::<f32>::new_sinc(
            ratio,
            2.0,
            &params,
            RESAMPLE_CHUNK_FRAMES,
            self.channels,
            FixedAsync::Input,
        )
        .map_err(|e| PipelineError::StageFailure(format!("failed to create resampler: {e}")))
    }

    fn process_chunk(
        resampler: &mut Async<f32>,
        channels: usize,
        input_chunk: &[f32],
    ) -> Result<Vec<f32>, PipelineError> {
        let frames = input_chunk.len() / channels;
        if frames == 0 {
            return Ok(Vec::new());
        }
        resampler.set_chunk_size(frames).map_err(|e| {
            PipelineError::StageFailure(format!("resampler set_chunk_size failed: {e}"))
        })?;

        let input = InterleavedSlice::new(input_chunk, channels, frames).map_err(|e| {
            PipelineError::StageFailure(format!("resample input buffer error: {e}"))
        })?;
        let output = resampler
            .process(&input, 0, None)
            .map_err(|e| PipelineError::StageFailure(format!("resample error: {e}")))?;
        Ok(output.take_data())
    }
}

impl TransformStage for ResamplerStage {
    fn prepare(
        &mut self,
        spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.channels = spec.channels.max(1) as usize;
        self.source_sample_rate = spec.sample_rate.max(1);
        self.target_sample_rate = self.plan.target_sample_rate.max(1);
        self.active = self.target_sample_rate != self.source_sample_rate;

        if self.active {
            self.resampler = Some(self.build_resampler()?);
            Ok(StreamSpec {
                sample_rate: self.target_sample_rate,
                channels: spec.channels,
            })
        } else {
            self.resampler = None;
            Ok(spec)
        }
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn process(&mut self, block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        if !self.active || block.is_empty() {
            return StageStatus::Ok;
        }

        let channels = block.channels.max(1) as usize;
        if channels != self.channels || !block.samples.len().is_multiple_of(channels) {
            return StageStatus::Fatal;
        }

        let Some(resampler) = self.resampler.as_mut() else {
            return StageStatus::Fatal;
        };

        let input = std::mem::take(&mut block.samples);
        let mut output = Vec::new();
        let mut offset = 0usize;
        while offset < input.len() {
            let remaining_frames = (input.len() - offset) / channels;
            let chunk_frames = remaining_frames.min(RESAMPLE_CHUNK_FRAMES);
            if chunk_frames == 0 {
                break;
            }
            let chunk_samples = chunk_frames * channels;
            let chunk = &input[offset..offset + chunk_samples];
            let resampled = match Self::process_chunk(resampler, channels, chunk) {
                Ok(samples) => samples,
                Err(_) => return StageStatus::Fatal,
            };
            output.extend(resampled);
            offset += chunk_samples;
        }
        block.samples = output;
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        if let Some(resampler) = self.resampler.as_mut() {
            resampler.reset();
        }
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        if let Some(resampler) = self.resampler.as_mut() {
            resampler.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(samples: &[f32], channels: u16) -> AudioBlock {
        AudioBlock {
            channels,
            samples: samples.to_vec(),
        }
    }

    #[test]
    fn downsamples_stereo_rate_by_half_with_rubato() {
        let mut stage = ResamplerStage::new(ResamplerPlan::new(24_000, ResampleQuality::Balanced));
        let mut ctx = PipelineContext::default();
        let out = stage
            .prepare(
                StreamSpec {
                    sample_rate: 48_000,
                    channels: 2,
                },
                &mut ctx,
            )
            .expect("prepare failed");
        assert_eq!(out.sample_rate, 24_000);

        let mut input = block(&[0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0], 2);
        assert_eq!(stage.process(&mut input, &mut ctx), StageStatus::Ok);
        assert_eq!(input.channels, 2);
        assert!(!input.samples.is_empty());
        assert!(input.samples.len().is_multiple_of(2));
    }

    #[test]
    fn passthrough_when_sample_rate_matches() {
        let mut stage = ResamplerStage::new(ResamplerPlan::new(48_000, ResampleQuality::High));
        let mut ctx = PipelineContext::default();
        let out = stage
            .prepare(
                StreamSpec {
                    sample_rate: 48_000,
                    channels: 1,
                },
                &mut ctx,
            )
            .expect("prepare failed");
        assert_eq!(out.sample_rate, 48_000);

        let mut input = block(&[0.2, 0.4, 0.6], 1);
        assert_eq!(stage.process(&mut input, &mut ctx), StageStatus::Ok);
        assert_eq!(input.samples, vec![0.2, 0.4, 0.6]);
    }
}
