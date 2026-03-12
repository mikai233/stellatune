mod core;
#[cfg(not(target_os = "windows"))]
mod shell;
pub mod state;

pub use core::GuiApp;
#[cfg(not(target_os = "windows"))]
pub use shell::GuiShell;
