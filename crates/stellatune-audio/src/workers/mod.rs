//! Internal worker entry points.
//!
//! Worker modules host long-lived loops that execute decode and sink operations
//! outside of the control actor thread.
//!
//! # Thread Topology
//!
//! The audio runtime uses two dedicated worker threads:
//! - decode worker: owns runner stepping and transition logic,
//! - sink worker: owns sink stage I/O and ring-buffer consumption.
//!
//! Commands originate from the engine actor and are forwarded to workers through
//! bounded channels to preserve backpressure behavior under load.

pub(crate) mod decode;
pub(crate) mod sink;
