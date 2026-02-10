//! Channel mixer for converting between channel layouts.

use crate::{ChannelLayout, MixMatrix};

/// Mixer for converting audio between different channel layouts.
///
/// # Example
/// ```
/// use stellatune_mixer::{ChannelLayout, ChannelMixer, LfeMode};
///
/// let mixer = ChannelMixer::new(
///     ChannelLayout::Surround5_1,
///     ChannelLayout::Stereo,
///     LfeMode::MixToFront,
/// );
/// let input_5_1 = vec![0.5; 6]; // One frame of 5.1 audio
/// let output_stereo = mixer.mix(&input_5_1);
/// assert_eq!(output_stereo.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct ChannelMixer {
    in_layout: ChannelLayout,
    out_layout: ChannelLayout,
    matrix: MixMatrix,
}

impl ChannelMixer {
    /// Create a new mixer for the given input/output layouts.
    pub fn new(
        in_layout: ChannelLayout,
        out_layout: ChannelLayout,
        lfe_mode: crate::LfeMode,
    ) -> Self {
        let matrix = MixMatrix::create(in_layout, out_layout, lfe_mode);
        Self {
            in_layout,
            out_layout,
            matrix,
        }
    }

    /// Mix interleaved input samples to the output layout.
    ///
    /// # Panics
    /// Panics if input length is not a multiple of input channel count.
    pub fn mix(&self, input: &[f32]) -> Vec<f32> {
        let in_ch = self.in_layout.channel_count();
        debug_assert!(
            input.len().is_multiple_of(in_ch),
            "input length {} not divisible by channel count {}",
            input.len(),
            in_ch
        );

        if self.in_layout == self.out_layout {
            return input.to_vec();
        }

        self.matrix.apply(input)
    }

    /// Returns the input channel count.
    pub fn in_channels(&self) -> usize {
        self.in_layout.channel_count()
    }

    /// Returns the output channel count.
    pub fn out_channels(&self) -> usize {
        self.out_layout.channel_count()
    }

    /// Returns the input layout.
    pub fn in_layout(&self) -> ChannelLayout {
        self.in_layout
    }

    /// Returns the output layout.
    pub fn out_layout(&self) -> ChannelLayout {
        self.out_layout
    }

    /// Returns true if this mixer is a no-op (same layout in and out).
    pub fn is_passthrough(&self) -> bool {
        self.in_layout == self.out_layout
    }
}

#[cfg(test)]
mod tests {
    use super::ChannelMixer;
    use crate::{ChannelLayout, LfeMode};

    #[test]
    fn passthrough_same_layout() {
        let mixer = ChannelMixer::new(
            ChannelLayout::Stereo,
            ChannelLayout::Stereo,
            LfeMode::default(),
        );
        assert!(mixer.is_passthrough());

        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = mixer.mix(&input);
        assert_eq!(output, input);
    }

    #[test]
    fn mono_to_stereo_mixer() {
        let mixer = ChannelMixer::new(
            ChannelLayout::Mono,
            ChannelLayout::Stereo,
            LfeMode::default(),
        );
        assert!(!mixer.is_passthrough());
        assert_eq!(mixer.in_channels(), 1);
        assert_eq!(mixer.out_channels(), 2);

        let input = vec![0.5, 1.0];
        let output = mixer.mix(&input);
        assert_eq!(output, vec![0.5, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn stereo_to_mono_mixer() {
        let mixer = ChannelMixer::new(
            ChannelLayout::Stereo,
            ChannelLayout::Mono,
            LfeMode::default(),
        );
        assert_eq!(mixer.in_channels(), 2);
        assert_eq!(mixer.out_channels(), 1);

        let input = vec![0.8, 0.2];
        let output = mixer.mix(&input);
        assert_eq!(output.len(), 1);
        assert!((output[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn surround_5_1_to_stereo() {
        let mixer = ChannelMixer::new(
            ChannelLayout::Surround5_1,
            ChannelLayout::Stereo,
            LfeMode::default(),
        );
        assert_eq!(mixer.in_channels(), 6);
        assert_eq!(mixer.out_channels(), 2);

        // FL=1, FR=1, others=0 → L=1, R=1
        let input = vec![1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        let output = mixer.mix(&input);
        assert_eq!(output.len(), 2);
        assert!((output[0] - 1.0).abs() < 0.001);
        assert!((output[1] - 1.0).abs() < 0.001);
    }
}
