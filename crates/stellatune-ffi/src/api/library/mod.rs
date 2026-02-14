use std::sync::{Arc, OnceLock};

use crate::frb_generated::StreamSink;
use anyhow::{Result, anyhow};
use stellatune_runtime as global_runtime;
use tracing::debug;

use stellatune_backend_api::library::LibraryService;
use stellatune_core::{LibraryEvent, PlaylistLite, TrackLite};

static LIBRARY_SERVICE: OnceLock<Arc<LibraryService>> = OnceLock::new();
static LIBRARY_INIT_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn shared_library() -> Result<Arc<LibraryService>> {
    LIBRARY_SERVICE
        .get()
        .map(Arc::clone)
        .ok_or_else(|| anyhow!("library is not initialized; call create_library first"))
}

pub async fn create_library(db_path: String) -> Result<()> {
    if LIBRARY_SERVICE.get().is_some() {
        return Ok(());
    }

    let lock = LIBRARY_INIT_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;

    if LIBRARY_SERVICE.get().is_some() {
        return Ok(());
    }

    let service = Arc::new(LibraryService::new(db_path).await?);
    let _ = LIBRARY_SERVICE.set(service);
    Ok(())
}

pub async fn library_add_root(path: String) -> Result<()> {
    shared_library()?.add_root(path).await
}

pub async fn library_remove_root(path: String) -> Result<()> {
    shared_library()?.remove_root(path).await
}

pub async fn library_delete_folder(path: String) -> Result<()> {
    shared_library()?.delete_folder(path).await
}

pub async fn library_restore_folder(path: String) -> Result<()> {
    shared_library()?.restore_folder(path).await
}

pub async fn library_list_excluded_folders() -> Result<Vec<String>> {
    shared_library()?.list_excluded_folders().await
}

pub async fn library_scan_all() -> Result<()> {
    shared_library()?.scan_all().await
}

pub async fn library_scan_all_force() -> Result<()> {
    shared_library()?.scan_all_force().await
}

pub async fn library_list_roots() -> Result<Vec<String>> {
    shared_library()?.list_roots().await
}

pub async fn library_list_folders() -> Result<Vec<String>> {
    shared_library()?.list_folders().await
}

pub async fn library_list_tracks(
    folder: String,
    recursive: bool,
    query: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackLite>> {
    shared_library()?
        .list_tracks(folder, recursive, query, limit, offset)
        .await
}

pub async fn library_search(query: String, limit: i64, offset: i64) -> Result<Vec<TrackLite>> {
    shared_library()?.search(query, limit, offset).await
}

pub async fn library_list_playlists() -> Result<Vec<PlaylistLite>> {
    shared_library()?.list_playlists().await
}

pub async fn library_create_playlist(name: String) -> Result<()> {
    shared_library()?.create_playlist(name).await
}

pub async fn library_rename_playlist(id: i64, name: String) -> Result<()> {
    shared_library()?.rename_playlist(id, name).await
}

pub async fn library_delete_playlist(id: i64) -> Result<()> {
    shared_library()?.delete_playlist(id).await
}

pub async fn library_list_playlist_tracks(
    playlist_id: i64,
    query: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackLite>> {
    shared_library()?
        .list_playlist_tracks(playlist_id, query, limit, offset)
        .await
}

pub async fn library_add_track_to_playlist(playlist_id: i64, track_id: i64) -> Result<()> {
    shared_library()?
        .add_track_to_playlist(playlist_id, track_id)
        .await
}

pub async fn library_add_tracks_to_playlist(playlist_id: i64, track_ids: Vec<i64>) -> Result<()> {
    shared_library()?
        .add_tracks_to_playlist(playlist_id, track_ids)
        .await
}

pub async fn library_remove_track_from_playlist(playlist_id: i64, track_id: i64) -> Result<()> {
    shared_library()?
        .remove_track_from_playlist(playlist_id, track_id)
        .await
}

pub async fn library_remove_tracks_from_playlist(
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<()> {
    shared_library()?
        .remove_tracks_from_playlist(playlist_id, track_ids)
        .await
}

pub async fn library_move_track_in_playlist(
    playlist_id: i64,
    track_id: i64,
    new_index: i64,
) -> Result<()> {
    shared_library()?
        .move_track_in_playlist(playlist_id, track_id, new_index)
        .await
}

pub async fn library_list_liked_track_ids() -> Result<Vec<i64>> {
    shared_library()?.list_liked_track_ids().await
}

pub async fn library_set_track_liked(track_id: i64, liked: bool) -> Result<()> {
    shared_library()?.set_track_liked(track_id, liked).await
}

pub fn library_events(sink: StreamSink<LibraryEvent>) -> Result<()> {
    let mut rx = shared_library()?.subscribe_events();
    global_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if sink.add(event).is_err() {
                        debug!("library_events stream sink closed");
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    debug!(skipped, "library_events lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    Ok(())
}

pub async fn library_plugin_disable(plugin_id: String) -> Result<()> {
    shared_library()?.plugin_disable(plugin_id).await
}

pub async fn library_plugin_enable(plugin_id: String) -> Result<()> {
    shared_library()?.plugin_enable(plugin_id).await
}

pub async fn library_plugin_apply_state() -> Result<()> {
    shared_library()?.plugin_apply_state().await
}

pub async fn library_plugin_apply_state_status_json() -> Result<String> {
    Ok(shared_library()?.plugin_apply_state_status_json().await)
}

pub async fn library_list_disabled_plugin_ids() -> Result<Vec<String>> {
    shared_library()?.list_disabled_plugin_ids().await
}
