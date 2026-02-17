#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).
#![allow(ambiguous_glob_reexports)]

pub mod dlna;
mod dlna_impl;
pub mod library;
pub mod player;
pub mod runtime;

pub use dlna::*;
pub use library::*;
pub use player::*;
pub use runtime::*;

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
