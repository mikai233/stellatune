//! Queue identities and navigation owned by the backend player service.

use crate::api::library::shared_player_service;
use anyhow::{Result, anyhow};
use stellatune_audio::playback::control::SwitchOptions;
use stellatune_audio_core::playback::PlaybackItemId;
use stellatune_backend_api::player_service::{
    identity::TrackId, queue::QueueSnapshot, state::RepeatMode,
};

#[derive(Clone)]
pub struct QueueEntry {
    pub item_id: u64,
    pub track_id: u64,
    pub local_library_track_id: Option<i64>,
    pub local_path: Option<String>,
    pub local_metadata: Option<stellatune_library::TrackLite>,
}

#[derive(Clone, Copy)]
pub enum QueueRepeatMode {
    Off,
    All,
    One,
}

pub struct PlaybackQueue {
    pub items: Vec<QueueEntry>,
    pub order: Vec<u64>,
    pub current_item_id: Option<u64>,
    pub requested_item_id: Option<u64>,
    pub repeat_mode: QueueRepeatMode,
    pub shuffle: bool,
    pub revision: u64,
}

async fn project(snapshot: QueueSnapshot) -> Result<PlaybackQueue> {
    let service = shared_player_service()?;
    let tracks: Vec<_> = snapshot.items.iter().map(|item| item.track_id).collect();
    let metadata = service.queue_local_metadata(&tracks).await?;
    let mut items = Vec::with_capacity(snapshot.items.len());
    for item in snapshot.items {
        let local = metadata.get(&item.track_id);
        let local_library_track_id = local.map(|(id, _)| *id);
        let local_path = local
            .and_then(|(_, path)| path.as_ref())
            .map(|track| track.path.clone());
        items.push(QueueEntry {
            item_id: item.item_id.get(),
            track_id: item.track_id.get(),
            local_library_track_id,
            local_path,
            local_metadata: local.and_then(|(_, track)| track.clone()),
        });
    }
    Ok(PlaybackQueue {
        items,
        order: snapshot
            .order
            .into_iter()
            .map(PlaybackItemId::get)
            .collect(),
        current_item_id: snapshot.current_item_id.map(PlaybackItemId::get),
        requested_item_id: snapshot.requested_item_id.map(PlaybackItemId::get),
        repeat_mode: match snapshot.repeat_mode {
            RepeatMode::Off => QueueRepeatMode::Off,
            RepeatMode::All => QueueRepeatMode::All,
            RepeatMode::One => QueueRepeatMode::One,
        },
        shuffle: snapshot.shuffle,
        revision: snapshot.revision,
    })
}

pub async fn playback_queue() -> Result<PlaybackQueue> {
    project(shared_player_service()?.queue_snapshot().await?).await
}

pub async fn replace_queue(track_ids: Vec<u64>) -> Result<PlaybackQueue> {
    let ids = track_ids
        .into_iter()
        .map(TrackId::new)
        .collect::<Result<Vec<_>, _>>()?;
    project(shared_player_service()?.replace_queue(ids).await?).await
}

pub async fn append_queue(track_ids: Vec<u64>) -> Result<PlaybackQueue> {
    let ids = track_ids
        .into_iter()
        .map(TrackId::new)
        .collect::<Result<Vec<_>, _>>()?;
    project(shared_player_service()?.append_queue(ids).await?).await
}

pub async fn remove_queue_items(item_ids: Vec<u64>) -> Result<PlaybackQueue> {
    let ids = item_ids
        .into_iter()
        .map(|id| PlaybackItemId::new(id).ok_or_else(|| anyhow!("invalid playback item ID")))
        .collect::<Result<Vec<_>>>()?;
    project(shared_player_service()?.remove_queue_items(ids).await?).await
}

pub async fn set_queue_mode(repeat: QueueRepeatMode, shuffle: bool) -> Result<PlaybackQueue> {
    let repeat = match repeat {
        QueueRepeatMode::Off => RepeatMode::Off,
        QueueRepeatMode::All => RepeatMode::All,
        QueueRepeatMode::One => RepeatMode::One,
    };
    project(
        shared_player_service()?
            .set_queue_mode(repeat, shuffle)
            .await?,
    )
    .await
}

/// Returns false when a newer navigation intent superseded this request.
pub async fn select_queue_item(item_id: u64, autoplay: bool) -> Result<bool> {
    let id = PlaybackItemId::new(item_id).ok_or_else(|| anyhow!("invalid playback item ID"))?;
    navigation_result(
        shared_player_service()?
            .select_item(
                id,
                SwitchOptions {
                    autoplay,
                    ..Default::default()
                },
            )
            .await,
    )
}

pub async fn next_queue_item() -> Result<bool> {
    navigation_result(shared_player_service()?.next().await)
}
pub async fn previous_queue_item() -> Result<bool> {
    navigation_result(shared_player_service()?.previous().await)
}

fn navigation_result(
    result: Result<(), stellatune_backend_api::player_service::error::PlayerServiceError>,
) -> Result<bool> {
    use stellatune_backend_api::player_service::error::PlayerServiceError;
    match result {
        Ok(()) => Ok(true),
        Err(PlayerServiceError::Superseded) => Ok(false),
        Err(error) => Err(error.into()),
    }
}
