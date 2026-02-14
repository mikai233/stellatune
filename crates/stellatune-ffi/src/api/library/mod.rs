use crate::frb_generated::{RustOpaque, StreamSink};
use anyhow::Result;
use stellatune_runtime as global_runtime;
use tracing::debug;

use stellatune_backend_api::library::LibraryService;
use stellatune_core::LibraryEvent;

pub struct Library {
    service: LibraryService,
}

impl Library {
    async fn new(db_path: String) -> Result<Self> {
        let service = LibraryService::new(db_path).await?;
        Ok(Self { service })
    }
}

pub async fn create_library(db_path: String) -> Result<RustOpaque<Library>> {
    Ok(RustOpaque::new(Library::new(db_path).await?))
}

pub async fn library_add_root(library: RustOpaque<Library>, path: String) -> Result<()> {
    library.service.add_root(path).await
}

pub async fn library_remove_root(library: RustOpaque<Library>, path: String) -> Result<()> {
    library.service.remove_root(path).await
}

pub async fn library_delete_folder(library: RustOpaque<Library>, path: String) -> Result<()> {
    library.service.delete_folder(path).await
}

pub async fn library_restore_folder(library: RustOpaque<Library>, path: String) -> Result<()> {
    library.service.restore_folder(path).await
}

pub async fn library_list_excluded_folders(library: RustOpaque<Library>) -> Result<()> {
    library.service.list_excluded_folders().await
}

pub async fn library_scan_all(library: RustOpaque<Library>) -> Result<()> {
    library.service.scan_all().await
}

pub async fn library_scan_all_force(library: RustOpaque<Library>) -> Result<()> {
    library.service.scan_all_force().await
}

pub async fn library_list_roots(library: RustOpaque<Library>) -> Result<()> {
    library.service.list_roots().await
}

pub async fn library_list_folders(library: RustOpaque<Library>) -> Result<()> {
    library.service.list_folders().await
}

pub async fn library_list_tracks(
    library: RustOpaque<Library>,
    folder: String,
    recursive: bool,
    query: String,
    limit: i64,
    offset: i64,
) -> Result<()> {
    library
        .service
        .list_tracks(folder, recursive, query, limit, offset)
        .await
}

pub async fn library_search(
    library: RustOpaque<Library>,
    query: String,
    limit: i64,
    offset: i64,
) -> Result<()> {
    library.service.search(query, limit, offset).await
}

pub async fn library_list_playlists(library: RustOpaque<Library>) -> Result<()> {
    library.service.list_playlists().await
}

pub async fn library_create_playlist(library: RustOpaque<Library>, name: String) -> Result<()> {
    library.service.create_playlist(name).await
}

pub async fn library_rename_playlist(
    library: RustOpaque<Library>,
    id: i64,
    name: String,
) -> Result<()> {
    library.service.rename_playlist(id, name).await
}

pub async fn library_delete_playlist(library: RustOpaque<Library>, id: i64) -> Result<()> {
    library.service.delete_playlist(id).await
}

pub async fn library_list_playlist_tracks(
    library: RustOpaque<Library>,
    playlist_id: i64,
    query: String,
    limit: i64,
    offset: i64,
) -> Result<()> {
    library
        .service
        .list_playlist_tracks(playlist_id, query, limit, offset)
        .await
}

pub async fn library_add_track_to_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
) -> Result<()> {
    library
        .service
        .add_track_to_playlist(playlist_id, track_id)
        .await
}

pub async fn library_add_tracks_to_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<()> {
    library
        .service
        .add_tracks_to_playlist(playlist_id, track_ids)
        .await
}

pub async fn library_remove_track_from_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
) -> Result<()> {
    library
        .service
        .remove_track_from_playlist(playlist_id, track_id)
        .await
}

pub async fn library_remove_tracks_from_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<()> {
    library
        .service
        .remove_tracks_from_playlist(playlist_id, track_ids)
        .await
}

pub async fn library_move_track_in_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
    new_index: i64,
) -> Result<()> {
    library
        .service
        .move_track_in_playlist(playlist_id, track_id, new_index)
        .await
}

pub async fn library_list_liked_track_ids(library: RustOpaque<Library>) -> Result<()> {
    library.service.list_liked_track_ids().await
}

pub async fn library_set_track_liked(
    library: RustOpaque<Library>,
    track_id: i64,
    liked: bool,
) -> Result<()> {
    library.service.set_track_liked(track_id, liked).await
}

pub fn library_events(library: RustOpaque<Library>, sink: StreamSink<LibraryEvent>) -> Result<()> {
    let mut rx = library.service.subscribe_events();
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

pub async fn library_plugin_disable(library: RustOpaque<Library>, plugin_id: String) -> Result<()> {
    library.service.plugin_disable(plugin_id).await
}

pub async fn library_plugin_enable(library: RustOpaque<Library>, plugin_id: String) -> Result<()> {
    library.service.plugin_enable(plugin_id).await
}

pub async fn library_plugin_apply_state(library: RustOpaque<Library>) -> Result<()> {
    library.service.plugin_apply_state().await
}

pub async fn library_plugin_apply_state_status_json(library: RustOpaque<Library>) -> String {
    library.service.plugin_apply_state_status_json().await
}

pub async fn library_list_disabled_plugin_ids(library: RustOpaque<Library>) -> Result<Vec<String>> {
    library.service.list_disabled_plugin_ids().await
}
