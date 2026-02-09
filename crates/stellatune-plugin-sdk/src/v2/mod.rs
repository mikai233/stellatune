//! V2 SDK scaffolding.
//!
//! This module is intentionally small during migration bootstrap.
//! Concrete descriptor/factory/instance shims will be moved here incrementally.

pub mod export;
pub mod instance;
pub mod update;

pub use export::*;
pub use instance::*;
pub use update::*;
