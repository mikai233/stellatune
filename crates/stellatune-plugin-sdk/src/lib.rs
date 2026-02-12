#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).

pub use stellatune_plugin_api::*;
pub use stellatune_plugin_protocol as protocol;

#[doc(hidden)]
pub mod __private {
    pub use serde_json;
}

mod codec;
mod control;
mod errors;
pub mod export;
#[doc(hidden)]
pub mod ffi_guard;
mod ffi_utils;
mod host;
pub mod instance;
mod macros;
mod metadata;
pub mod update;

pub use codec::*;
pub use control::*;
pub use errors::*;
pub use export::*;
pub use ffi_utils::*;
pub use host::*;
pub use metadata::*;
pub use update::*;

#[cfg(test)]
mod tests;
