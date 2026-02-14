use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::broadcast;
use tracing::info;

use stellatune_core::{LibraryCommand, LibraryEvent};
use stellatune_runtime::tokio_actor::ActorRef;

use crate::worker::{LibraryWorker, WorkerDeps};

mod service_actor;

use self::service_actor::LibraryServiceActor;
use self::service_actor::handlers::command::LibraryCommandMessage;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::collections::HashSet;

#[derive(Clone)]
pub struct LibraryHandle {
    actor_ref: ActorRef<LibraryServiceActor>,
    events: Arc<EventHub>,
    plugins_dir: PathBuf,
    db_path: PathBuf,
}

impl LibraryHandle {
    pub async fn send_command(&self, cmd: LibraryCommand) -> std::result::Result<(), String> {
        self.actor_ref
            .cast(LibraryCommandMessage { command: cmd })
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
    let events = Arc::new(EventHub::new());

    let plugins_dir = PathBuf::from(&db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("plugins");
    let db_path = PathBuf::from(db_path);

    ensure_parent_dir(&db_path)?;
    let deps = WorkerDeps::new(&db_path, Arc::clone(&events), plugins_dir.clone()).await?;
    let worker = LibraryWorker::new(deps);
    let (actor_ref, _join) = stellatune_runtime::tokio_actor::spawn_actor(
        LibraryServiceActor::new(worker, Arc::clone(&events)),
    );
    info!("library actor started");

    Ok(LibraryHandle {
        actor_ref,
        events,
        plugins_dir,
        db_path,
    })
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
