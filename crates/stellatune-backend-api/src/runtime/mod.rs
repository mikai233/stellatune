use std::sync::OnceLock;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::sync::{Arc, Mutex};
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
};

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::LocalTime;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugins::{PluginManager, default_host_vtable};

mod bus;
mod control;
mod router;
#[cfg(all(
    test,
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
mod tests;
mod types;

#[derive(Clone)]
struct TeeWriter {
    file: Option<Arc<Mutex<std::fs::File>>>,
}

impl TeeWriter {
    fn new(file: Option<Arc<Mutex<std::fs::File>>>) -> Self {
        Self { file }
    }
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = io::stderr().write_all(buf);
        if let Some(file) = &self.file
            && let Ok(mut guard) = file.lock()
        {
            let _ = guard.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = io::stderr().flush();
        if let Some(file) = &self.file
            && let Ok(mut guard) = file.lock()
        {
            let _ = guard.flush();
        }
        Ok(())
    }
}

fn tracing_log_file_path() -> PathBuf {
    std::env::temp_dir().join("stellatune").join("tracing.log")
}

fn open_tracing_log_file() -> Option<Arc<Mutex<std::fs::File>>> {
    let path = tracing_log_file_path();
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return None;
        }
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()?;
    Some(Arc::new(Mutex::new(file)))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub type SharedPlugins = Arc<Mutex<PluginManager>>;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub type SharedPlugins = ();

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn shared_plugins() -> SharedPlugins {
    static SHARED: OnceLock<SharedPlugins> = OnceLock::new();
    SHARED
        .get_or_init(|| Arc::new(Mutex::new(PluginManager::new(default_host_vtable()))))
        .clone()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn shared_plugins() -> SharedPlugins {
    ()
}

pub fn init_tracing() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                EnvFilter::new(
                    "warn,stellatune_backend_api=debug,stellatune_audio=debug,stellatune_decode=debug,stellatune_output=debug,stellatune_library=debug,stellatune_plugins=info",
                )
            } else {
                EnvFilter::new("info")
            }
        });
        let file = open_tracing_log_file();
        let writer = move || TeeWriter::new(file.clone());
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_timer(LocalTime::rfc_3339())
            .with_target(true)
            .with_thread_names(true)
            .with_thread_ids(true)
            .with_writer(writer)
            .try_init()
            .ok();
    });
}

pub fn register_plugin_runtime_engine(engine: stellatune_audio::EngineHandle) {
    router::register_plugin_runtime_engine(engine);
}

pub fn register_plugin_runtime_library(library: stellatune_library::LibraryHandle) {
    router::register_plugin_runtime_library(library);
}

pub fn subscribe_plugin_runtime_events_global()
-> crossbeam_channel::Receiver<stellatune_core::PluginRuntimeEvent> {
    router::subscribe_plugin_runtime_events_global()
}
