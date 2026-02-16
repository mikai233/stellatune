#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum ChannelLayout {
    Mono,
    #[default]
    Stereo,
    Surround5_1,
    Surround7_1,
    Custom(u16),
}

impl ChannelLayout {
    pub(super) const fn channel_count(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround5_1 => 6,
            Self::Surround7_1 => 8,
            Self::Custom(n) => n as usize,
        }
    }

    pub(super) const fn from_count(channels: u16) -> Self {
        match channels {
            1 => Self::Mono,
            2 => Self::Stereo,
            6 => Self::Surround5_1,
            8 => Self::Surround7_1,
            other => Self::Custom(other),
        }
    }
}
