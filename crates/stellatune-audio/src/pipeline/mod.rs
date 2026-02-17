//! Pipeline graph and assembly abstractions.
//!
//! Public items in this module define how decode pipelines are planned,
//! transformed, and materialized for runtime execution.

/// Pipeline assembly contracts and mutation types.
pub mod assembly;
/// Transform graph model and mutation primitives.
pub mod graph;
pub(crate) mod runtime;
