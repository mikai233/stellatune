use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use tokio::sync::broadcast;
use tracing::info;

use crate::{LibraryEvent, PlaylistLite, TrackLite};
use stellatune_runtime::tokio_actor::{ActorRef, CallError, Handler, Message, spawn_actor};

use crate::worker::{LibraryWorker, WorkerDeps};

mod service_actor;

use self::service_actor::LibraryServiceActor;
use self::service_actor::handlers::command::{
    AddRootMessage, AddTrackToPlaylistMessage, AddTracksToPlaylistMessage, CreatePlaylistMessage,
    DeleteFolderMessage, DeletePlaylistMessage, MoveTrackInPlaylistMessage, RemoveRootMessage,
    RemoveTrackFromPlaylistMessage, RemoveTracksFromPlaylistMessage, RenamePlaylistMessage,
    RestoreFolderMessage, ScanAllForceMessage, ScanAllMessage, SetTrackLikedMessage,
    ShutdownMessage,
};
use self::service_actor::handlers::query::{
    ListExcludedFoldersMessage, ListFoldersMessage, ListLikedTrackIdsMessage,
    ListPlaylistTracksMessage, ListPlaylistsMessage, ListRootsMessage, ListTracksMessage,
    SearchTracksMessage,
};

use std::collections::HashSet;

#[derive(Clone)]
pub struct LibraryHandle {
    actor_ref: ActorRef<LibraryServiceActor>,
    events: Arc<EventHub>,
    plugins_dir: PathBuf,
    db_path: PathBuf,
}

impl LibraryHandle {
    const QUERY_TIMEOUT: Duration = Duration::from_secs(15);

    fn cast_command<M>(&self, message: M) -> Result<(), String>
    where
        M: Message<Response = ()>,
        LibraryServiceActor: Handler<M>,
    {
        self.actor_ref
            .cast(message)
            .map_err(|_| "library command channel closed".to_string())
    }

    pub async fn add_root(&self, path: String) -> Result<(), String> {
        self.cast_command(AddRootMessage { path })
    }

    pub async fn remove_root(&self, path: String) -> Result<(), String> {
        self.cast_command(RemoveRootMessage { path })
    }

    pub async fn delete_folder(&self, path: String) -> Result<(), String> {
        self.cast_command(DeleteFolderMessage { path })
    }

    pub async fn restore_folder(&self, path: String) -> Result<(), String> {
        self.cast_command(RestoreFolderMessage { path })
    }

    pub async fn scan_all(&self) -> Result<(), String> {
        self.cast_command(ScanAllMessage)
    }

    pub async fn scan_all_force(&self) -> Result<(), String> {
        self.cast_command(ScanAllForceMessage)
    }

    pub async fn create_playlist(&self, name: String) -> Result<(), String> {
        self.cast_command(CreatePlaylistMessage { name })
    }

    pub async fn rename_playlist(&self, id: i64, name: String) -> Result<(), String> {
        self.cast_command(RenamePlaylistMessage { id, name })
    }

    pub async fn delete_playlist(&self, id: i64) -> Result<(), String> {
        self.cast_command(DeletePlaylistMessage { id })
    }

    pub async fn add_track_to_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
    ) -> Result<(), String> {
        self.cast_command(AddTrackToPlaylistMessage {
            playlist_id,
            track_id,
        })
    }

    pub async fn add_tracks_to_playlist(
        &self,
        playlist_id: i64,
        track_ids: Vec<i64>,
    ) -> Result<(), String> {
        self.cast_command(AddTracksToPlaylistMessage {
            playlist_id,
            track_ids,
        })
    }

    pub async fn remove_track_from_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
    ) -> Result<(), String> {
        self.cast_command(RemoveTrackFromPlaylistMessage {
            playlist_id,
            track_id,
        })
    }

    pub async fn remove_tracks_from_playlist(
        &self,
        playlist_id: i64,
        track_ids: Vec<i64>,
    ) -> Result<(), String> {
        self.cast_command(RemoveTracksFromPlaylistMessage {
            playlist_id,
            track_ids,
        })
    }

    pub async fn move_track_in_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
        new_index: i64,
    ) -> Result<(), String> {
        self.cast_command(MoveTrackInPlaylistMessage {
            playlist_id,
            track_id,
            new_index,
        })
    }

    pub async fn set_track_liked(&self, track_id: i64, liked: bool) -> Result<(), String> {
        self.cast_command(SetTrackLikedMessage { track_id, liked })
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.cast_command(ShutdownMessage)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LibraryEvent> {
        self.events.subscribe()
    }

    pub fn plugins_dir_path(&self) -> &Path {
        &self.plugins_dir
    }

    pub async fn list_roots(&self) -> Result<Vec<String>> {
        let result = self
            .actor_ref
            .call(ListRootsMessage, Self::QUERY_TIMEOUT)
            .await
            .map_err(map_call_error)?;
        result.map_err(|e| anyhow!(e))
    }

    pub async fn list_folders(&self) -> Result<Vec<String>> {
        let result = self
            .actor_ref
            .call(ListFoldersMessage, Self::QUERY_TIMEOUT)
            .await
            .map_err(map_call_error)?;
        result.map_err(|e| anyhow!(e))
    }

    pub async fn list_excluded_folders(&self) -> Result<Vec<String>> {
        let result = self
            .actor_ref
            .call(ListExcludedFoldersMessage, Self::QUERY_TIMEOUT)
            .await
            .map_err(map_call_error)?;
        result.map_err(|e| anyhow!(e))
    }

    pub async fn list_tracks(
        &self,
        folder: String,
        recursive: bool,
        query: String,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TrackLite>> {
        let result = self
            .actor_ref
            .call(
                ListTracksMessage {
                    folder,
                    recursive,
                    query,
                    limit,
                    offset,
                },
                Self::QUERY_TIMEOUT,
            )
            .await
            .map_err(map_call_error)?;
        result.map_err(|e| anyhow!(e))
    }

    pub async fn search(&self, query: String, limit: i64, offset: i64) -> Result<Vec<TrackLite>> {
        let result = self
            .actor_ref
            .call(
                SearchTracksMessage {
                    query,
                    limit,
                    offset,
                },
                Self::QUERY_TIMEOUT,
            )
            .await
            .map_err(map_call_error)?;
        result.map_err(|e| anyhow!(e))
    }

    pub async fn list_playlists(&self) -> Result<Vec<PlaylistLite>> {
        let result = self
            .actor_ref
            .call(ListPlaylistsMessage, Self::QUERY_TIMEOUT)
            .await
            .map_err(map_call_error)?;
        result.map_err(|e| anyhow!(e))
    }

    pub async fn list_playlist_tracks(
        &self,
        playlist_id: i64,
        query: String,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TrackLite>> {
        let result = self
            .actor_ref
            .call(
                ListPlaylistTracksMessage {
                    playlist_id,
                    query,
                    limit,
                    offset,
                },
                Self::QUERY_TIMEOUT,
            )
            .await
            .map_err(map_call_error)?;
        result.map_err(|e| anyhow!(e))
    }

    pub async fn list_liked_track_ids(&self) -> Result<Vec<i64>> {
        let result = self
            .actor_ref
            .call(ListLikedTrackIdsMessage, Self::QUERY_TIMEOUT)
            .await
            .map_err(map_call_error)?;
        result.map_err(|e| anyhow!(e))
    }

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

        Ok(())
    }

    pub async fn list_disabled_plugin_ids(&self) -> Result<Vec<String>> {
        let mut out = load_disabled_plugin_ids(&self.db_path)
            .await?
            .into_iter()
            .collect::<Vec<_>>();
        out.sort();
        Ok(out)
    }
}

fn map_call_error(err: CallError) -> anyhow::Error {
    match err {
        CallError::Timeout => {
            anyhow!("library query timed out")
        },
        _ => anyhow!("library actor unavailable"),
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
    let (actor_ref, _join) = spawn_actor(LibraryServiceActor::new(worker, Arc::clone(&events)));
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

async fn persist_disabled_plugin_ids(db_path: &Path, disabled: &HashSet<String>) -> Result<()> {
    let pool = crate::worker::db::open_state_db_pool(db_path).await?;
    crate::worker::db::replace_disabled_plugin_ids(&pool, disabled).await?;
    pool.close().await;
    Ok(())
}

async fn load_disabled_plugin_ids(db_path: &Path) -> Result<HashSet<String>> {
    let pool = crate::worker::db::open_state_db_pool(db_path).await?;
    let out = crate::worker::db::list_disabled_plugin_ids(&pool).await?;
    pool.close().await;
    Ok(out)
}
