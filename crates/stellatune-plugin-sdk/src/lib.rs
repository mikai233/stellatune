#![deny(unsafe_code)]

pub mod capabilities;
pub mod common;
pub mod error;
pub mod export;
pub mod host_stream;
pub mod hot_path;
pub mod http_client;
pub mod lifecycle;
pub mod prelude;
pub mod sidecar;

pub use capabilities::*;
pub use error::{SdkError, SdkResult};
pub use export::*;
pub use stellatune_world_bindings as guest_bindings;

#[doc(hidden)]
pub mod __private {
    pub use parking_lot;
    pub use stellatune_world_decoder;
    pub use stellatune_world_dsp;
    pub use stellatune_world_lyrics;
    pub use stellatune_world_output_sink;
    pub use stellatune_world_source;
}
