//! Channel and channel layout definitions.

/// Individual audio channel identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Channel {
    FrontLeft = 0,
    FrontRight = 1,
    FrontCenter = 2,
    LowFrequency = 3,
    BackLeft = 4,
    BackRight = 5,
    SideLeft = 6,
    SideRight = 7,
}

/// Standard channel layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChannelLayout {
    /// Mono: 1 channel (Center)
    Mono,
    /// Stereo: 2 channels (FL, FR)
    #[default]
    Stereo,
    /// 5.1 Surround: 6 channels (FL, FR, FC, LFE, BL, BR)
    Surround5_1,
    /// 7.1 Surround: 8 channels (FL, FR, FC, LFE, BL, BR, SL, SR)
    Surround7_1,
    /// Custom/unknown layout with arbitrary channel count
    Custom(u16),
}

/// Options for LFE channel routing during downmixing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LfeMode {
    /// LFE is discarded (standard ITU-R BS.775-3 behavior).
    #[default]
    Mute,
    /// LFE is mixed into Front Left and Front Right channels.
    /// Typically used when speakers can handle low frequencies (Large speakers).
    MixToFront,
}

impl ChannelLayout {
    /// Returns the number of channels in this layout.
    pub const fn channel_count(&self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround5_1 => 6,
            Self::Surround7_1 => 8,
            Self::Custom(n) => *n as usize,
        }
    }

    /// Create a layout from channel count.
    /// Maps common counts to named layouts, otherwise uses Custom.
    pub const fn from_count(n: u16) -> Self {
        match n {
            1 => Self::Mono,
            2 => Self::Stereo,
            6 => Self::Surround5_1,
            8 => Self::Surround7_1,
            _ => Self::Custom(n),
        }
    }

    /// Returns the channels in order for this layout.
    pub fn channels(&self) -> &'static [Channel] {
        match self {
            Self::Mono => &[Channel::FrontCenter],
            Self::Stereo => &[Channel::FrontLeft, Channel::FrontRight],
            Self::Surround5_1 => &[
                Channel::FrontLeft,
                Channel::FrontRight,
                Channel::FrontCenter,
                Channel::LowFrequency,
                Channel::BackLeft,
                Channel::BackRight,
            ],
            Self::Surround7_1 => &[
                Channel::FrontLeft,
                Channel::FrontRight,
                Channel::FrontCenter,
                Channel::LowFrequency,
                Channel::BackLeft,
                Channel::BackRight,
                Channel::SideLeft,
                Channel::SideRight,
            ],
            Self::Custom(_) => &[], // Unknown mapping
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_count_matches() {
        assert_eq!(ChannelLayout::Mono.channel_count(), 1);
        assert_eq!(ChannelLayout::Stereo.channel_count(), 2);
        assert_eq!(ChannelLayout::Surround5_1.channel_count(), 6);
        assert_eq!(ChannelLayout::Surround7_1.channel_count(), 8);
        assert_eq!(ChannelLayout::Custom(4).channel_count(), 4);
    }

    #[test]
    fn from_count_roundtrip() {
        assert_eq!(ChannelLayout::from_count(1), ChannelLayout::Mono);
        assert_eq!(ChannelLayout::from_count(2), ChannelLayout::Stereo);
        assert_eq!(ChannelLayout::from_count(6), ChannelLayout::Surround5_1);
        assert_eq!(ChannelLayout::from_count(8), ChannelLayout::Surround7_1);
        assert_eq!(ChannelLayout::from_count(4), ChannelLayout::Custom(4));
    }
}
