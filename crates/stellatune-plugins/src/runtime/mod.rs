//! Runtime scaffolding.
//!
//! During migration this module hosts generation lifecycle, instance registry,
//! and config update orchestration for the new plugin execution model.

pub mod instance_registry;
pub mod lifecycle;
pub mod update;

pub use instance_registry::*;
pub use lifecycle::*;
pub use update::*;
