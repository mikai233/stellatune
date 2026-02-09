use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use tokio::sync::mpsc;
use tracing::{error, info};

use stellatune_core::{LibraryCommand, LibraryEvent};

use crate::worker::{DisabledPluginIds, LibraryWorker, Plugins, WorkerDeps};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::collections::HashSet;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use arc_swap::ArcSwap;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugins::{PluginManager, default_host_vtable};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub type SharedPlugins = Arc<std::sync::Mutex<PluginManager>>;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub type SharedPlugins = ();

#[derive(Clone)]
pub struct LibraryHandle {
    cmd_tx: Sender<LibraryCommand>,
    events: Arc<EventHub>,
    plugins_dir: PathBuf,
    plugins: Plugins,
    disabled_plugin_ids: DisabledPluginIds,
}

impl LibraryHandle {
    pub fn send_command(&self, cmd: LibraryCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn subscribe_events(&self) -> Receiver<LibraryEvent> {
        self.events.subscribe()
    }

    pub fn plugins_reload_with_disabled(&self, dir: String, disabled_ids: Vec<String>) {
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        {
            let dir = if dir.trim().is_empty() {
                self.plugins_dir.clone()
            } else {
                PathBuf::from(dir)
            };

            let disabled = disabled_ids
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<HashSet<_>>();

            self.disabled_plugin_ids.store(Arc::new(disabled.clone()));

            if !dir.exists() {
                return;
            }

            let mut pm = match self.plugins.lock() {
                Ok(v) => v,
                Err(_) => {
                    self.events.emit(LibraryEvent::Log {
                        message: "plugins reload skipped: plugins mutex poisoned".to_string(),
                    });
                    return;
                }
            };
            pm.set_disabled_ids(disabled.clone());
            match unsafe { pm.load_dir_additive_filtered(&dir, &disabled) } {
                Ok(report) => {
                    self.events.emit(LibraryEvent::Log {
                        message: format!(
                            "library plugins reloaded from {}: loaded={} errors={}",
                            dir.display(),
                            report.loaded.len(),
                            report.errors.len()
                        ),
                    });
                    for e in report.errors {
                        self.events.emit(LibraryEvent::Log {
                            message: format!("plugin load error: {e:#}"),
                        });
                    }
                }
                Err(e) => {
                    self.events.emit(LibraryEvent::Log {
                        message: format!("library plugins reload failed: {e:#}"),
                    });
                }
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = (dir, disabled_ids);
        }
    }
}

pub fn start_library(db_path: String, disabled_plugin_ids: Vec<String>) -> Result<LibraryHandle> {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    let plugins: SharedPlugins = Arc::new(std::sync::Mutex::new(PluginManager::new(
        default_host_vtable(),
    )));

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let plugins: SharedPlugins = ();

    start_library_with_plugins(db_path, disabled_plugin_ids, plugins)
}

pub fn start_library_with_plugins(
    db_path: String,
    disabled_plugin_ids: Vec<String>,
    shared_plugins: SharedPlugins,
) -> Result<LibraryHandle> {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<LibraryCommand>();
    let events = Arc::new(EventHub::new());
    let thread_events = Arc::clone(&events);
    let (init_tx, init_rx) = crossbeam_channel::bounded::<Result<()>>(1);

    let plugins_dir = PathBuf::from(&db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("plugins");

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    let disabled_plugin_ids = disabled_plugin_ids
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<HashSet<_>>();

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    let disabled_plugin_ids: DisabledPluginIds =
        Arc::new(ArcSwap::from_pointee(disabled_plugin_ids));

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let disabled_plugin_ids: DisabledPluginIds = ();

    let plugins: Plugins = shared_plugins;

    let plugins_dir_thread = plugins_dir.clone();
    let plugins_thread = plugins.clone();
    let disabled_plugin_ids_thread = disabled_plugin_ids.clone();

    thread::Builder::new()
        .name("stellatune-library".to_string())
        .spawn(move || {
            if let Err(e) = library_thread_main(
                db_path,
                cmd_rx,
                thread_events,
                init_tx,
                plugins_dir_thread,
                plugins_thread,
                disabled_plugin_ids_thread,
            ) {
                error!("library thread exited with error: {e:?}");
            }
        })
        .context("failed to spawn stellatune-library thread")?;

    init_rx.recv().context("library init channel closed")??;

    Ok(LibraryHandle {
        cmd_tx,
        events,
        plugins_dir,
        plugins,
        disabled_plugin_ids,
    })
}

fn library_thread_main(
    db_path: String,
    cmd_rx: Receiver<LibraryCommand>,
    events: Arc<EventHub>,
    init_tx: Sender<Result<()>>,
    plugins_dir: PathBuf,
    plugins: Plugins,
    disabled_plugin_ids: DisabledPluginIds,
) -> Result<()> {
    info!("library thread started");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .enable_io()
        .thread_name("stellatune-library-rt")
        .build()
        .context("failed to build tokio runtime")?;

    rt.block_on(async move {
        let (cmd_async_tx, mut cmd_async_rx) = mpsc::unbounded_channel::<LibraryCommand>();

        // Bridge crossbeam -> tokio so external callers don't depend on tokio.
        tokio::task::spawn_blocking(move || {
            for cmd in cmd_rx.iter() {
                if cmd_async_tx.send(cmd).is_err() {
                    break;
                }
            }
        });

        let db_path = PathBuf::from(db_path);
        if let Err(e) = ensure_parent_dir(&db_path) {
            let _ = init_tx.send(Err(e));
            return Ok::<_, anyhow::Error>(());
        }

        let deps = match WorkerDeps::new(
            &db_path,
            Arc::clone(&events),
            plugins_dir,
            plugins,
            disabled_plugin_ids,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = init_tx.send(Err(e));
                return Ok::<_, anyhow::Error>(());
            }
        };
        let _ = init_tx.send(Ok(()));

        let mut worker = LibraryWorker::new(deps);

        while let Some(cmd) = cmd_async_rx.recv().await {
            let is_shutdown = matches!(cmd, LibraryCommand::Shutdown);
            if let Err(e) = worker.handle_command(cmd).await {
                events.emit(LibraryEvent::Error {
                    message: format!("{e:#}"),
                });
            }
            if is_shutdown {
                break;
            }
        }

        info!("library thread exiting");
        Ok::<_, anyhow::Error>(())
    })?;

    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create data dir: {}", parent.display()))?;
    Ok(())
}

pub(crate) struct EventHub {
    subscribers: std::sync::Mutex<Vec<Sender<LibraryEvent>>>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        Self {
            subscribers: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn subscribe(&self) -> Receiver<LibraryEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.push(tx);
        }
        rx
    }

    pub(crate) fn emit(&self, event: LibraryEvent) {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.retain(|tx| tx.send(event.clone()).is_ok());
        }
    }
}
