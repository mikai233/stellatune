//! Pipeline graph and assembly abstractions.
//!
//! Public items in this module define how decode pipelines are planned,
//! transformed, and materialized for runtime execution.

/// Pipeline assembly contracts.
pub mod assembly;
/// Capability descriptors, typed factories, and registry validation.
pub mod capability;
/// Strongly typed playback intent, source negotiation, and executable plans.
pub mod plan;
pub(crate) mod runtime;
