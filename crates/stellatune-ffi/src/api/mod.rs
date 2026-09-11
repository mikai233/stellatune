pub mod diagnostics;
pub mod dlna;
pub mod error;
pub mod events;
pub mod library;
pub mod media_catalog;
pub mod player;
pub mod runtime;

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
    stellatune_backend_api::runtime::init_tracing();
}
