use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{error, info};

use stellatune_core::{LibraryCommand, LibraryEvent};
use stellatune_runtime as global_runtime;

use crate::worker::{LibraryWorker, WorkerDeps};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::collections::HashSet;

#[derive(Clone)]
pub struct LibraryHandle {
    cmd_tx: mpsc::Sender<LibraryCommand>,
    events: Arc<EventHub>,
    plugins_dir: PathBuf,
    db_path: PathBuf,
}

impl LibraryHandle {
    pub async fn send_command(&self, cmd: LibraryCommand) -> std::result::Result<(), String> {
        self.cmd_tx
            .send(cmd)
            .await
            .map_err(|_| "library command channel closed".to_string())
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LibraryEvent> {
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

        stellatune_plugins::runtime::handle::shared_runtime_service()
            .set_plugin_enabled(&plugin_id, enabled)
            .await;

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

pub async fn start_library(db_path: String) -> Result<LibraryHandle> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<LibraryCommand>(256);
    let events = Arc::new(EventHub::new());
    let thread_events = Arc::clone(&events);
    let (init_tx, init_rx) = oneshot::channel::<Result<()>>();

    let plugins_dir = PathBuf::from(&db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("plugins");
    let db_path = PathBuf::from(db_path);

    let plugins_dir_thread = plugins_dir.clone();
    let db_path_thread = db_path.clone();

    global_runtime::spawn(async move {
        if let Err(e) = library_task_main(
            db_path_thread,
            cmd_rx,
            thread_events,
            init_tx,
            plugins_dir_thread,
        )
        .await
        {
            error!("library task exited with error: {e:?}");
        }
    });

    let init = init_rx.await.context("library init channel closed")?;
    init?;

    Ok(LibraryHandle {
        cmd_tx,
        events,
        plugins_dir,
        db_path,
    })
}

async fn library_task_main(
    db_path: PathBuf,
    mut cmd_rx: mpsc::Receiver<LibraryCommand>,
    events: Arc<EventHub>,
    init_tx: oneshot::Sender<Result<()>>,
    plugins_dir: PathBuf,
) -> Result<()> {
    info!("library task started");

    if let Err(e) = ensure_parent_dir(&db_path) {
        let _ = init_tx.send(Err(e));
        return Ok(());
    }

    let deps = match WorkerDeps::new(&db_path, Arc::clone(&events), plugins_dir).await {
        Ok(v) => v,
        Err(e) => {
            let _ = init_tx.send(Err(e));
            return Ok(());
        }
    };
    let _ = init_tx.send(Ok(()));

    let mut worker = LibraryWorker::new(deps);

    while let Some(cmd) = cmd_rx.recv().await {
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

    info!("library task exiting");

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
    tx: broadcast::Sender<LibraryEvent>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<LibraryEvent> {
        self.tx.subscribe()
    }

    pub(crate) fn emit(&self, event: LibraryEvent) {
        let _ = self.tx.send(event);
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
