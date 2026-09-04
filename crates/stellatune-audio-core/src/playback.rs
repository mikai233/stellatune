use std::num::NonZeroU64;
use std::sync::Arc;

use crate::{decoder::DecoderFactory, source::SourceFactory};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaybackItemId(NonZeroU64);

impl PlaybackItemId {
    pub fn new(value: u64) -> Option<Self> {
        (value <= i64::MAX as u64)
            .then(|| NonZeroU64::new(value))
            .flatten()
            .map(Self)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl From<PlaybackItemId> for u64 {
    fn from(value: PlaybackItemId) -> Self {
        value.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaTime(u64);

impl MediaTime {
    pub const ZERO: Self = Self(0);

    pub const fn from_millis(value: u64) -> Self {
        Self(value)
    }

    pub const fn as_millis(self) -> u64 {
        self.0
    }

    pub fn to_frames(self, sample_rate: u32) -> u64 {
        self.0.saturating_mul(u64::from(sample_rate)) / 1000
    }

    pub fn from_frames(frames: u64, sample_rate: u32) -> Self {
        if sample_rate == 0 {
            return Self::ZERO;
        }
        Self(frames.saturating_mul(1000) / u64::from(sample_rate))
    }
}

#[derive(Clone)]
pub struct PlaybackItem {
    pub id: PlaybackItemId,
    pub source: Arc<dyn SourceFactory>,
    pub required_decoder: Option<Arc<dyn DecoderFactory>>,
}

impl std::fmt::Debug for PlaybackItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlaybackItem")
            .field("id", &self.id)
            .field("source", &"<bound source factory>")
            .field(
                "required_decoder",
                &self
                    .required_decoder
                    .as_ref()
                    .map(|factory| factory.descriptor().id.as_str()),
            )
            .finish()
    }
}
