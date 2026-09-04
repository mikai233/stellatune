//! Positioned PCM formats and interleaved audio blocks.
//!
//! [`ChannelLayout`](crate::format::ChannelLayout) is the source of truth for
//! both channel count and sample order. Each frame in an
//! [`AudioBlock`](crate::format::AudioBlock) contains one `f32` sample for every
//! position returned by
//! [`ChannelLayout::positions`](crate::format::ChannelLayout::positions), in
//! that order.

const MAX_SUPPORTED_CHANNELS: u16 = 12;

/// A positioned loudspeaker in the canonical interleaved PCM order.
///
/// The discriminants intentionally match the first 18 speaker bits used by
/// WAVEFORMATEXTENSIBLE and Symphonia's positioned channel representation.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpeakerPosition {
    /// Front-left speaker.
    FrontLeft = 0,
    /// Front-right speaker.
    FrontRight = 1,
    /// Front-center speaker.
    FrontCenter = 2,
    /// Low-frequency effects channel.
    Lfe = 3,
    /// Rear-left speaker.
    RearLeft = 4,
    /// Rear-right speaker.
    RearRight = 5,
    /// Front speaker between the left and center positions.
    FrontLeftCenter = 6,
    /// Front speaker between the right and center positions.
    FrontRightCenter = 7,
    /// Rear-center speaker.
    RearCenter = 8,
    /// Side-left speaker.
    SideLeft = 9,
    /// Side-right speaker.
    SideRight = 10,
    /// Overhead center speaker.
    TopCenter = 11,
    /// Top-front-left speaker.
    TopFrontLeft = 12,
    /// Top-front-center speaker.
    TopFrontCenter = 13,
    /// Top-front-right speaker.
    TopFrontRight = 14,
    /// Top-rear-left speaker.
    TopRearLeft = 15,
    /// Top-rear-center speaker.
    TopRearCenter = 16,
    /// Top-rear-right speaker.
    TopRearRight = 17,
}

impl SpeakerPosition {
    /// All supported speaker positions in canonical interleaving order.
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
/// An error returned when constructing a [`ChannelLayout`].
pub enum ChannelLayoutError {
    /// No speaker positions were supplied.
    #[error("channel layout must contain at least one speaker position")]
    Empty,
    /// The same speaker position was supplied more than once.
    #[error("channel layout contains duplicate speaker position {0:?}")]
    Duplicate(SpeakerPosition),
    /// The layout contains more channels than the pipeline supports.
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
    /// Mono audio carried by the front-center speaker.
    pub const MONO: Self = Self(SpeakerPosition::FrontCenter.bit());
    /// Two-channel front-left/front-right stereo.
    pub const STEREO: Self =
        Self(SpeakerPosition::FrontLeft.bit() | SpeakerPosition::FrontRight.bit());
    /// Three front channels: left, right, and center.
    pub const SURROUND_3_0: Self = Self(
        SpeakerPosition::FrontLeft.bit()
            | SpeakerPosition::FrontRight.bit()
            | SpeakerPosition::FrontCenter.bit(),
    );
    /// Three front channels plus an LFE channel.
    pub const SURROUND_3_1: Self = Self(Self::SURROUND_3_0.0 | SpeakerPosition::Lfe.bit());
    /// Four corner speakers using rear-left and rear-right surrounds.
    pub const QUAD: Self = Self(
        SpeakerPosition::FrontLeft.bit()
            | SpeakerPosition::FrontRight.bit()
            | SpeakerPosition::RearLeft.bit()
            | SpeakerPosition::RearRight.bit(),
    );
    /// Five-channel surround using rear-left and rear-right surrounds.
    pub const SURROUND_5_0_REAR: Self = Self(Self::SURROUND_3_0.0 | Self::QUAD.0);
    /// 5.1 surround using rear-left and rear-right surrounds.
    pub const SURROUND_5_1_REAR: Self =
        Self(Self::SURROUND_5_0_REAR.0 | SpeakerPosition::Lfe.bit());
    /// Five-channel surround using side-left and side-right surrounds.
    pub const SURROUND_5_0_SIDE: Self = Self(
        Self::SURROUND_3_0.0 | SpeakerPosition::SideLeft.bit() | SpeakerPosition::SideRight.bit(),
    );
    /// 5.1 surround using side-left and side-right surrounds.
    pub const SURROUND_5_1_SIDE: Self =
        Self(Self::SURROUND_5_0_SIDE.0 | SpeakerPosition::Lfe.bit());
    /// Seven-channel surround with both rear and side pairs.
    pub const SURROUND_7_0: Self = Self(Self::SURROUND_5_0_SIDE.0 | Self::QUAD.0);
    /// 7.1 surround with both rear and side pairs.
    pub const SURROUND_7_1: Self = Self(Self::SURROUND_7_0.0 | SpeakerPosition::Lfe.bit());
    /// 7.1.4 surround with four top-front and top-rear speakers.
    pub const SURROUND_7_1_4: Self = Self(
        Self::SURROUND_7_1.0
            | SpeakerPosition::TopFrontLeft.bit()
            | SpeakerPosition::TopFrontRight.bit()
            | SpeakerPosition::TopRearLeft.bit()
            | SpeakerPosition::TopRearRight.bit(),
    );

    /// Constructs a layout from positioned speakers.
    ///
    /// Input order does not affect interleaving order. [`Self::positions`]
    /// always returns positions in [`SpeakerPosition::ALL`] order.
    ///
    /// # Errors
    ///
    /// Returns [`ChannelLayoutError::Empty`] for no positions,
    /// [`ChannelLayoutError::Duplicate`] for a repeated position, or
    /// [`ChannelLayoutError::TooManyChannels`] for more than 12 positions.
    ///
    /// # Examples
    ///
    /// ```
    /// use stellatune_audio_core::format::{ChannelLayout, SpeakerPosition};
    ///
    /// let layout = ChannelLayout::from_positions([
    ///     SpeakerPosition::FrontRight,
    ///     SpeakerPosition::FrontLeft,
    /// ])?;
    /// assert_eq!(layout, ChannelLayout::STEREO);
    /// assert_eq!(layout.positions().next(), Some(SpeakerPosition::FrontLeft));
    /// # Ok::<(), stellatune_audio_core::format::ChannelLayoutError>(())
    /// ```
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

    /// Returns whether the layout contains `position`.
    pub const fn contains(self, position: SpeakerPosition) -> bool {
        self.0 & position.bit() != 0
    }

    /// Returns the number of interleaved samples in each frame.
    pub const fn channel_count(self) -> u16 {
        self.0.count_ones() as u16
    }

    /// Iterates over speaker positions in canonical interleaving order.
    pub fn positions(self) -> impl Iterator<Item = SpeakerPosition> {
        SpeakerPosition::ALL
            .into_iter()
            .filter(move |position| self.contains(*position))
    }

    /// Returns the interleaved sample index of `position` within a frame.
    ///
    /// Returns `None` when the position is not present in this layout.
    pub fn index_of(self, position: SpeakerPosition) -> Option<usize> {
        self.contains(position).then(|| {
            self.positions()
                .take_while(|item| *item != position)
                .count()
        })
    }
}

/// The sample rate and positioned channel layout of PCM audio.
///
/// Samples are interleaved `f32` values. The channel count and order are
/// derived exclusively from [`Self::channel_layout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmFormat {
    /// The number of PCM frames per second.
    pub sample_rate: u32,
    /// The speaker positions and interleaving order of each frame.
    pub channel_layout: ChannelLayout,
}

impl PcmFormat {
    /// Validates the format and returns it unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error when [`Self::sample_rate`] is zero.
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.sample_rate == 0 {
            return Err("sample rate must be non-zero");
        }
        Ok(self)
    }
}

/// The logical position of an [`AudioBlock`] within a playback stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockTimeline {
    /// The first frame's zero-based position within the current item.
    pub start_frame: u64,
    /// The PCM generation used to invalidate data queued before a discontinuity.
    pub epoch: u64,
}

/// A block of interleaved `f32` PCM samples and its stream timeline.
///
/// A valid block contains a whole number of frames for [`Self::format`]. A
/// value of `1.0` or `-1.0` conventionally represents full scale, although
/// callers may use wider intermediate values before the final mix is limited.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBlock {
    /// The format used by every sample in this block.
    pub format: PcmFormat,
    /// The block's item-relative position and discontinuity epoch.
    pub timeline: BlockTimeline,
    /// Interleaved PCM samples in canonical channel-layout order.
    pub samples: Vec<f32>,
}

impl AudioBlock {
    /// Creates an empty block for `format` at the default timeline.
    pub fn new(format: PcmFormat) -> Self {
        Self {
            format,
            timeline: BlockTimeline::default(),
            samples: Vec::new(),
        }
    }

    /// Returns the number of complete PCM frames represented by the samples.
    ///
    /// This method uses integer division. Call [`Self::validate`] when the
    /// block may have been assembled from untrusted or partial data.
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.format.channel_layout.channel_count())
    }

    /// Validates the format and interleaved sample count.
    ///
    /// # Errors
    ///
    /// Returns an error when the sample rate is zero or when the sample count
    /// is not divisible by the channel count.
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
