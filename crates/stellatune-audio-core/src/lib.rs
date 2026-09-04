//! Stable contracts for Stellatune's audio pipeline.
//!
//! This crate defines the data types and stage interfaces shared by the
//! playback runtime and audio adapters. It does not select stages, schedule
//! playback, access a media catalog, or own an output thread. Those policies
//! belong to `stellatune-audio` and the application layers above it.
//!
//! # Pipeline model
//!
//! A playback item binds a [`source::SourceFactory`] and, optionally, a
//! required [`decoder::DecoderFactory`]. The runtime connects the resulting
//! stages in this order:
//!
//! ```text
//! SourceFactory -> EncodedSource -> DecoderStage -> TransformStage -> SinkStage
//!                                     |                 |
//!                                     +---- AudioBlock -+
//! ```
//!
//! Opening a source is asynchronous because it may perform network or storage
//! I/O. Once opened, encoded reads, decoding, transformation, and sink writes
//! are synchronous, bounded operations. A stage reports temporary
//! backpressure explicitly instead of blocking an audio-processing turn.
//!
//! # Modules
//!
//! - [`mod@format`] defines positioned channel layouts and interleaved PCM blocks.
//! - [`playback`] defines stable item identifiers and media time.
//! - [`source`] defines encoded media acquisition.
//! - [`decoder`], [`transform`], and [`sink`] define pipeline stage interfaces.
//! - [`stage`] defines stable stage identities.
//! - [`error`] defines stage, control, and recovery failures.
//!
//! Public types remain under their owning modules; the crate root deliberately
//! does not re-export them.
//!
//! # Example
//!
//! ```
//! use stellatune_audio_core::format::{AudioBlock, ChannelLayout, PcmFormat};
//!
//! let format = PcmFormat {
//!     sample_rate: 48_000,
//!     channel_layout: ChannelLayout::STEREO,
//! };
//! let mut block = AudioBlock::new(format);
//! block.samples.extend([0.25, -0.25, 0.5, -0.5]);
//!
//! assert_eq!(block.frames(), 2);
//! assert!(block.validate().is_ok());
//! ```
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
#![deny(clippy::wildcard_imports)]

/// Decoder stage contracts and decoded stream metadata.
pub mod decoder;
/// Errors shared across source, stage, and playback boundaries.
pub mod error;
/// Positioned PCM formats, channel layouts, and audio blocks.
pub mod format;
/// Playback item identities and media-time values.
pub mod playback;
/// Audio output stage contracts and compatibility metadata.
pub mod sink;
/// Encoded source factories, capabilities, and cancellation.
pub mod source;
/// Stable identities for pluggable audio stages.
pub mod stage;
/// In-place PCM transform contracts and placement rules.
pub mod transform;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_root_does_not_flatten_module_api() {
        let source = include_str!("lib.rs");
        assert!(
            source
                .lines()
                .all(|line| !line.trim_start().starts_with("pub use ")),
            "audio-core types must remain under their owning public modules"
        );
    }
}
