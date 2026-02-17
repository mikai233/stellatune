//! Runtime execution components for assembled pipelines.
//!
//! # Role
//!
//! This module sits between static pipeline assembly and worker loops. It owns
//! the runtime-only concerns that cannot be represented by a plan graph:
//! - stepping decode/transform stages frame-by-frame,
//! - managing sink activation/reuse through sink sessions,
//! - synchronizing hot control data with active stages.
//!
//! # Control/Data Split
//!
//! Runtime execution is intentionally split into two planes:
//! - Control plane: command handlers mutate runner/session state and stage controls.
//! - Data plane: [`runner`] produces audio blocks and pushes them into sink runtime.
//!
//! This split keeps high-frequency data flow off the actor command path while still
//! allowing deterministic state transitions.

pub(crate) mod dsp;
pub(crate) mod runner;
pub(crate) mod sink_session;
