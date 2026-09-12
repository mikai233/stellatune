mod artist_names;
pub mod catalog;
pub mod cue;
pub mod metadata_provider;
pub mod rebuild;
pub mod service;
mod types;
mod worker;

pub use service::{LibraryHandle, start_library, start_library_with_metadata_provider};
pub use types::{LibraryEvent, PlaylistLite, TrackLite};
