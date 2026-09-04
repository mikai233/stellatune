const MAX_SUPPORTED_CHANNELS: u16 = 12;

/// A positioned loudspeaker in the canonical interleaved PCM order.
///
/// The discriminants intentionally match the first 18 speaker bits used by
/// WAVEFORMATEXTENSIBLE and Symphonia's positioned channel representation.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpeakerPosition {
    FrontLeft = 0,
    FrontRight = 1,
    FrontCenter = 2,
    Lfe = 3,
    RearLeft = 4,
    RearRight = 5,
    FrontLeftCenter = 6,
    FrontRightCenter = 7,
    RearCenter = 8,
    SideLeft = 9,
    SideRight = 10,
    TopCenter = 11,
    TopFrontLeft = 12,
    TopFrontCenter = 13,
    TopFrontRight = 14,
    TopRearLeft = 15,
    TopRearCenter = 16,
    TopRearRight = 17,
}

impl SpeakerPosition {
    pub const ALL: [Self; 18] = [
        Self::FrontLeft,
        Self::FrontRight,
        Self::FrontCenter,
        Self::Lfe,
        Self::RearLeft,
        Self::RearRight,
        Self::FrontLeftCenter,
        Self::FrontRightCenter,
        Self::RearCenter,
        Self::SideLeft,
        Self::SideRight,
        Self::TopCenter,
        Self::TopFrontLeft,
        Self::TopFrontCenter,
        Self::TopFrontRight,
        Self::TopRearLeft,
        Self::TopRearCenter,
        Self::TopRearRight,
    ];

    const fn bit(self) -> u32 {
        1_u32 << self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChannelLayoutError {
    #[error("channel layout must contain at least one speaker position")]
    Empty,
    #[error("channel layout contains duplicate speaker position {0:?}")]
    Duplicate(SpeakerPosition),
    #[error("channel layout exceeds the supported maximum of 12 channels")]
    TooManyChannels,
}

/// A non-empty set of positioned loudspeakers.
///
/// Samples in each interleaved PCM frame use the order returned by
/// [`ChannelLayout::positions`]. The representation is private so invalid or
/// ambiguous layouts cannot enter the audio pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelLayout(u32);

impl ChannelLayout {
    pub const MONO: Self = Self(SpeakerPosition::FrontCenter.bit());
    pub const STEREO: Self =
        Self(SpeakerPosition::FrontLeft.bit() | SpeakerPosition::FrontRight.bit());
    pub const SURROUND_3_0: Self = Self(
        SpeakerPosition::FrontLeft.bit()
            | SpeakerPosition::FrontRight.bit()
            | SpeakerPosition::FrontCenter.bit(),
    );
    pub const SURROUND_3_1: Self = Self(Self::SURROUND_3_0.0 | SpeakerPosition::Lfe.bit());
    pub const QUAD: Self = Self(
        SpeakerPosition::FrontLeft.bit()
            | SpeakerPosition::FrontRight.bit()
            | SpeakerPosition::RearLeft.bit()
            | SpeakerPosition::RearRight.bit(),
    );
    pub const SURROUND_5_0_REAR: Self = Self(Self::SURROUND_3_0.0 | Self::QUAD.0);
    pub const SURROUND_5_1_REAR: Self =
        Self(Self::SURROUND_5_0_REAR.0 | SpeakerPosition::Lfe.bit());
    pub const SURROUND_5_0_SIDE: Self = Self(
        Self::SURROUND_3_0.0 | SpeakerPosition::SideLeft.bit() | SpeakerPosition::SideRight.bit(),
    );
    pub const SURROUND_5_1_SIDE: Self =
        Self(Self::SURROUND_5_0_SIDE.0 | SpeakerPosition::Lfe.bit());
    pub const SURROUND_7_0: Self = Self(Self::SURROUND_5_0_SIDE.0 | Self::QUAD.0);
    pub const SURROUND_7_1: Self = Self(Self::SURROUND_7_0.0 | SpeakerPosition::Lfe.bit());
    pub const SURROUND_7_1_4: Self = Self(
        Self::SURROUND_7_1.0
            | SpeakerPosition::TopFrontLeft.bit()
            | SpeakerPosition::TopFrontRight.bit()
            | SpeakerPosition::TopRearLeft.bit()
            | SpeakerPosition::TopRearRight.bit(),
    );

    pub fn from_positions(
        positions: impl IntoIterator<Item = SpeakerPosition>,
    ) -> Result<Self, ChannelLayoutError> {
        let mut bits = 0_u32;
        let mut count = 0_u16;
        for position in positions {
            let bit = position.bit();
            if bits & bit != 0 {
                return Err(ChannelLayoutError::Duplicate(position));
            }
            bits |= bit;
            count += 1;
            if count > MAX_SUPPORTED_CHANNELS {
                return Err(ChannelLayoutError::TooManyChannels);
            }
        }
        if bits == 0 {
            return Err(ChannelLayoutError::Empty);
        }
        Ok(Self(bits))
    }

    pub const fn contains(self, position: SpeakerPosition) -> bool {
        self.0 & position.bit() != 0
    }

    pub const fn channel_count(self) -> u16 {
        self.0.count_ones() as u16
    }

    pub fn positions(self) -> impl Iterator<Item = SpeakerPosition> {
        SpeakerPosition::ALL
            .into_iter()
            .filter(move |position| self.contains(*position))
    }

    pub fn index_of(self, position: SpeakerPosition) -> Option<usize> {
        self.contains(position).then(|| {
            self.positions()
                .take_while(|item| *item != position)
                .count()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channel_layout: ChannelLayout,
}

impl PcmFormat {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.sample_rate == 0 {
            return Err("sample rate must be non-zero");
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockTimeline {
    pub start_frame: u64,
    pub epoch: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioBlock {
    pub format: PcmFormat,
    pub timeline: BlockTimeline,
    pub samples: Vec<f32>,
}

impl AudioBlock {
    pub fn new(format: PcmFormat) -> Self {
        Self {
            format,
            timeline: BlockTimeline::default(),
            samples: Vec::new(),
        }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.format.channel_layout.channel_count())
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.format.validate()?;
        if !self
            .samples
            .len()
            .is_multiple_of(usize::from(self.format.channel_layout.channel_count()))
        {
            return Err("sample count must be divisible by channel count");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelLayout, ChannelLayoutError, SpeakerPosition};

    #[test]
    fn standard_layouts_report_expected_channel_counts() {
        assert_eq!(ChannelLayout::MONO.channel_count(), 1);
        assert_eq!(ChannelLayout::STEREO.channel_count(), 2);
        assert_eq!(ChannelLayout::QUAD.channel_count(), 4);
        assert_eq!(ChannelLayout::SURROUND_5_1_SIDE.channel_count(), 6);
        assert_eq!(ChannelLayout::SURROUND_5_1_REAR.channel_count(), 6);
        assert_eq!(ChannelLayout::SURROUND_7_1.channel_count(), 8);
        assert_eq!(ChannelLayout::SURROUND_7_1_4.channel_count(), 12);
    }

    #[test]
    fn positions_follow_canonical_interleaved_order() {
        assert_eq!(
            ChannelLayout::SURROUND_7_1.positions().collect::<Vec<_>>(),
            vec![
                SpeakerPosition::FrontLeft,
                SpeakerPosition::FrontRight,
                SpeakerPosition::FrontCenter,
                SpeakerPosition::Lfe,
                SpeakerPosition::RearLeft,
                SpeakerPosition::RearRight,
                SpeakerPosition::SideLeft,
                SpeakerPosition::SideRight,
            ]
        );
        assert_eq!(
            ChannelLayout::SURROUND_7_1.index_of(SpeakerPosition::SideLeft),
            Some(6)
        );
    }

    #[test]
    fn invalid_layout_construction_is_rejected() {
        assert_eq!(
            ChannelLayout::from_positions([]),
            Err(ChannelLayoutError::Empty)
        );
        assert_eq!(
            ChannelLayout::from_positions(
                [SpeakerPosition::FrontLeft, SpeakerPosition::FrontLeft,]
            ),
            Err(ChannelLayoutError::Duplicate(SpeakerPosition::FrontLeft))
        );
        assert_eq!(
            ChannelLayout::from_positions(SpeakerPosition::ALL.into_iter().take(13),),
            Err(ChannelLayoutError::TooManyChannels)
        );
    }
}
