use crate::config::engine::LfeMode;
use crate::pipeline::runtime::dsp::mixer::layout::ChannelLayout;

const CENTER_COEFF: f32 = std::f32::consts::FRAC_1_SQRT_2;
const SURROUND_COEFF: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn lfe_coeff(mode: LfeMode) -> f32 {
    match mode {
        LfeMode::Mute => 0.0,
        LfeMode::MixToFront => 0.707,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MixMatrix {
    coeffs: Vec<Vec<f32>>,
    in_channels: usize,
    out_channels: usize,
}

impl MixMatrix {
    pub(crate) fn create(from: ChannelLayout, to: ChannelLayout, lfe_mode: LfeMode) -> Self {
        let in_ch = from.channel_count();
        let out_ch = to.channel_count();
        if in_ch == out_ch {
            return Self::identity(in_ch);
        }

        match (from, to) {
            (ChannelLayout::Mono, ChannelLayout::Stereo) => Self::upmix_mono_to_stereo(),
            (ChannelLayout::Stereo, ChannelLayout::Mono) => Self::downmix_stereo_to_mono(),
            (ChannelLayout::Surround5_1, ChannelLayout::Stereo) => {
                Self::downmix_5_1_to_stereo(lfe_mode)
            },
            (ChannelLayout::Surround7_1, ChannelLayout::Stereo) => {
                Self::downmix_7_1_to_stereo(lfe_mode)
            },
            (ChannelLayout::Surround5_1, ChannelLayout::Mono) => {
                Self::downmix_5_1_to_mono(lfe_mode)
            },
            (ChannelLayout::Surround7_1, ChannelLayout::Mono) => {
                Self::downmix_7_1_to_mono(lfe_mode)
            },
            (ChannelLayout::Stereo, ChannelLayout::Surround5_1) => Self::upmix_stereo_to_5_1(),
            (ChannelLayout::Surround7_1, ChannelLayout::Surround5_1) => Self::downmix_7_1_to_5_1(),
            _ => Self::create_generic(in_ch, out_ch),
        }
    }

    pub(crate) fn apply(&self, input: &[f32]) -> Vec<f32> {
        if self.in_channels == self.out_channels {
            return input.to_vec();
        }
        let frames = input.len() / self.in_channels;
        let mut output = vec![0.0; frames * self.out_channels];
        for frame in 0..frames {
            let in_offset = frame * self.in_channels;
            let out_offset = frame * self.out_channels;
            for out_ch in 0..self.out_channels {
                let mut sum = 0.0;
                for in_ch in 0..self.in_channels {
                    sum += input[in_offset + in_ch] * self.coeffs[out_ch][in_ch];
                }
                output[out_offset + out_ch] = sum;
            }
        }
        output
    }

    fn identity(channels: usize) -> Self {
        let mut coeffs = vec![vec![0.0; channels]; channels];
        for (index, row) in coeffs.iter_mut().enumerate().take(channels) {
            row[index] = 1.0;
        }
        Self {
            coeffs,
            in_channels: channels,
            out_channels: channels,
        }
    }

    fn upmix_mono_to_stereo() -> Self {
        Self {
            coeffs: vec![vec![1.0], vec![1.0]],
            in_channels: 1,
            out_channels: 2,
        }
    }

    fn downmix_stereo_to_mono() -> Self {
        Self {
            coeffs: vec![vec![0.5, 0.5]],
            in_channels: 2,
            out_channels: 1,
        }
    }

    fn downmix_5_1_to_stereo(lfe_mode: LfeMode) -> Self {
        let lfe = lfe_coeff(lfe_mode);
        Self {
            coeffs: vec![
                vec![1.0, 0.0, CENTER_COEFF, lfe, SURROUND_COEFF, 0.0],
                vec![0.0, 1.0, CENTER_COEFF, lfe, 0.0, SURROUND_COEFF],
            ],
            in_channels: 6,
            out_channels: 2,
        }
    }

    fn downmix_7_1_to_stereo(lfe_mode: LfeMode) -> Self {
        let lfe = lfe_coeff(lfe_mode);
        Self {
            coeffs: vec![
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
        }
    }

    fn downmix_5_1_to_mono(lfe_mode: LfeMode) -> Self {
        let lfe = lfe_coeff(lfe_mode);
        let k = 0.5;
        Self {
            coeffs: vec![vec![
                k,
                k,
                k * CENTER_COEFF * 2.0,
                lfe,
                k * SURROUND_COEFF,
                k * SURROUND_COEFF,
            ]],
            in_channels: 6,
            out_channels: 1,
        }
    }

    fn downmix_7_1_to_mono(lfe_mode: LfeMode) -> Self {
        let lfe = lfe_coeff(lfe_mode);
        let k = 0.5;
        Self {
            coeffs: vec![vec![
                k,
                k,
                k * CENTER_COEFF * 2.0,
                lfe,
                k * SURROUND_COEFF,
                k * SURROUND_COEFF,
                k * SURROUND_COEFF,
                k * SURROUND_COEFF,
            ]],
            in_channels: 8,
            out_channels: 1,
        }
    }

    fn upmix_stereo_to_5_1() -> Self {
        Self {
            coeffs: vec![
                vec![1.0, 0.0],
                vec![0.0, 1.0],
                vec![0.5, 0.5],
                vec![0.0, 0.0],
                vec![0.707, 0.0],
                vec![0.0, 0.707],
            ],
            in_channels: 2,
            out_channels: 6,
        }
    }

    fn downmix_7_1_to_5_1() -> Self {
        Self {
            coeffs: vec![
                vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.707, 0.0],
                vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.707],
            ],
            in_channels: 8,
            out_channels: 6,
        }
    }

    fn create_generic(in_channels: usize, out_channels: usize) -> Self {
        let mut coeffs = vec![vec![0.0; in_channels]; out_channels];
        if out_channels <= in_channels {
            for (i, row) in coeffs.iter_mut().enumerate().take(out_channels) {
                row[i] = 1.0;
            }
            if in_channels > out_channels {
                let extra = in_channels - out_channels;
                let share = 1.0 / out_channels.max(1) as f32;
                for row in coeffs.iter_mut().take(out_channels) {
                    for coeff in row.iter_mut().take(in_channels).skip(out_channels) {
                        *coeff = share / extra as f32;
                    }
                }
            }
        } else {
            for (i, row) in coeffs.iter_mut().enumerate().take(in_channels) {
                row[i] = 1.0;
            }
        }
        Self {
            coeffs,
            in_channels,
            out_channels,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::engine::LfeMode;
    use crate::pipeline::runtime::dsp::mixer::layout::ChannelLayout;
    use crate::pipeline::runtime::dsp::mixer::matrix::MixMatrix;

    #[test]
    fn mono_to_stereo_duplicates_samples() {
        let matrix = MixMatrix::create(ChannelLayout::Mono, ChannelLayout::Stereo, LfeMode::Mute);
        let mixed = matrix.apply(&[0.5, 1.0]);
        assert_eq!(mixed, vec![0.5, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn stereo_to_mono_averages_channels() {
        let matrix = MixMatrix::create(ChannelLayout::Stereo, ChannelLayout::Mono, LfeMode::Mute);
        let mixed = matrix.apply(&[0.8, 0.2, 0.4, 0.6]);
        assert_eq!(mixed, vec![0.5, 0.5]);
    }
}
