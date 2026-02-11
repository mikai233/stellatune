use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use tokio::sync::mpsc;
use tracing::{error, info};

use stellatune_core::{LibraryCommand, LibraryEvent};

use crate::worker::{LibraryWorker, WorkerDeps};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::collections::HashSet;

#[derive(Clone)]
pub struct LibraryHandle {
    cmd_tx: Sender<LibraryCommand>,
    events: Arc<EventHub>,
    plugins_dir: PathBuf,
    db_path: PathBuf,
}

impl LibraryHandle {
    pub fn send_command(&self, cmd: LibraryCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn subscribe_events(&self) -> Receiver<LibraryEvent> {
        self.events.subscribe()
    }

    pub fn plugins_dir_path(&self) -> &Path {
        &self.plugins_dir
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub async fn plugin_set_enabled(&self, plugin_id: String, enabled: bool) -> Result<()> {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return Ok(());
        }

        let mut disabled = load_disabled_plugin_ids(&self.db_path).await?;
        if enabled {
            disabled.remove(&plugin_id);
        } else {
            disabled.insert(plugin_id.clone());
        }
        persist_disabled_plugin_ids(&self.db_path, &disabled).await?;

        if let Ok(service) = stellatune_plugins::shared_runtime_service().lock() {
            service.set_plugin_enabled(&plugin_id, enabled);
        }

        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    pub async fn plugin_set_enabled(&self, plugin_id: String, enabled: bool) -> Result<()> {
        let _ = (plugin_id, enabled);
        Ok(())
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub async fn list_disabled_plugin_ids(&self) -> Result<Vec<String>> {
        let mut out = load_disabled_plugin_ids(&self.db_path)
            .await?
            .into_iter()
            .collect::<Vec<_>>();
        out.sort();
        Ok(out)
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    pub async fn list_disabled_plugin_ids(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

pub fn start_library(db_path: String) -> Result<LibraryHandle> {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<LibraryCommand>();
    let events = Arc::new(EventHub::new());
    let thread_events = Arc::clone(&events);
    let (init_tx, init_rx) = crossbeam_channel::bounded::<Result<()>>(1);

    let plugins_dir = PathBuf::from(&db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("plugins");
    let db_path = PathBuf::from(db_path);

    let plugins_dir_thread = plugins_dir.clone();
    let db_path_thread = db_path.clone();

    thread::Builder::new()
        .name("stellatune-library".to_string())
        .spawn(move || {
            if let Err(e) = library_thread_main(
                db_path_thread,
                cmd_rx,
                thread_events,
                init_tx,
                plugins_dir_thread,
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
        db_path,
    })
}

fn library_thread_main(
    db_path: PathBuf,
    cmd_rx: Receiver<LibraryCommand>,
    events: Arc<EventHub>,
    init_tx: Sender<Result<()>>,
    plugins_dir: PathBuf,
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

        if let Err(e) = ensure_parent_dir(&db_path) {
            let _ = init_tx.send(Err(e));
            return Ok::<_, anyhow::Error>(());
        }

        let deps = match WorkerDeps::new(&db_path, Arc::clone(&events), plugins_dir).await {
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

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
async fn persist_disabled_plugin_ids(db_path: &Path, disabled: &HashSet<String>) -> Result<()> {
    let pool = crate::worker::db::open_state_db_pool(db_path).await?;
    crate::worker::db::replace_disabled_plugin_ids(&pool, disabled).await?;
    pool.close().await;
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
async fn load_disabled_plugin_ids(db_path: &Path) -> Result<HashSet<String>> {
    let pool = crate::worker::db::open_state_db_pool(db_path).await?;
    let out = crate::worker::db::list_disabled_plugin_ids(&pool).await?;
    pool.close().await;
    Ok(out)
}
