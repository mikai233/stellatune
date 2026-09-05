//! Playback control, observable events, and runtime ownership.
//!
//! The public surface is split by responsibility:
//!
//! - [`control`](crate::playback::control) contains the cloneable command endpoint.
//! - [`event`](crate::playback::event) contains state snapshots and broadcast events.
//! - [`runtime`](crate::playback::runtime) starts and deterministically shuts down
//!   playback resources.
//!
//! Internal modules implement the actor state machine and synchronous PCM data
//! path. They are intentionally not public extension points.

/// Typed commands sent to a running playback actor.
pub mod control;
/// Observable playback states, snapshots, and events.
pub mod event;
/// Playback actor startup, configuration, and shutdown.
pub mod runtime;

mod actor;
mod lifecycle;
mod normalizer;
mod output_workers;
mod pipeline;
mod preparation;
mod pump;
mod pump_signal;
mod sink_worker;
mod state;
mod transition;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod actor_tests;
