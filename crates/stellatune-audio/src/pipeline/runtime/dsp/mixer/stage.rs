use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::pipeline::assembly::MixerPlan;
use crate::pipeline::runtime::dsp::mixer::layout::ChannelLayout;
use crate::pipeline::runtime::dsp::mixer::matrix::MixMatrix;

#[derive(Debug, Clone)]
pub(crate) struct MixerStage {
    plan: MixerPlan,
    in_channels: usize,
    out_channels: usize,
    matrix: Option<MixMatrix>,
}

impl MixerStage {
    pub(crate) fn new(plan: MixerPlan) -> Self {
        let out_channels = plan.target_channels.max(1) as usize;
        Self {
            plan,
            in_channels: out_channels,
            out_channels,
            matrix: None,
        }
    }

    fn reconfigure(&mut self, in_channels: usize) {
        self.in_channels = in_channels.max(1);
        self.out_channels = self.plan.target_channels.max(1) as usize;
        if self.in_channels == self.out_channels {
            self.matrix = None;
            return;
        }
        self.matrix = Some(MixMatrix::create(
            ChannelLayout::from_count(self.in_channels as u16),
            ChannelLayout::from_count(self.out_channels as u16),
            self.plan.lfe_mode,
        ));
    }
}

impl TransformStage for MixerStage {
    fn prepare(
        &mut self,
        spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.reconfigure(spec.channels.max(1) as usize);
        Ok(StreamSpec {
            sample_rate: spec.sample_rate,
            channels: self.out_channels as u16,
        })
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn process(&mut self, block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        if block.is_empty() {
            return StageStatus::Ok;
        }
        let in_channels = block.channels.max(1) as usize;
        if in_channels != self.in_channels {
            self.reconfigure(in_channels);
        }
        if !block.samples.len().is_multiple_of(in_channels) {
            return StageStatus::Fatal;
        }
        if let Some(matrix) = self.matrix.as_ref() {
            block.samples = matrix.apply(&block.samples);
            block.channels = self.out_channels as u16;
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
    use crate::config::engine::LfeMode;
    use crate::pipeline::assembly::MixerPlan;
    use crate::pipeline::runtime::dsp::mixer::stage::MixerStage;
    use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
    use stellatune_audio_core::pipeline::stages::StageStatus;
    use stellatune_audio_core::pipeline::stages::transform::TransformStage;

    fn block(samples: &[f32], channels: u16) -> AudioBlock {
        AudioBlock {
            channels,
            samples: samples.to_vec(),
        }
    }

    #[test]
    fn mixes_mono_into_stereo() {
        let mut stage = MixerStage::new(MixerPlan::new(2, LfeMode::Mute));
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
        assert_eq!(out.channels, 2);

        let mut mono = block(&[0.5, 1.0], 1);
        assert_eq!(stage.process(&mut mono, &mut ctx), StageStatus::Ok);
        assert_eq!(mono.channels, 2);
        assert_eq!(mono.samples, vec![0.5, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn downmixes_stereo_into_mono() {
        let mut stage = MixerStage::new(MixerPlan::new(1, LfeMode::Mute));
        let mut ctx = PipelineContext::default();
        let out = stage
            .prepare(
                StreamSpec {
                    sample_rate: 44_100,
                    channels: 2,
                },
                &mut ctx,
            )
            .expect("prepare failed");
        assert_eq!(out.channels, 1);

        let mut stereo = block(&[0.8, 0.2, 0.4, 0.6], 2);
        assert_eq!(stage.process(&mut stereo, &mut ctx), StageStatus::Ok);
        assert_eq!(stereo.channels, 1);
        assert_eq!(stereo.samples, vec![0.5, 0.5]);
    }
}
