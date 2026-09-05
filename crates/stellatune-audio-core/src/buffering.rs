//! Duration budgets shared by the playback runtime and audio adapters.

use crate::format::PcmFormat;

/// Software buffering preset; this does not include OS or hardware latency.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LatencyProfile {
    /// Smaller scheduling margin and faster response.
    Low,
    /// General-purpose playback buffering.
    #[default]
    Medium,
    /// More tolerance for scheduling and input stalls.
    High,
}

/// Time budgets for one output session. Changes apply when the output is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferingConfig {
    /// Combined output transport and adapter ring target, in milliseconds.
    pub output_ms: u32,
    /// Decoded PCM read-ahead target, in milliseconds.
    pub decode_ahead_ms: u32,
    /// Requested duration of one decode operation, in milliseconds.
    pub block_ms: u32,
    /// Capacity of the software ring immediately before the device callback.
    pub device_ms: u32,
}

impl Default for BufferingConfig {
    fn default() -> Self {
        LatencyProfile::Medium.buffering()
    }
}

impl LatencyProfile {
    /// Returns the software budgets for this preset.
    pub const fn buffering(self) -> BufferingConfig {
        match self {
            Self::Low => BufferingConfig {
                output_ms: 40,
                decode_ahead_ms: 40,
                block_ms: 5,
                device_ms: 20,
            },
            Self::Medium => BufferingConfig {
                output_ms: 100,
                decode_ahead_ms: 100,
                block_ms: 10,
                device_ms: 40,
            },
            Self::High => BufferingConfig {
                output_ms: 200,
                decode_ahead_ms: 200,
                block_ms: 10,
                device_ms: 80,
            },
        }
    }
}

/// PCM allocation ceiling per buffered layer, independent of sample rate.
pub const MAX_BUFFER_BYTES: usize = 8 * 1024 * 1024;
/// Maximum accepted PCM block allocation, bounding target overshoot.
pub const MAX_BLOCK_BYTES: usize = 1024 * 1024;

/// Converts milliseconds to complete frames, rounding upward and limiting PCM memory.
pub fn frames_for_ms(format: PcmFormat, ms: u32) -> usize {
    let bytes_per_frame =
        usize::from(format.channel_layout.channel_count()).max(1) * size_of::<f32>();
    (u64::from(format.sample_rate) * u64::from(ms))
        .div_ceil(1000)
        .max(1)
        .min((MAX_BUFFER_BYTES / bytes_per_frame) as u64) as usize
}

impl BufferingConfig {
    /// Requested decode frames at the input rate, with a separate block memory limit.
    pub fn block_frames(self, format: PcmFormat) -> usize {
        frames_for_ms(format, self.block_ms).min(
            MAX_BLOCK_BYTES
                / (usize::from(format.channel_layout.channel_count()).max(1) * size_of::<f32>()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::ChannelLayout;

    #[test]
    fn preset_durations_are_independent_of_sample_rate() {
        for rate in [8000, 22050, 44100, 48000, 96000, 192000, 384000] {
            for profile in [
                LatencyProfile::Low,
                LatencyProfile::Medium,
                LatencyProfile::High,
            ] {
                let format = PcmFormat {
                    sample_rate: rate,
                    channel_layout: ChannelLayout::STEREO,
                };
                let config = profile.buffering();
                for ms in [
                    config.output_ms,
                    config.decode_ahead_ms,
                    config.device_ms,
                    config.block_ms,
                ] {
                    let frames = frames_for_ms(format, ms) as u64;
                    assert!(frames * 1000 >= u64::from(rate) * u64::from(ms));
                    assert!(frames * 1000 < u64::from(rate) * u64::from(ms) + 1000);
                }
            }
        }
    }

    #[test]
    fn extreme_configuration_still_obeys_pcm_allocation_limits() {
        let format = PcmFormat {
            sample_rate: u32::MAX,
            channel_layout: ChannelLayout::STEREO,
        };
        assert!(frames_for_ms(format, u32::MAX) * 8 <= MAX_BUFFER_BYTES);
        let config = BufferingConfig {
            block_ms: u32::MAX,
            ..Default::default()
        };
        assert!(config.block_frames(format) * 8 <= MAX_BLOCK_BYTES);
    }
}
