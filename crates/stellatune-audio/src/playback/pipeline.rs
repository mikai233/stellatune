//! Track-local decoding state and configured transform stages.
//! Prepared, active, and overlapping tracks move this state as one unit. Decoded
//! cursors include encoder trim at the decoder rate; audible cursors use the mix
//! rate. Output clocks and epochs belong to the output worker, outside this module.
use super::normalizer::PcmNormalizer;
use stellatune_audio_core::{
    decoder::DecodeStatus,
    decoder::DecoderStage,
    error::PlaybackControlError,
    format::{AudioBlock, PcmFormat},
    transform::{DrainStatus, TransformStage, TransformStatus},
};

/// Decoder, trim, transforms, and normalization shared by every track role.
pub(super) struct TrackPipeline {
    pub(super) decoder_id: stellatune_audio_core::stage::StageId,
    pub(super) decoder: Box<dyn DecoderStage>,
    pub(super) pre_mix_transforms: Vec<ConfiguredTransform>,
    pub(super) decoded_format: PcmFormat,
    pub(super) mix_format: PcmFormat,
    pub(super) normalizer: Option<PcmNormalizer>,
    pub(super) duration_frames: Option<u64>,
    pub(super) trim_head_frames: u64,
    pub(super) trim_tail_frames: u64,
    pub(super) raw_duration_frames: Option<u64>,
    pub(super) tail_buffer: Vec<f32>,
    pub(super) decoded_frame: u64,
    pub(super) produced_audible_frame: u64,
    pub(super) normalizer_input_format: PcmFormat,
}

/// A stage and its configured output format always travel together.
pub(super) struct ConfiguredTransform {
    stage: Box<dyn TransformStage>,
    id: stellatune_audio_core::stage::StageId,
    pub(super) output_format: PcmFormat,
}
impl ConfiguredTransform {
    pub(super) fn new(
        stage: Box<dyn TransformStage>,
        output_format: PcmFormat,
        id: stellatune_audio_core::stage::StageId,
    ) -> Self {
        Self {
            stage,
            output_format,
            id,
        }
    }
    pub(super) fn process(
        &mut self,
        block: &mut AudioBlock,
    ) -> Result<TransformStatus, stellatune_audio_core::error::PlaybackControlError> {
        let status = self.stage.process(block).map_err(|error| {
            stellatune_audio_core::error::PlaybackControlError::transform(error, self.id.clone())
        })?;
        block.format = self.output_format;
        Ok(status)
    }
    pub(super) fn drain(
        &mut self,
        block: &mut AudioBlock,
    ) -> Result<DrainStatus, stellatune_audio_core::error::PlaybackControlError> {
        let status = self.stage.drain(block).map_err(|error| {
            stellatune_audio_core::error::PlaybackControlError::transform(error, self.id.clone())
        })?;
        block.format = self.output_format;
        Ok(status)
    }
    pub(super) fn reset(&mut self) {
        self.stage.reset();
    }
}

/// Output of one bounded track decode operation.
pub(super) enum TrackBlockStatus {
    Data(AudioBlock),
    Pending,
    EndOfStream,
}

/// Decodes, gapless-trims, transforms, and normalizes one track block.
impl TrackPipeline {
    pub(super) fn decode(
        &mut self,
        block_frames: usize,
        epoch: u64,
    ) -> Result<TrackBlockStatus, PlaybackControlError> {
        let pipeline = self;
        let mut block = AudioBlock::new(pipeline.decoded_format);
        block.timeline.start_frame = pipeline.decoded_frame;
        block.timeline.epoch = epoch;
        block
            .samples
            .reserve(block_frames.saturating_mul(usize::from(
                pipeline.decoded_format.channel_layout.channel_count(),
            )));
        match pipeline
            .decoder
            .decode(&mut block)
            .map_err(|error| PlaybackControlError::decoder(error, pipeline.decoder_id.clone()))?
        {
            DecodeStatus::Produced { frames } if frames > 0 && !block.samples.is_empty() => {
                let raw_start = pipeline.decoded_frame;
                pipeline.decoded_frame =
                    pipeline.decoded_frame.saturating_add(block.frames() as u64);
                trim_gapless_samples(
                    &mut block,
                    raw_start,
                    pipeline.trim_head_frames,
                    pipeline.trim_tail_frames,
                    pipeline.raw_duration_frames,
                    &mut pipeline.tail_buffer,
                );
                if block.samples.is_empty() {
                    return Ok(TrackBlockStatus::Pending);
                }
                block.timeline.start_frame = pipeline.produced_audible_frame;
                process_transform_chain(&mut pipeline.pre_mix_transforms, &mut block)?;
                if block.samples.is_empty() {
                    return Ok(TrackBlockStatus::Pending);
                }
                if let Some(normalizer) = pipeline.normalizer.as_mut() {
                    normalizer.process(&mut block)?;
                }
                if block.samples.is_empty() {
                    return Ok(TrackBlockStatus::Pending);
                }
                pipeline.produced_audible_frame = pipeline
                    .produced_audible_frame
                    .saturating_add(block.frames() as u64);
                Ok(TrackBlockStatus::Data(block))
            },
            DecodeStatus::Produced { .. } | DecodeStatus::Pending => Ok(TrackBlockStatus::Pending),
            DecodeStatus::EndOfStream => Ok(TrackBlockStatus::EndOfStream),
        }
    }
}

/// Runs a block through an ordered transform suffix until output or buffering.
pub(super) fn process_transform_chain(
    transforms: &mut [ConfiguredTransform],
    block: &mut AudioBlock,
) -> Result<(), PlaybackControlError> {
    for transform in transforms {
        match transform.process(block)? {
            TransformStatus::Produced => {},
            TransformStatus::Buffered => {
                block.samples.clear();
                return Ok(());
            },
        }
    }
    Ok(())
}

/// Removes encoder delay and withholds possible tail padding from raw PCM.
pub(super) fn trim_gapless_samples(
    block: &mut AudioBlock,
    raw_start: u64,
    trim_head_frames: u64,
    trim_tail_frames: u64,
    raw_duration_frames: Option<u64>,
    tail_buffer: &mut Vec<f32>,
) {
    let channels = usize::from(block.format.channel_layout.channel_count());
    let raw_end = raw_start.saturating_add(block.frames() as u64);
    let keep_start = raw_start.max(trim_head_frames);
    let known_keep_end =
        raw_duration_frames.map(|duration| duration.saturating_sub(trim_tail_frames));
    let keep_end = known_keep_end.map_or(raw_end, |end| raw_end.min(end));
    if keep_end <= keep_start {
        block.samples.clear();
        return;
    }
    let drop_head_frames = keep_start.saturating_sub(raw_start) as usize;
    let keep_frames = keep_end.saturating_sub(keep_start) as usize;
    let start_sample = drop_head_frames.saturating_mul(channels);
    let end_sample = start_sample.saturating_add(keep_frames.saturating_mul(channels));
    if start_sample > 0 || end_sample < block.samples.len() {
        block.samples = block.samples[start_sample..end_sample].to_vec();
    }

    if raw_duration_frames.is_none() && trim_tail_frames > 0 {
        tail_buffer.extend_from_slice(&block.samples);
        let held_samples = (trim_tail_frames as usize).saturating_mul(channels);
        if tail_buffer.len() <= held_samples {
            block.samples.clear();
        } else {
            let emit_samples = tail_buffer.len().saturating_sub(held_samples);
            block.samples = tail_buffer.drain(..emit_samples).collect();
        }
    }
}
