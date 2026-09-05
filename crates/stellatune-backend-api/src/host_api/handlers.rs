use super::{
    HostApiState,
    error::ApiError,
    model::{PlayerCommand, Repeat},
};
use crate::player_service::{
    identity::TrackId, plugin_tracks::ensure_provider_track, queue::QueueSnapshot,
    state::RepeatMode,
};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    response::Sse,
    response::sse::{Event, KeepAlive},
};
use futures_util::stream::{self, Stream, StreamExt};
use serde_json::{Value, json};
use std::{convert::Infallible, time::Duration};
use stellatune_audio::playback::{control::SwitchOptions, event::PlaybackEvent};
use stellatune_audio_core::playback::{MediaTime, PlaybackItemId};

fn identity(raw: &str) -> Result<u64, ApiError> {
    raw.parse::<u64>()
        .ok()
        .filter(|id| *id > 0 && *id <= i64::MAX as u64 && id.to_string() == raw)
        .ok_or_else(|| ApiError::bad("identity must be a positive decimal string"))
}
fn track(raw: &str) -> Result<TrackId, ApiError> {
    Ok(TrackId::new(identity(raw)?)?)
}
fn item(raw: &str) -> Result<PlaybackItemId, ApiError> {
    PlaybackItemId::new(identity(raw)?).ok_or_else(|| ApiError::bad("invalid item ID"))
}

pub(super) async fn state(State(state): State<HostApiState>) -> Result<Json<Value>, ApiError> {
    let snapshot = state.controller.snapshot().await?;
    let track = match snapshot.current_item_id {
        Some(item) => state
            .service
            .track_id_for_item(item)
            .await
            .ok()
            .map(|id| id.get().to_string()),
        None => None,
    };
    Ok(Json(json!({
        "state": format!("{:?}", snapshot.state).to_lowercase(),
        "itemId": snapshot.current_item_id.map(|id| id.get().to_string()),
        "trackId": track,
        "positionMs": snapshot.consumed_position.as_millis(),
        "durationMs": snapshot.duration.map(|duration| duration.as_millis()),
    })))
}

fn queue_value(queue: QueueSnapshot) -> Value {
    json!({
        "items": queue.items.iter().map(|item| json!({"itemId": item.item_id.get().to_string(), "trackId": item.track_id.get().to_string()})).collect::<Vec<_>>(),
        "order": queue.order.iter().map(|id| id.get().to_string()).collect::<Vec<_>>(),
        "currentItemId": queue.current_item_id.map(|id| id.get().to_string()),
        "requestedItemId": queue.requested_item_id.map(|id| id.get().to_string()),
        "repeat": format!("{:?}", queue.repeat_mode).to_lowercase(),
        "shuffle": queue.shuffle, "revision": queue.revision.to_string(),
    })
}
pub(super) async fn queue(State(state): State<HostApiState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(queue_value(state.service.queue_snapshot().await?)))
}

pub(super) async fn command(
    State(state): State<HostApiState>,
    body: Result<Json<PlayerCommand>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(command) = body.map_err(ApiError::bad)?;
    let service = &state.service;
    let mut result = json!({});
    match command {
        PlayerCommand::Play => service.play().await?,
        PlayerCommand::Pause => state.controller.pause().await?,
        PlayerCommand::Stop => service.stop().await?,
        PlayerCommand::Seek { position_ms } => {
            state
                .controller
                .seek(MediaTime::from_millis(position_ms))
                .await?;
        },
        PlayerCommand::Next => service.next().await?,
        PlayerCommand::Previous => service.previous().await?,
        PlayerCommand::SelectItem { item_id } => {
            service
                .select_item(item(&item_id)?, SwitchOptions::default())
                .await?
        },
        PlayerCommand::AppendQueue { track_ids } => {
            let tracks = track_ids
                .iter()
                .map(|id| track(id))
                .collect::<Result<Vec<_>, _>>()?;
            result = queue_value(service.append_queue(tracks).await?);
        },
        PlayerCommand::ReplaceQueue { track_ids } => {
            let tracks = track_ids
                .iter()
                .map(|id| track(id))
                .collect::<Result<Vec<_>, _>>()?;
            result = queue_value(service.replace_queue(tracks).await?);
        },
        PlayerCommand::RemoveQueueItems { item_ids } => {
            result = queue_value(
                service
                    .remove_queue_items(
                        item_ids
                            .iter()
                            .map(|id| item(id))
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                    .await?,
            );
        },
        PlayerCommand::SetQueueMode { repeat, shuffle } => {
            let repeat = match repeat {
                Repeat::Off => RepeatMode::Off,
                Repeat::All => RepeatMode::All,
                Repeat::One => RepeatMode::One,
            };
            result = queue_value(service.set_queue_mode(repeat, shuffle).await?);
        },
        PlayerCommand::PlayTrack { track_id } => {
            let snapshot = service.append_queue(vec![track(&track_id)?]).await?;
            let id = snapshot.items.last().expect("one appended item").item_id;
            service.select_item(id, SwitchOptions::default()).await?;
            result = json!({"itemId": id.get().to_string(), "trackId": track_id});
        },
        command @ (PlayerCommand::PlayProviderTrack { .. }
        | PlayerCommand::EnqueueProviderTrack { .. }) => {
            let play = matches!(&command, PlayerCommand::PlayProviderTrack { .. });
            let (PlayerCommand::PlayProviderTrack { track }
            | PlayerCommand::EnqueueProviderTrack { track }) = command
            else {
                unreachable!()
            };
            let track = ensure_provider_track(
                service,
                state.plugins.clone(),
                &track.plugin_id,
                &track.capability_id,
                &track.provider_id,
                &track.provider_key,
            )
            .await?;
            let snapshot = service.append_queue(vec![track]).await?;
            let id = snapshot.items.last().expect("one appended item").item_id;
            if play {
                service.select_item(id, SwitchOptions::default()).await?;
            }
            result = json!({"itemId": id.get().to_string(), "trackId": track.get().to_string()});
        },
    }
    Ok(Json(result))
}

fn event_value(event: PlaybackEvent) -> Value {
    match event {
        PlaybackEvent::StateChanged(state) => {
            json!({"type": "stateChanged", "state": format!("{state:?}").to_lowercase()})
        },
        PlaybackEvent::TrackChanged { item_id } => {
            json!({"type": "trackChanged", "itemId": item_id.get().to_string()})
        },
        PlaybackEvent::PlaybackEnded { item_id } => {
            json!({"type": "playbackEnded", "itemId": item_id.get().to_string()})
        },
        PlaybackEvent::Position { item_id, position } => {
            json!({"type": "position", "itemId": item_id.get().to_string(), "positionMs": position.as_millis()})
        },
        PlaybackEvent::Buffering { item_id, active } => {
            json!({"type": "buffering", "itemId": item_id.get().to_string(), "active": active})
        },
        PlaybackEvent::Failed(error) => json!({"type": "failed", "message": format!("{error:?}")}),
    }
}
fn sse(value: Value) -> Result<Event, Infallible> {
    Ok(Event::default().event("event").data(value.to_string()))
}

pub(super) async fn events(
    State(state): State<HostApiState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let playback = state.controller.subscribe_events();
    let queues = state.service.subscribe_queue();
    let initial = json!({"type": "snapshot", "state": self::state(State(state.clone())).await?.0, "queue": queue(State(state.clone())).await?.0});
    let stream = stream::unfold(
        (playback, queues, state.shutdown, state.service),
        |(mut playback, mut queues, shutdown, service)| async move {
            let value = tokio::select! {
                biased;
                _ = shutdown.cancelled() => return None,
                event = playback.recv() => match event {
                    Ok(event) => event_value(event),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => json!({"type": "resync"}),
                    Err(_) => return None,
                },
                queue = queues.recv() => match queue {
                    Ok(_) => match service.queue_snapshot().await {
                        Ok(queue) => json!({"type": "queueChanged", "queue": queue_value(queue)}),
                        Err(_) => json!({"type": "resync"}),
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => json!({"type": "resync"}),
                    Err(_) => return None,
                },
            };
            Some((sse(value), (playback, queues, shutdown, service)))
        },
    );
    Ok(
        Sse::new(stream::once(async move { sse(initial) }).chain(stream))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15))),
    )
}
