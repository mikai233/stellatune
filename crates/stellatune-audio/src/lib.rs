//! Stellatune's typed playback planner and runtime.
//!
//! This crate turns an already-materialized
//! [`PlaybackItem`](stellatune_audio_core::playback::PlaybackItem) into a
//! running audio pipeline. It owns stage selection, playback state, transition
//! policy, bounded PCM movement, recovery, and output lifecycle. It does not
//! resolve catalog identifiers, persist queue state, fetch application
//! metadata, or implement concrete codecs and devices.
//!
//! # Architecture
//!
//! [`planner`] creates an immutable executable plan from a playback item and a
//! stage registry. [`playback::runtime::PlaybackRuntime`] owns a dedicated
//! playback actor, while cloneable
//! [`playback::control::PlaybackController`] values send typed commands to it.
//! Slow source preparation runs outside actor turns. PCM moves through a
//! bounded ring to a separate sink worker and never enters the actor mailbox.
//!
//! ```text
//! PlaybackController -> PlaybackActor -> decode/transform/mix
//!                                           |
//!                                           v
//!                                    bounded PCM ring
//!                                           |
//!                                           v
//!                                      SinkWorker
//! ```
//!
//! The actor owns the current item, queued item, generation, seek, transition,
//! and recovery policy. Public position values are derived from sink-consumed
//! frames rather than the decoder or queue frontier.
//!
//! # Control example
//!
//! The application normally obtains a controller from a running composition
//! root. Control examples therefore accept an existing controller instead of
//! constructing fake audio stages.
//!
//! ```
//! use stellatune_audio::playback::control::PlaybackController;
//! use stellatune_audio_core::error::PlaybackControlError;
//! use stellatune_audio_core::playback::MediaTime;
//!
//! async fn pause_seek_and_resume(
//!     controller: &PlaybackController,
//! ) -> Result<(), PlaybackControlError> {
//!     controller.pause().await?;
//!     controller.seek(MediaTime::from_millis(30_000)).await?;
//!     controller.play().await
//! }
//! ```
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
#![deny(clippy::wildcard_imports)]

/// Runtime configuration values shared with application layers.
pub mod config;
/// Deterministic stage selection and executable playback plans.
pub mod planner;
/// Playback control, events, and runtime lifecycle.
pub mod playback;
