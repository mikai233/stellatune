pub use stellatune_plugin_api::*;
pub use stellatune_plugin_protocol as protocol;

#[doc(hidden)]
pub mod __private {
    pub use serde_json;
}

mod codec;
mod control;
mod errors;
mod ffi_utils;
mod host;
mod macros;
mod metadata;
pub mod v2;

pub use codec::*;
pub use control::*;
pub use errors::*;
pub use ffi_utils::*;
pub use host::*;
pub use metadata::*;

#[cfg(test)]
mod tests;
