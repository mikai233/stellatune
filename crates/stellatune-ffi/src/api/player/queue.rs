//! Queue identities and navigation owned by the backend player service.

use crate::api::library::shared_player_service;
use crate::frb_generated::StreamSink;
use anyhow::{Result, anyhow};
use stellatune_audio::playback::control::SwitchOptions;
use stellatune_audio_core::playback::PlaybackItemId;
use stellatune_backend_api::player_service::metadata::TrackPresentation;
use stellatune_backend_api::player_service::{
    identity::TrackId, queue::QueueSnapshot, state::RepeatMode,
};

#[derive(Clone)]
pub struct QueueProviderTrack {
    pub catalog_capability_id: Option<String>,
    pub provider_id: String,
    pub provider_key: String,
    pub plugin_id: String,
    pub capability_id: String,
}

pub struct QueueMetadataUpdate {
    pub track_id: u64,
    pub metadata: TrackPresentation,
}

#[derive(Clone)]
pub struct QueueEntry {
    pub item_id: u64,
    pub track_id: u64,
    pub local_library_track_id: Option<i64>,
    pub local_path: Option<String>,
    pub local_metadata: Option<stellatune_library::TrackLite>,
    pub provider_track: Option<QueueProviderTrack>,
    pub metadata: Option<TrackPresentation>,
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
    let providers = service.queue_provider_metadata(&tracks).await?;
    let mut items = Vec::with_capacity(snapshot.items.len());
    for item in snapshot.items {
        let local = metadata.get(&item.track_id);
        let provider = providers.get(&item.track_id);
        let presentation = provider
            .as_ref()
            .and_then(|provider| provider.presentation.clone());
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
            provider_track: provider.map(|provider| QueueProviderTrack {
                catalog_capability_id: provider.catalog_capability_id.clone(),
                provider_id: provider.provider_id.clone(),
                provider_key: provider.provider_key.clone(),
                plugin_id: provider.plugin_id.clone(),
                capability_id: provider.capability_id.clone(),
            }),
            metadata: presentation,
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

pub async fn playback_queue() -> Result<PlaybackQueue, crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { project(shared_player_service()?.queue_snapshot().await?).await }).await;
    result.map_err(|error| crate::api::error::AppError::capture("playback_queue", error))
}

/// Subscribe before projecting the initial snapshot; lag also resynchronizes.
pub fn queue_events(sink: StreamSink<PlaybackQueue>) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (|| {
        let service = shared_player_service()?;
        let mut events = service.subscribe_queue();
        crate::background_runtime::spawn(async move {
            loop {
                let projection = match service.queue_snapshot().await {
                    Ok(snapshot) => project(snapshot).await,
                    Err(error) => Err(error.into()),
                };
                match projection {
                    Ok(snapshot) => {
                        if sink.add(snapshot).is_err() {
                            break;
                        }
                    },
                    Err(error) => {
                        tracing::warn!(%error, "queue event projection failed");
                        if sink.add_error(error).is_err() {
                            break;
                        }
                        // Keep this subscription alive across transient catalog errors.
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue;
                    },
                }
                match events.recv().await {
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {},
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Ok(())
    })();
    result.map_err(|error| crate::api::error::AppError::capture("queue_events", error))
}

pub async fn store_queue_metadata(
    updates: Vec<QueueMetadataUpdate>,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        let updates = updates
            .into_iter()
            .map(|update| Ok((TrackId::new(update.track_id)?, update.metadata)))
            .collect::<Result<Vec<_>>>()?;
        shared_player_service()?
            .store_track_presentations(&updates)
            .await?;
        Ok(())
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("store_queue_metadata", error))
}

pub async fn replace_queue(
    track_ids: Vec<u64>,
) -> Result<PlaybackQueue, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        let ids = track_ids
            .into_iter()
            .map(TrackId::new)
            .collect::<Result<Vec<_>, _>>()?;
        project(shared_player_service()?.replace_queue(ids).await?).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("replace_queue", error))
}

pub async fn append_queue(
    track_ids: Vec<u64>,
) -> Result<PlaybackQueue, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        let ids = track_ids
            .into_iter()
            .map(TrackId::new)
            .collect::<Result<Vec<_>, _>>()?;
        project(shared_player_service()?.append_queue(ids).await?).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("append_queue", error))
}

pub async fn remove_queue_items(
    item_ids: Vec<u64>,
) -> Result<PlaybackQueue, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        let ids = item_ids
            .into_iter()
            .map(|id| PlaybackItemId::new(id).ok_or_else(|| anyhow!("invalid playback item ID")))
            .collect::<Result<Vec<_>>>()?;
        project(shared_player_service()?.remove_queue_items(ids).await?).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("remove_queue_items", error))
}

pub async fn set_queue_mode(
    repeat: QueueRepeatMode,
    shuffle: bool,
) -> Result<PlaybackQueue, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
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
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("set_queue_mode", error))
}

/// Returns false when a newer navigation intent superseded this request.
pub async fn select_queue_item(
    item_id: u64,
    autoplay: bool,
) -> Result<bool, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
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
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("select_queue_item", error))
}

pub async fn next_queue_item() -> Result<bool, crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { navigation_result(shared_player_service()?.next().await) }).await;
    result.map_err(|error| crate::api::error::AppError::capture("next_queue_item", error))
}
pub async fn previous_queue_item() -> Result<bool, crate::api::error::AppError> {
    let result: anyhow::Result<_> =
        (async move { navigation_result(shared_player_service()?.previous().await) }).await;
    result.map_err(|error| crate::api::error::AppError::capture("previous_queue_item", error))
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
