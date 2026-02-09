//! Parallel V2 runtime structure.
//!
//! This module is intentionally introduced alongside the legacy `PluginManager`
//! so backend call sites can migrate incrementally before old paths are removed.

mod capabilities;
mod capability_registry;
mod load;
mod service;
mod types;

pub use capabilities::*;
pub use load::*;
pub use service::*;
pub use stellatune_plugin_api::v2::{STELLATUNE_PLUGIN_API_VERSION_V2, StHostVTableV2};
pub use types::*;
