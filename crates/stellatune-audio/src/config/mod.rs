//! Runtime configuration values exposed to application layers.
//!
//! These values describe audio-processing choices. Playback lifecycle and
//! scheduling configuration lives in [`crate::playback::runtime`].

/// Engine state, event, and control configuration models.
pub mod engine;
