//! Decoder factories, stream metadata, and incremental decoding.
//!
//! A decoder is created by a [`DecoderFactory`](crate::decoder::DecoderFactory),
//! opened once with an [`EncodedSource`](crate::source::EncodedSource), and
//! then driven through bounded calls to
//! [`DecoderStage::decode`](crate::decoder::DecoderStage::decode). Seeking may
//! span multiple calls so a runtime never has to block one scheduling turn on
//! a slow container seek.

use crate::{
    error::{DecodeError, FactoryError},
    format::{AudioBlock, PcmFormat},
    source::{EncodedSource, MediaHints},
    stage::StageId,
};

/// Encoder delay and padding to remove for gapless playback.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GaplessTrimSpec {
    /// Frames to discard from the beginning of the decoded stream.
    pub head_frames: u32,
    /// Frames to withhold from the end of the decoded stream.
    pub tail_frames: u32,
}

/// PCM format, duration, and gapless metadata discovered while opening a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedStreamInfo {
    /// The format produced by the decoder.
    pub format: PcmFormat,
    /// Total decoded frames before gapless trimming, when known.
    pub duration_frames: Option<u64>,
    /// Encoder delay and padding metadata, when available.
    pub gapless_trim: Option<GaplessTrimSpec>,
}

/// The result of one bounded decoding turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeStatus {
    /// PCM frames were written to the supplied output block.
    Produced {
        /// The number of complete frames produced.
        frames: usize,
    },
    /// The decoder needs more encoded input before it can produce PCM.
    Pending,
    /// The decoder has no more PCM to produce for this stream.
    EndOfStream,
}

/// The actual decoder position reached by a seek operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekResult {
    /// The zero-based decoded frame at which subsequent output begins.
    pub actual_frame: u64,
}

/// Progress made by an incremental decoder seek.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderSeekStatus {
    /// The seek needs another bounded call to [`DecoderStage::continue_seek`].
    Pending,
    /// The decoder has reached the reported frame.
    Complete(SeekResult),
}

/// A stateful encoded-audio decoder driven in bounded turns.
///
/// The runtime calls [`Self::open`] before decoding, and calls [`Self::reset`]
/// before reusing or dropping a prepared pipeline. The format returned from
/// `open` must remain stable until reset.
pub trait DecoderStage: Send {
    /// Opens an encoded stream and reports the PCM stream it will produce.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Unsupported`] for an unsupported stream or
    /// another [`DecodeError`] when probing or initialization fails.
    fn open(
        &mut self,
        source: Box<dyn EncodedSource>,
        hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError>;
    /// Performs one bounded decode operation into `output`.
    ///
    /// A produced block must use the format returned by [`Self::open`], contain
    /// the reported number of complete frames, and satisfy
    /// [`AudioBlock::validate`].
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Pending`] or [`DecodeStatus::Pending`] when no
    /// PCM is currently available, and another [`DecodeError`] for decoding or
    /// source I/O failures.
    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError>;
    /// Starts seeking toward an absolute decoded frame.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Unsupported`] when seeking is unavailable, or
    /// another [`DecodeError`] when the seek cannot be started.
    fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError>;
    /// Advances a seek previously reported as [`DecoderSeekStatus::Pending`].
    ///
    /// # Errors
    ///
    /// Returns a [`DecodeError`] when the in-progress seek cannot continue.
    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError>;
    /// Clears stream-specific decoding and seek state.
    fn reset(&mut self);
}

/// Static metadata used to select and order a decoder factory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderDescriptor {
    /// The stable decoder implementation identifier.
    pub id: StageId,
    /// Selection priority; larger values are preferred.
    pub priority: i32,
    /// Supported filename extensions, compared without ASCII case.
    pub extensions: Vec<String>,
    /// Supported media types, compared without ASCII case.
    pub mime_types: Vec<String>,
}

/// Creates independent decoder stages and exposes their selection metadata.
pub trait DecoderFactory: Send + Sync {
    /// Returns the factory's stable selection descriptor.
    fn descriptor(&self) -> &DecoderDescriptor;
    /// Creates a fresh, unopened decoder stage.
    ///
    /// # Errors
    ///
    /// Returns [`FactoryError`] when configuration is invalid or the decoder
    /// instance cannot be created.
    fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError>;
}
