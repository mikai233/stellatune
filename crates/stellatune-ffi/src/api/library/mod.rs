use std::sync::{Arc, OnceLock};

use crate::frb_generated::StreamSink;
use anyhow::{Result, anyhow};
use tracing::debug;

use crate::api::events::LibraryEvent;
use stellatune_backend_api::library::LibraryService;
use stellatune_backend_api::player_service::catalog::PlayerCatalog;
use stellatune_backend_api::player_service::service::PlayerService;
use stellatune_backend_api::runtime::{TypeScriptSourceResolverFactory, shared_typescript_runtime};
use stellatune_library::{PlaylistLite, TrackLite};

static LIBRARY_SERVICE: OnceLock<Arc<LibraryService>> = OnceLock::new();
static PLAYER_SERVICE: OnceLock<Arc<PlayerService>> = OnceLock::new();
static LIBRARY_INIT_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

pub async fn library_rebuild_required(
    db_path: String,
) -> Result<bool, crate::api::error::AppError> {
    stellatune_library::rebuild::required(std::path::Path::new(&db_path))
        .await
        .map_err(|error| crate::api::error::AppError::capture("library_rebuild_required", error))
}

pub async fn library_rebuild(db_path: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<()> = async {
        let _guard = LIBRARY_INIT_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await;
        if LIBRARY_SERVICE.get().is_some() || PLAYER_SERVICE.get().is_some() {
            anyhow::bail!("Close the library before rebuilding it");
        }
        stellatune_library::rebuild::rebuild(std::path::Path::new(&db_path)).await
    }
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("library_rebuild", error))
}

pub(crate) fn shared_library_if_initialized() -> Option<Arc<LibraryService>> {
    LIBRARY_SERVICE.get().map(Arc::clone)
}

pub(crate) fn shared_player_service() -> Result<Arc<PlayerService>> {
    PLAYER_SERVICE
        .get()
        .map(Arc::clone)
        .ok_or_else(|| anyhow!("player service is not initialized; call create_library first"))
}

fn shared_library() -> Result<Arc<LibraryService>> {
    LIBRARY_SERVICE
        .get()
        .map(Arc::clone)
        .ok_or_else(|| anyhow!("library is not initialized; call create_library first"))
}

pub async fn create_library(db_path: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        if LIBRARY_SERVICE.get().is_some() {
            return Ok(());
        }

        let lock = LIBRARY_INIT_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
        let _guard = lock.lock().await;

        if LIBRARY_SERVICE.get().is_some() {
            return Ok(());
        }

        let service = Arc::new(LibraryService::new(db_path.clone()).await?);
        let catalog_result = async {
            let catalog = PlayerCatalog::open(&db_path).await?;
            catalog.ensure_local_source().await?;
            Ok::<_, anyhow::Error>(catalog)
        }
        .await;
        let catalog = match catalog_result {
            Ok(catalog) => catalog,
            Err(error) => {
                let _ = service.handle().shutdown().await;
                return Err(error);
            },
        };
        let player_service = Arc::new(PlayerService::new(
            catalog,
            stellatune_backend_api::runtime::shared_playback_controller(),
            Arc::new(service.handle().clone()),
            Arc::new(TypeScriptSourceResolverFactory::new(
                shared_typescript_runtime(),
            )),
        ));
        let _ =
            stellatune_backend_api::runtime::install_player_service(Arc::clone(&player_service));
        let _ = PLAYER_SERVICE.set(player_service);
        let _ = LIBRARY_SERVICE.set(service);
        Ok(())
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("create_library", error))
}

pub(crate) async fn shutdown_library() {
    if let Some(service) = LIBRARY_SERVICE.get()
        && let Err(error) = service.handle().shutdown().await
    {
        tracing::warn!(%error, "library shutdown failed");
    }
    if let Some(service) = PLAYER_SERVICE.get() {
        service.close_catalog().await;
    }
}

pub async fn library_add_root(path: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move { shared_library()?.add_root(path).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_add_root", error))
}

pub async fn library_remove_root(path: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.remove_root(path).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_remove_root", error))
}

pub async fn library_delete_folder(path: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.delete_folder(path).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_delete_folder", error))
}

pub async fn library_restore_folder(path: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.restore_folder(path).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_restore_folder", error))
}

pub async fn library_list_excluded_folders() -> Result<Vec<String>, crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.list_excluded_folders().await }).await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_list_excluded_folders", error)
    })
}

pub async fn library_scan_all() -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move { shared_library()?.scan_all().await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_scan_all", error))
}

pub async fn library_scan_all_force() -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move { shared_library()?.scan_all_force().await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_scan_all_force", error))
}

pub async fn library_list_roots() -> Result<Vec<String>, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move { shared_library()?.list_roots().await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_list_roots", error))
}

pub async fn library_list_folders() -> Result<Vec<String>, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move { shared_library()?.list_folders().await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_list_folders", error))
}

pub async fn library_list_tracks(
    folder: String,
    recursive: bool,
    query: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackLite>, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        shared_library()?
            .list_tracks(folder, recursive, query, limit, offset)
            .await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("library_list_tracks", error))
}

pub async fn library_search(
    query: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackLite>, crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.search(query, limit, offset).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_search", error))
}

pub async fn library_list_playlists() -> Result<Vec<PlaylistLite>, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move { shared_library()?.list_playlists().await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_list_playlists", error))
}

pub async fn library_create_playlist(name: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.create_playlist(name).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_create_playlist", error))
}

pub async fn library_rename_playlist(
    id: i64,
    name: String,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.rename_playlist(id, name).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_rename_playlist", error))
}

pub async fn library_delete_playlist(id: i64) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.delete_playlist(id).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_delete_playlist", error))
}

pub async fn library_list_playlist_tracks(
    playlist_id: i64,
    query: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackLite>, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        shared_library()?
            .list_playlist_tracks(playlist_id, query, limit, offset)
            .await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_list_playlist_tracks", error)
    })
}

pub async fn library_add_track_to_playlist(
    playlist_id: i64,
    track_id: i64,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        shared_library()?
            .add_track_to_playlist(playlist_id, track_id)
            .await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_add_track_to_playlist", error)
    })
}

pub async fn library_add_tracks_to_playlist(
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        shared_library()?
            .add_tracks_to_playlist(playlist_id, track_ids)
            .await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_add_tracks_to_playlist", error)
    })
}

pub async fn library_remove_track_from_playlist(
    playlist_id: i64,
    track_id: i64,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        shared_library()?
            .remove_track_from_playlist(playlist_id, track_id)
            .await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_remove_track_from_playlist", error)
    })
}

pub async fn library_remove_tracks_from_playlist(
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        shared_library()?
            .remove_tracks_from_playlist(playlist_id, track_ids)
            .await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_remove_tracks_from_playlist", error)
    })
}

pub async fn library_move_track_in_playlist(
    playlist_id: i64,
    track_id: i64,
    new_index: i64,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        shared_library()?
            .move_track_in_playlist(playlist_id, track_id, new_index)
            .await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_move_track_in_playlist", error)
    })
}

pub async fn library_list_liked_track_ids() -> Result<Vec<i64>, crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.list_liked_track_ids().await }).await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_list_liked_track_ids", error)
    })
}

pub async fn library_set_track_liked(
    track_id: i64,
    liked: bool,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.set_track_liked(track_id, liked).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_set_track_liked", error))
}

pub fn library_events(sink: StreamSink<LibraryEvent>) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (|| {
        let mut rx = shared_library()?.subscribe_events();
        crate::background_runtime::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if sink.add(event.into()).is_err() {
                            debug!("library_events stream sink closed");
                            break;
                        }
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        debug!(skipped, "library_events lagged");
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    })();
    result.map_err(|error| crate::api::error::AppError::capture("library_events", error))
}

pub async fn library_plugin_disable(plugin_id: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.plugin_disable(plugin_id).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_plugin_disable", error))
}

pub async fn library_plugin_enable(plugin_id: String) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.plugin_enable(plugin_id).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("library_plugin_enable", error))
}

pub async fn library_plugin_apply_state() -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { shared_library()?.plugin_apply_state().await }).await;
    result
        .map_err(|error| crate::api::error::AppError::capture("library_plugin_apply_state", error))
}

pub async fn library_plugin_apply_state_status_json() -> Result<String, crate::api::error::AppError>
{
    let result: anyhow::Result<_> =
        (async move { Ok(shared_library()?.plugin_apply_state_status_json().await) }).await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_plugin_apply_state_status_json", error)
    })
}

pub async fn library_list_disabled_plugin_ids() -> Result<Vec<String>, crate::api::error::AppError>
{
    let result: anyhow::Result<_> =
        (async move { shared_library()?.list_disabled_plugin_ids().await }).await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("library_list_disabled_plugin_ids", error)
    })
}
