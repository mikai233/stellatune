//! Runtime configuration and event models.
//!
//! This module contains user-facing settings and event payload types consumed by
//! the engine and surrounding backend layers.

/// Engine state, event, and control configuration models.
pub mod engine;
/// Gain transition policy configuration.
pub mod gain;
/// Sink latency and recovery policy configuration.
pub mod sink;
