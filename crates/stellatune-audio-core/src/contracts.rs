use std::num::NonZeroU64;
use std::sync::Arc;

use crate::{DecoderFactory, SourceFactory};

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU64);

        impl $name {
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

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.get()
            }
        }
    };
}

opaque_id!(PlaybackItemId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageId(String);

impl StageId {
    pub fn new(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            return Err("stage id cannot be empty");
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub channel_mask: Option<u64>,
}

impl AudioFormat {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.sample_rate == 0 {
            return Err("sample rate must be non-zero");
        }
        if self.channels == 0 {
            return Err("channel count must be non-zero");
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
    pub format: AudioFormat,
    pub timeline: BlockTimeline,
    pub samples: Vec<f32>,
}

impl AudioBlock {
    pub fn new(format: AudioFormat) -> Self {
        Self {
            format,
            timeline: BlockTimeline::default(),
            samples: Vec::new(),
        }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.format.channels.max(1))
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.format.validate()?;
        if !self
            .samples
            .len()
            .is_multiple_of(usize::from(self.format.channels))
        {
            return Err("sample count must be divisible by channel count");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaHints {
    pub extension: Option<String>,
    pub mime_type: Option<String>,
    pub content_length: Option<u64>,
    pub container_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceCapabilities {
    pub byte_seekable: bool,
    pub reopenable: bool,
    pub live: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceDescriptor {
    pub media: MediaHints,
    pub capabilities: SourceCapabilities,
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
