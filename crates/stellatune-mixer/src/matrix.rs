//! Mixing matrix for channel conversion.
//!
//! Implements ITU-R BS.775-3 compliant mixing coefficients.

use crate::ChannelLayout;

/// Standard mixing coefficient for center channel (1/√2 ≈ -3dB)
pub const CENTER_COEFF: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Standard mixing coefficient for surround channels (1/√2 ≈ -3dB)
pub const SURROUND_COEFF: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Calculate LFE coefficient based on mode.
fn get_lfe_coeff(mode: crate::LfeMode) -> f32 {
    match mode {
        crate::LfeMode::Mute => 0.0,
        crate::LfeMode::MixToFront => 0.707, // Mix LFE into mains at -3dB
    }
}

/// Mixing matrix for converting between channel layouts.
///
/// The matrix is stored as `coeffs[out_ch][in_ch]`, where each output sample
/// is computed as: `out[ch] = sum(in[i] * coeffs[ch][i] for i in 0..in_channels)`
#[derive(Debug, Clone)]
pub struct MixMatrix {
    coeffs: Vec<Vec<f32>>,
    in_channels: usize,
    pub out_channels: usize,
    #[allow(dead_code)]
    pub lfe_mode: crate::LfeMode,
}

impl MixMatrix {
    /// Create a mixing matrix for the given layout conversion.
    pub fn create(from: ChannelLayout, to: ChannelLayout, lfe_mode: crate::LfeMode) -> Self {
        let in_ch = from.channel_count();
        let out_ch = to.channel_count();

        if in_ch == out_ch {
            return Self::identity(in_ch);
        }

        match (from, to) {
            // Mono → Stereo: duplicate
            (ChannelLayout::Mono, ChannelLayout::Stereo) => Self::upmix_mono_to_stereo(lfe_mode),

            // Stereo → Mono: average
            (ChannelLayout::Stereo, ChannelLayout::Mono) => Self::downmix_stereo_to_mono(lfe_mode),

            // 5.1 → Stereo: ITU-R BS.775-3
            (ChannelLayout::Surround5_1, ChannelLayout::Stereo) => {
                Self::downmix_5_1_to_stereo(lfe_mode)
            },

            // 7.1 → Stereo: ITU-R BS.775-3 extended
            (ChannelLayout::Surround7_1, ChannelLayout::Stereo) => {
                Self::downmix_7_1_to_stereo(lfe_mode)
            },

            // 5.1 → Mono
            (ChannelLayout::Surround5_1, ChannelLayout::Mono) => {
                Self::downmix_5_1_to_mono(lfe_mode)
            },

            // 7.1 → Mono
            (ChannelLayout::Surround7_1, ChannelLayout::Mono) => {
                Self::downmix_7_1_to_mono(lfe_mode)
            },

            // Stereo → 5.1: basic upmix
            (ChannelLayout::Stereo, ChannelLayout::Surround5_1) => {
                Self::upmix_stereo_to_5_1(lfe_mode)
            },

            // 7.1 → 5.1: drop side channels
            (ChannelLayout::Surround7_1, ChannelLayout::Surround5_1) => {
                Self::downmix_7_1_to_5_1(lfe_mode)
            },

            // Default: try to create a reasonable matrix
            _ => Self::create_generic(in_ch, out_ch, lfe_mode),
        }
    }

    /// Identity matrix (no mixing, passthrough).
    pub fn identity(channels: usize) -> Self {
        let mut coeffs = vec![vec![0.0; channels]; channels];
        for (i, row) in coeffs.iter_mut().enumerate().take(channels) {
            row[i] = 1.0;
        }
        Self {
            coeffs,
            in_channels: channels,
            out_channels: channels,
            lfe_mode: crate::LfeMode::default(),
        }
    }

    /// Mono → Stereo: L = R = M
    fn upmix_mono_to_stereo(lfe_mode: crate::LfeMode) -> Self {
        Self {
            coeffs: vec![
                vec![1.0], // L = M
                vec![1.0], // R = M
            ],
            in_channels: 1,
            out_channels: 2,
            lfe_mode,
        }
    }

    /// Stereo → Mono: M = (L + R) * 0.5
    fn downmix_stereo_to_mono(lfe_mode: crate::LfeMode) -> Self {
        Self {
            coeffs: vec![vec![0.5, 0.5]], // M = (L + R) * 0.5
            in_channels: 2,
            out_channels: 1,
            lfe_mode,
        }
    }

    /// 5.1 → Stereo (ITU-R BS.775-3)
    /// Order: FL, FR, FC, LFE, BL, BR
    fn downmix_5_1_to_stereo(lfe_mode: crate::LfeMode) -> Self {
        let lfe = get_lfe_coeff(lfe_mode);
        Self {
            coeffs: vec![
                // L = FL + CENTER_COEFF*FC + SURROUND_COEFF*BL + lfe*LFE
                vec![1.0, 0.0, CENTER_COEFF, lfe, SURROUND_COEFF, 0.0],
                // R = FR + CENTER_COEFF*FC + SURROUND_COEFF*BR + lfe*LFE
                vec![0.0, 1.0, CENTER_COEFF, lfe, 0.0, SURROUND_COEFF],
            ],
            in_channels: 6,
            out_channels: 2,
            lfe_mode,
        }
    }

    /// 7.1 → Stereo (ITU-R BS.775-3 extended)
    /// Order: FL, FR, FC, LFE, BL, BR, SL, SR
    fn downmix_7_1_to_stereo(lfe_mode: crate::LfeMode) -> Self {
        let lfe = get_lfe_coeff(lfe_mode);
        Self {
            coeffs: vec![
                // L = FL + CENTER_COEFF*FC + SURROUND_COEFF*(BL + SL) + lfe*LFE
                vec![
                    1.0,
                    0.0,
                    CENTER_COEFF,
                    lfe,
                    SURROUND_COEFF,
                    0.0,
                    SURROUND_COEFF,
                    0.0,
                ],
                // R = FR + CENTER_COEFF*FC + SURROUND_COEFF*(BR + SR) + lfe*LFE
                vec![
                    0.0,
                    1.0,
                    CENTER_COEFF,
                    lfe,
                    0.0,
                    SURROUND_COEFF,
                    0.0,
                    SURROUND_COEFF,
                ],
            ],
            in_channels: 8,
            out_channels: 2,
            lfe_mode,
        }
    }

    /// 5.1 → Mono
    fn downmix_5_1_to_mono(lfe_mode: crate::LfeMode) -> Self {
        let lfe = get_lfe_coeff(lfe_mode);
        // First downmix to stereo, then to mono
        let k = 0.5; // stereo to mono factor
        Self {
            coeffs: vec![vec![
                k,                      // FL
                k,                      // FR
                k * CENTER_COEFF * 2.0, // FC (appears in both L and R)
                lfe,                    // LFE
                k * SURROUND_COEFF,     // BL
                k * SURROUND_COEFF,     // BR
            ]],
            in_channels: 6,
            out_channels: 1,
            lfe_mode,
        }
    }

    /// 7.1 → Mono
    fn downmix_7_1_to_mono(lfe_mode: crate::LfeMode) -> Self {
        let lfe = get_lfe_coeff(lfe_mode);
        let k = 0.5;
        Self {
            coeffs: vec![vec![
                k,                      // FL
                k,                      // FR
                k * CENTER_COEFF * 2.0, // FC
                lfe,                    // LFE
                k * SURROUND_COEFF,     // BL
                k * SURROUND_COEFF,     // BR
                k * SURROUND_COEFF,     // SL
                k * SURROUND_COEFF,     // SR
            ]],
            in_channels: 8,
            out_channels: 1,
            lfe_mode,
        }
    }

    /// Stereo → 5.1 (basic upmix)
    fn upmix_stereo_to_5_1(lfe_mode: crate::LfeMode) -> Self {
        Self {
            coeffs: vec![
                vec![1.0, 0.0],   // FL = L
                vec![0.0, 1.0],   // FR = R
                vec![0.5, 0.5],   // FC = (L + R) / 2
                vec![0.0, 0.0],   // LFE = silence
                vec![0.707, 0.0], // BL = L * 0.707
                vec![0.0, 0.707], // BR = R * 0.707
            ],
            in_channels: 2,
            out_channels: 6,
            lfe_mode,
        }
    }

    /// 7.1 → 5.1 (fold side channels into back)
    fn downmix_7_1_to_5_1(lfe_mode: crate::LfeMode) -> Self {
        // Combine side with back: BL' = BL + SL*0.707, BR' = BR + SR*0.707
        Self {
            coeffs: vec![
                vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],   // FL
                vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],   // FR
                vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],   // FC
                vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],   // LFE
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.707, 0.0], // BL + SL
                vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.707], // BR + SR
            ],
            in_channels: 8,
            out_channels: 6,
            lfe_mode,
        }
    }

    /// Generic matrix for arbitrary channel counts.
    /// Downmix: average extra channels into first out_ch channels.
    /// Upmix: copy first channels, zero the rest.
    fn create_generic(in_ch: usize, out_ch: usize, lfe_mode: crate::LfeMode) -> Self {
        let mut coeffs = vec![vec![0.0; in_ch]; out_ch];

        if out_ch <= in_ch {
            // Downmix: each output channel gets its corresponding input
            // plus averaged contribution from extra inputs
            for (i, row) in coeffs.iter_mut().enumerate().take(out_ch) {
                row[i] = 1.0;
            }
            // Distribute extra channels
            if in_ch > out_ch {
                let extra = in_ch - out_ch;
                let factor = 1.0 / (out_ch as f32);
                for row in coeffs.iter_mut().take(out_ch) {
                    for coeff in row.iter_mut().take(in_ch).skip(out_ch) {
                        *coeff = factor / (extra as f32);
                    }
                }
            }
        } else {
            // Upmix: copy inputs, rest are silent
            for (i, row) in coeffs.iter_mut().enumerate().take(in_ch) {
                row[i] = 1.0;
            }
        }

        Self {
            coeffs,
            in_channels: in_ch,
            out_channels: out_ch,
            lfe_mode,
        }
    }

    /// Apply the mixing matrix to interleaved input samples.
    pub fn apply(&self, input: &[f32]) -> Vec<f32> {
        let frames = input.len() / self.in_channels;
        let mut output = vec![0.0; frames * self.out_channels];

        for frame in 0..frames {
            let in_offset = frame * self.in_channels;
            let out_offset = frame * self.out_channels;

            for (out_ch, coeff_row) in self.coeffs.iter().enumerate() {
                let mut sum = 0.0;
                for (in_ch, &coeff) in coeff_row.iter().enumerate() {
                    sum += input[in_offset + in_ch] * coeff;
                }
                output[out_offset + out_ch] = sum;
            }
        }

        output
    }

    /// Number of input channels.
    pub fn in_channels(&self) -> usize {
        self.in_channels
    }

    /// Number of output channels.
    pub fn out_channels(&self) -> usize {
        self.out_channels
    }
}

#[cfg(test)]
mod tests {
    use super::{CENTER_COEFF, MixMatrix};
    use crate::{ChannelLayout, LfeMode};

    #[test]
    fn identity_passthrough() {
        let matrix = MixMatrix::identity(2);
        let input = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, stereo
        let output = matrix.apply(&input);
        assert_eq!(output, input);
    }

    #[test]
    fn mono_to_stereo() {
        let matrix = MixMatrix::create(
            ChannelLayout::Mono,
            ChannelLayout::Stereo,
            LfeMode::default(),
        );
        let input = vec![0.5, 1.0]; // 2 frames, mono
        let output = matrix.apply(&input);
        // Each mono sample should appear in both L and R
        assert_eq!(output, vec![0.5, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn stereo_to_mono() {
        let matrix = MixMatrix::create(
            ChannelLayout::Stereo,
            ChannelLayout::Mono,
            LfeMode::default(),
        );
        let input = vec![0.6, 0.4, 1.0, 0.0]; // 2 frames, stereo
        let output = matrix.apply(&input);
        // (L + R) * 0.5
        assert_eq!(output, vec![0.5, 0.5]);
    }

    #[test]
    fn downmix_5_1_to_stereo_basic() {
        let matrix = MixMatrix::create(
            ChannelLayout::Surround5_1,
            ChannelLayout::Stereo,
            LfeMode::default(),
        );
        // One frame: FL=1, FR=0, FC=0, LFE=0, BL=0, BR=0
        let input = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let output = matrix.apply(&input);
        // L should be 1.0, R should be 0.0
        assert!((output[0] - 1.0).abs() < 0.001);
        assert!((output[1] - 0.0).abs() < 0.001);
    }

    #[test]
    fn downmix_5_1_center_contribution() {
        let matrix = MixMatrix::create(
            ChannelLayout::Surround5_1,
            ChannelLayout::Stereo,
            LfeMode::default(),
        );
        // One frame: FC=1, all others=0
        let input = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
        let output = matrix.apply(&input);
        // Both L and R should get CENTER_COEFF * 1.0
        assert!((output[0] - CENTER_COEFF).abs() < 0.001);
        assert!((output[1] - CENTER_COEFF).abs() < 0.001);
    }
}
