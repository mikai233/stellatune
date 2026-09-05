//! Positioned channel mixing and sample-rate normalization.
//!
//! A `PcmNormalizer` converts one planned track format into the shared mix
//! format. Channel mixing happens before resampling so the resampler always
//! operates at the target channel count. Exact speaker positions pass through;
//! expansion leaves missing target positions silent; reduction routes each
//! non-LFE source toward the nearest supported target position.
//!
//! LFE is copied only when the target also has LFE and is otherwise discarded.
//! Matrix rows whose absolute coefficient sum exceeds unity are normalized to
//! prevent mathematical clipping at full-scale input. Resampler startup delay
//! is removed, and drain output is truncated to the exact rate-converted frame
//! count.

use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Indexing, Resampler, Resizable, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use stellatune_audio_core::error::FailureStage;
use stellatune_audio_core::{
    error::PlaybackControlError,
    format::{AudioBlock, ChannelLayout, PcmFormat, SpeakerPosition},
};
const NORMALIZER_CHUNK_FRAMES: usize = 1024;
/// The initial fold-down coefficient for one semantic speaker route.
pub(super) const SQRT_HALF: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// A precomputed row-major channel rematrix applied independently to each frame.
pub(super) struct ChannelMixer {
    source_channels: usize,
    target_channels: usize,
    matrix: Vec<f32>,
}

impl ChannelMixer {
    /// Builds and normalizes a routing matrix for two positioned layouts.
    pub(super) fn new(
        source: ChannelLayout,
        target: ChannelLayout,
    ) -> Result<Self, PlaybackControlError> {
        let source_positions = source.positions().collect::<Vec<_>>();
        let target_positions = target.positions().collect::<Vec<_>>();
        let mut matrix = vec![0.0; source_positions.len() * target_positions.len()];

        for (source_index, position) in source_positions.iter().copied().enumerate() {
            if let Some(target_index) = target.index_of(position) {
                matrix[target_index * source_positions.len() + source_index] = 1.0;
                continue;
            }
            if position == SpeakerPosition::Lfe {
                continue;
            }
            let routes = channel_routes(position, target).ok_or_else(|| {
                PlaybackControlError::failed(
                    FailureStage::Transform,
                    format!("cannot route {position:?} from {source:?} to {target:?}"),
                )
            })?;
            for (target_position, coefficient) in routes {
                let target_index = target.index_of(target_position).ok_or_else(|| {
                    PlaybackControlError::failed(
                        FailureStage::Transform,
                        format!("invalid target route {target_position:?} for {target:?}"),
                    )
                })?;
                matrix[target_index * source_positions.len() + source_index] += coefficient;
            }
        }

        for row in matrix.chunks_exact_mut(source_positions.len()) {
            let sum = row.iter().map(|coefficient| coefficient.abs()).sum::<f32>();
            if sum > 1.0 {
                for coefficient in row {
                    *coefficient /= sum;
                }
            }
        }

        Ok(Self {
            source_channels: source_positions.len(),
            target_channels: target_positions.len(),
            matrix,
        })
    }

    /// Applies the immutable routing matrix to complete interleaved frames.
    pub(super) fn process(&self, input: &[f32]) -> Vec<f32> {
        let frames = input.len() / self.source_channels;
        let mut output = Vec::with_capacity(frames.saturating_mul(self.target_channels));
        for frame in input.chunks_exact(self.source_channels) {
            for row in self.matrix.chunks_exact(self.source_channels) {
                output.push(
                    row.iter()
                        .zip(frame)
                        .map(|(coefficient, sample)| coefficient * sample)
                        .sum(),
                );
            }
        }
        output
    }
}

fn channel_routes(
    source: SpeakerPosition,
    target: ChannelLayout,
) -> Option<Vec<(SpeakerPosition, f32)>> {
    use SpeakerPosition::{
        FrontCenter, FrontLeft, FrontLeftCenter, FrontRight, FrontRightCenter, RearCenter,
        RearLeft, RearRight, SideLeft, SideRight, TopCenter, TopFrontCenter, TopFrontLeft,
        TopFrontRight, TopRearCenter, TopRearLeft, TopRearRight,
    };

    let single = |candidates: &[SpeakerPosition]| {
        candidates
            .iter()
            .copied()
            .find(|position| target.contains(*position))
            .map(|position| vec![(position, SQRT_HALF)])
    };
    let pair = |left: SpeakerPosition, right: SpeakerPosition| {
        (target.contains(left) && target.contains(right))
            .then_some(vec![(left, SQRT_HALF), (right, SQRT_HALF)])
    };

    match source {
        FrontLeft => single(&[FrontLeftCenter, FrontCenter]),
        FrontRight => single(&[FrontRightCenter, FrontCenter]),
        FrontCenter => pair(FrontLeft, FrontRight)
            .or_else(|| pair(FrontLeftCenter, FrontRightCenter))
            .or_else(|| single(&[TopFrontCenter, TopCenter, RearCenter])),
        FrontLeftCenter => single(&[FrontLeft, FrontCenter]),
        FrontRightCenter => single(&[FrontRight, FrontCenter]),
        SideLeft => single(&[RearLeft, FrontLeft, FrontLeftCenter, FrontCenter]),
        SideRight => single(&[RearRight, FrontRight, FrontRightCenter, FrontCenter]),
        RearLeft => single(&[SideLeft, FrontLeft, FrontLeftCenter, FrontCenter]),
        RearRight => single(&[SideRight, FrontRight, FrontRightCenter, FrontCenter]),
        RearCenter => pair(RearLeft, RearRight)
            .or_else(|| pair(SideLeft, SideRight))
            .or_else(|| single(&[FrontCenter]))
            .or_else(|| pair(FrontLeft, FrontRight)),
        TopFrontLeft => single(&[FrontLeft, FrontLeftCenter, FrontCenter]),
        TopFrontRight => single(&[FrontRight, FrontRightCenter, FrontCenter]),
        TopFrontCenter => single(&[FrontCenter]).or_else(|| pair(FrontLeft, FrontRight)),
        TopRearLeft => single(&[RearLeft, SideLeft, FrontLeft, FrontCenter]),
        TopRearRight => single(&[RearRight, SideRight, FrontRight, FrontCenter]),
        TopRearCenter => single(&[RearCenter])
            .or_else(|| pair(RearLeft, RearRight))
            .or_else(|| pair(SideLeft, SideRight))
            .or_else(|| single(&[FrontCenter]))
            .or_else(|| pair(FrontLeft, FrontRight)),
        TopCenter => single(&[FrontCenter, TopFrontCenter, TopRearCenter])
            .or_else(|| pair(FrontLeft, FrontRight)),
        SpeakerPosition::Lfe => Some(Vec::new()),
    }
}

/// A chunked interleaved sinc resampler with exact output-length accounting.
pub(super) struct PcmResampler {
    source_rate: u32,
    target_rate: u32,
    channels: usize,
    inner: Async<f32>,
    input_frames: u64,
    output_frames: u64,
    leading_frames_to_trim: usize,
    drained: bool,
}

impl PcmResampler {
    /// Creates a resampler for one channel count and sample-rate ratio.
    pub(super) fn new(
        source_rate: u32,
        target_rate: u32,
        channels: usize,
    ) -> Result<Self, PlaybackControlError> {
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: Some(0.94),
            oversampling_factor: 128,
            interpolation: SincInterpolationType::Linear,
            window: WindowFunction::Blackman,
        };
        let inner = Async::<f32>::new_sinc(
            target_rate as f64 / source_rate as f64,
            2.0,
            &params,
            NORMALIZER_CHUNK_FRAMES,
            channels,
            FixedAsync::Input,
        )
        .map_err(|error| {
            PlaybackControlError::failed(FailureStage::Transform, error.to_string())
        })?;
        let leading_frames_to_trim = inner.output_delay();
        Ok(Self {
            source_rate,
            target_rate,
            channels,
            inner,
            input_frames: 0,
            output_frames: 0,
            leading_frames_to_trim,
            drained: false,
        })
    }

    /// Converts all complete input frames and removes filter startup delay.
    pub(super) fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, PlaybackControlError> {
        self.input_frames = self
            .input_frames
            .saturating_add((input.len() / self.channels) as u64);
        let mut output = Vec::new();
        let mut offset = 0;
        while offset < input.len() {
            let remaining_frames = (input.len() - offset) / self.channels;
            let frames = remaining_frames.min(NORMALIZER_CHUNK_FRAMES);
            if frames == 0 {
                break;
            }
            let samples = frames.saturating_mul(self.channels);
            self.inner.set_chunk_size(frames).map_err(|error| {
                PlaybackControlError::failed(FailureStage::Transform, error.to_string())
            })?;
            let adapter =
                InterleavedSlice::new(&input[offset..offset + samples], self.channels, frames)
                    .map_err(|error| {
                        PlaybackControlError::failed(FailureStage::Transform, error.to_string())
                    })?;
            output.extend(
                self.inner
                    .process(&adapter, None)
                    .map_err(|error| {
                        PlaybackControlError::failed(FailureStage::Transform, error.to_string())
                    })?
                    .take_data(),
            );
            offset += samples;
        }
        self.trim_leading_frames(&mut output);
        self.output_frames = self
            .output_frames
            .saturating_add((output.len() / self.channels) as u64);
        Ok(output)
    }

    /// Produces one tail chunk, or `None` after the exact target length is reached.
    pub(super) fn drain(&mut self) -> Result<Option<Vec<f32>>, PlaybackControlError> {
        if self.drained {
            return Ok(None);
        }
        let expected_frames = ((self.input_frames as f64 * self.target_rate as f64
            / self.source_rate as f64)
            .ceil()) as u64;
        if self.output_frames >= expected_frames {
            self.drained = true;
            return Ok(None);
        }
        while self.output_frames < expected_frames {
            let input_frames = self.inner.input_frames_next();
            let silence = vec![0.0; input_frames.saturating_mul(self.channels)];
            let adapter =
                InterleavedSlice::new(&silence, self.channels, input_frames).map_err(|error| {
                    PlaybackControlError::failed(FailureStage::Transform, error.to_string())
                })?;
            let mut output = self
                .inner
                .process(&adapter, Some(&Indexing::new().partial_len(0)))
                .map_err(|error| {
                    PlaybackControlError::failed(FailureStage::Transform, error.to_string())
                })?
                .take_data();
            self.trim_leading_frames(&mut output);
            let remaining_frames = expected_frames.saturating_sub(self.output_frames) as usize;
            output.truncate(remaining_frames.saturating_mul(self.channels));
            if !output.is_empty() {
                self.output_frames = self
                    .output_frames
                    .saturating_add((output.len() / self.channels) as u64);
                return Ok(Some(output));
            }
        }
        self.drained = true;
        Ok(None)
    }

    /// Removes as many outstanding filter-delay frames as `samples` contains.
    pub(super) fn trim_leading_frames(&mut self, samples: &mut Vec<f32>) {
        if self.leading_frames_to_trim == 0 {
            return;
        }
        let available_frames = samples.len() / self.channels;
        let trim_frames = available_frames.min(self.leading_frames_to_trim);
        samples.drain(..trim_frames.saturating_mul(self.channels));
        self.leading_frames_to_trim -= trim_frames;
    }

    /// Clears counters and restores the underlying resampler's initial delay.
    pub(super) fn reset(&mut self) {
        self.inner.reset();
        self.leading_frames_to_trim = self.inner.output_delay();
        self.input_frames = 0;
        self.output_frames = 0;
        self.drained = false;
    }
}

/// A planned channel mixer followed by an optional sample-rate converter.
pub(super) struct PcmNormalizer {
    source: PcmFormat,
    target: PcmFormat,
    channel_mixer: Option<ChannelMixer>,
    resampler: Option<PcmResampler>,
}

impl PcmNormalizer {
    /// Builds the minimum conversion chain needed between two valid PCM formats.
    pub(super) fn new(source: PcmFormat, target: PcmFormat) -> Result<Self, PlaybackControlError> {
        source.validate().map_err(|message| {
            PlaybackControlError::failed(FailureStage::Transform, message.to_owned())
        })?;
        target.validate().map_err(|message| {
            PlaybackControlError::failed(FailureStage::Transform, message.to_owned())
        })?;
        let channel_mixer = if source.channel_layout == target.channel_layout {
            None
        } else {
            Some(ChannelMixer::new(
                source.channel_layout,
                target.channel_layout,
            )?)
        };
        let resampler = if source.sample_rate == target.sample_rate {
            None
        } else {
            Some(PcmResampler::new(
                source.sample_rate,
                target.sample_rate,
                usize::from(target.channel_layout.channel_count()),
            )?)
        };
        Ok(Self {
            source,
            target,
            channel_mixer,
            resampler,
        })
    }

    /// Converts one block in place and assigns the target format.
    pub(super) fn process(&mut self, block: &mut AudioBlock) -> Result<(), PlaybackControlError> {
        if block.samples.is_empty() {
            block.format = self.target;
            return Ok(());
        }
        let source_channels = usize::from(self.source.channel_layout.channel_count());
        if block.format != self.source || !block.samples.len().is_multiple_of(source_channels) {
            return Err(PlaybackControlError::failed(
                FailureStage::Transform,
                "normalizer input format changed after planning".to_owned(),
            ));
        }
        let input = std::mem::take(&mut block.samples);
        let remapped = match self.channel_mixer.as_ref() {
            Some(mixer) => mixer.process(&input),
            None => input,
        };
        block.samples = if let Some(resampler) = self.resampler.as_mut() {
            resampler.process(&remapped)?
        } else {
            remapped
        };
        block.format = self.target;
        Ok(())
    }

    /// Writes one resampler tail chunk and reports whether output was produced.
    pub(super) fn drain(&mut self, block: &mut AudioBlock) -> Result<bool, PlaybackControlError> {
        block.format = self.target;
        block.samples.clear();
        let Some(resampler) = self.resampler.as_mut() else {
            return Ok(false);
        };
        block.samples = resampler.drain()?.unwrap_or_default();
        Ok(!block.samples.is_empty())
    }

    /// Resets rate-conversion state after a seek or pipeline teardown.
    pub(super) fn reset(&mut self) {
        if let Some(resampler) = self.resampler.as_mut() {
            resampler.reset();
        }
    }
}
