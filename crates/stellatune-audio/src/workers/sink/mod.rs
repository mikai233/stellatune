//! Sink worker runtime and control loop.
//!
//! # Role
//!
//! The sink worker isolates sink I/O from decode pacing. Decode-side producers
//! push blocks into a bounded ring, while the sink thread consumes and writes
//! to sink stages.
//!
//! # Design Notes
//!
//! - Audio transport uses a bounded queue to express backpressure explicitly.
//! - Control RPCs (`drain`, `drop_queued`, `sync_runtime_control`) use a separate
//!   mailbox so they are not blocked behind audio block traffic.

pub(crate) mod worker;
