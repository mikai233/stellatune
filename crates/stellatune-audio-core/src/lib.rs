//! Stable contracts shared by the playback runtime and audio adapters.
//!
//! Public types are addressed through their owning modules. The crate root
//! intentionally does not re-export them, so module ownership remains visible
//! at every dependency boundary.
#![deny(clippy::wildcard_imports)]

pub mod decoder;
pub mod error;
pub mod format;
pub mod playback;
pub mod sink;
pub mod source;
pub mod stage;
pub mod transform;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_root_does_not_flatten_module_api() {
        let source = include_str!("lib.rs");
        assert!(
            source
                .lines()
                .all(|line| !line.trim_start().starts_with("pub use ")),
            "audio-core types must remain under their owning public modules"
        );
    }
}
