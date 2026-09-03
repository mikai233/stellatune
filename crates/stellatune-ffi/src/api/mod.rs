pub mod dlna;
pub mod library;
pub mod player;
pub mod runtime;

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
