//! Typed Stellatune playback runtime.
//!
//! [`playback`] owns the actor, bounded PCM pump, sink worker, state machine,
//! generation and epoch. [`planner`] turns an already-materialized
//! `PlaybackItem` into the single executable factory plan.
#![deny(clippy::wildcard_imports)]

pub mod config;
pub mod planner;
pub mod playback;
