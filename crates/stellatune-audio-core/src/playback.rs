//! Playback item identities and media-time values.
//!
//! These types identify an already-materialized playback request. Catalog
//! lookup, persistence, and source resolution remain outside this crate.

use std::num::NonZeroU64;
use std::sync::Arc;

use crate::{decoder::DecoderFactory, source::SourceFactory};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A stable, non-zero identifier for one playback item.
///
/// Values are restricted to the positive signed 64-bit range so they can be
/// persisted losslessly in SQLite and passed through signed FFI boundaries.
pub struct PlaybackItemId(NonZeroU64);

impl PlaybackItemId {
    /// Constructs an identifier from its integer representation.
    ///
    /// Returns `None` for zero or for a value greater than [`i64::MAX`].
    ///
    /// # Examples
    ///
    /// ```
    /// use stellatune_audio_core::playback::PlaybackItemId;
    ///
    /// let id = PlaybackItemId::new(42).expect("42 is a valid item id");
    /// assert_eq!(id.get(), 42);
    /// assert!(PlaybackItemId::new(0).is_none());
    /// ```
    pub fn new(value: u64) -> Option<Self> {
        (value <= i64::MAX as u64)
            .then(|| NonZeroU64::new(value))
            .flatten()
            .map(Self)
    }

    /// Returns the non-zero integer representation of this identifier.
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
/// A non-negative media position with millisecond precision.
///
/// Frame conversions use integer arithmetic and truncate fractional results.
pub struct MediaTime(u64);

impl MediaTime {
    /// The zero media position.
    pub const ZERO: Self = Self(0);

    /// Constructs a media position from milliseconds.
    pub const fn from_millis(value: u64) -> Self {
        Self(value)
    }

    /// Returns this position in milliseconds.
    pub const fn as_millis(self) -> u64 {
        self.0
    }

    /// Converts this position to a frame index at `sample_rate`.
    ///
    /// Multiplication saturates on overflow. A zero sample rate produces zero
    /// frames.
    pub fn to_frames(self, sample_rate: u32) -> u64 {
        self.0.saturating_mul(u64::from(sample_rate)) / 1000
    }

    /// Constructs a media position from a frame index and sample rate.
    ///
    /// Multiplication saturates on overflow. A zero sample rate produces
    /// [`Self::ZERO`].
    ///
    /// # Examples
    ///
    /// ```
    /// use stellatune_audio_core::playback::MediaTime;
    ///
    /// let position = MediaTime::from_frames(48_000, 48_000);
    /// assert_eq!(position, MediaTime::from_millis(1_000));
    /// assert_eq!(position.to_frames(44_100), 44_100);
    /// ```
    pub fn from_frames(frames: u64, sample_rate: u32) -> Self {
        if sample_rate == 0 {
            return Self::ZERO;
        }
        Self(frames.saturating_mul(1000) / u64::from(sample_rate))
    }
}

#[derive(Clone)]
/// A source and decoder requirement bound to a stable playback identity.
///
/// The source factory is reusable so preparation can reopen the same item for
/// decoder fallback or recovery. When [`Self::required_decoder`] is `None`, the
/// planner selects compatible decoder candidates from its registry.
pub struct PlaybackItem {
    /// The stable identity used by events, persistence, and failure context.
    pub id: PlaybackItemId,
    /// The factory that opens encoded media for this item.
    pub source: Arc<dyn SourceFactory>,
    /// A decoder that must be used instead of registry-based selection.
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
