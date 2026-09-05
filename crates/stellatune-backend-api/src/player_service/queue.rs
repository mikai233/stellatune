//! Authoritative queue order, navigation intent, and successor replenishment.
//!
//! The mutex serializes decisions and mailbox admission, never source resolution
//! or pipeline preparation. A newer intent cancels older resolution and checks
//! its intent token again before submitting work. Audible boundaries update the
//! observed cursor without rolling back a newer requested target.

use super::{
    error::PlayerServiceError,
    identity::TrackId,
    service::PlayerService,
    state::{PlaybackQueueRecord, RepeatMode},
};
use rand::seq::SliceRandom;
use std::{
    sync::{Arc, atomic::Ordering},
    task::Poll,
    time::Duration,
};
use stellatune_audio::playback::{
    control::{AdvanceOutcome, SwitchOptions},
    event::{PlaybackEvent, PlaybackState},
};
use stellatune_audio_core::playback::PlaybackItemId;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct QueueSnapshot {
    pub items: Vec<PlaybackQueueRecord>,
    pub order: Vec<PlaybackItemId>,
    pub current_item_id: Option<PlaybackItemId>,
    pub requested_item_id: Option<PlaybackItemId>,
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub revision: u64,
}

pub(super) struct QueueCoordinator {
    loaded: bool,
    items: Vec<PlaybackQueueRecord>,
    order: Vec<PlaybackItemId>,
    current: Option<PlaybackItemId>,
    requested: Option<PlaybackItemId>,
    repeat: RepeatMode,
    shuffle: bool,
    revision: u64,
    navigation: CancellationToken,
    prewarm: CancellationToken,
    prepared: Option<PlaybackItemId>,
    stopped: bool,
}

impl Default for QueueCoordinator {
    fn default() -> Self {
        Self {
            loaded: false,
            items: vec![],
            order: vec![],
            current: None,
            requested: None,
            repeat: RepeatMode::Off,
            shuffle: false,
            revision: 0,
            navigation: CancellationToken::new(),
            prewarm: CancellationToken::new(),
            prepared: None,
            stopped: true,
        }
    }
}

impl QueueCoordinator {
    fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            items: self.items.clone(),
            order: self.order.clone(),
            current_item_id: self.current,
            requested_item_id: self.requested,
            repeat_mode: self.repeat,
            shuffle: self.shuffle,
            revision: self.revision,
        }
    }

    fn reorder(&mut self) {
        self.order = self.items.iter().map(|item| item.item_id).collect();
        if self.shuffle {
            self.order.shuffle(&mut rand::rng());
            if let Some(index) = self.order.iter().position(|id| Some(*id) == self.requested) {
                self.order.swap(0, index);
            }
        }
    }

    fn successor(&self, from: PlaybackItemId, automatic: bool) -> Option<PlaybackItemId> {
        if automatic && self.repeat == RepeatMode::One {
            return Some(from);
        }
        let index = self.order.iter().position(|id| *id == from)?;
        self.order.get(index + 1).copied().or_else(|| {
            (self.repeat == RepeatMode::All)
                .then(|| self.order.first().copied())
                .flatten()
        })
    }

    fn invalidate_prewarm(&mut self) {
        self.prewarm.cancel();
        self.prewarm = CancellationToken::new();
        self.prepared = None;
    }

    fn select(&mut self, item: PlaybackItemId) -> CancellationToken {
        self.navigation.cancel();
        self.navigation = CancellationToken::new();
        self.revision = self.revision.wrapping_add(1);
        self.requested = Some(item);
        self.stopped = false;
        self.navigation.clone()
    }
}

impl PlayerService {
    pub fn subscribe_queue(&self) -> tokio::sync::broadcast::Receiver<u64> {
        self.queue_events.subscribe()
    }

    pub(super) async fn load_queue(&self) -> Result<(), PlayerServiceError> {
        let mut queue = self.queue.lock().await;
        if queue.loaded {
            return Ok(());
        }
        let stored = self.catalog.load_state().await?;
        queue.items = stored.queue;
        queue.current = stored.current_item_id;
        queue.requested = stored.current_item_id;
        queue.repeat = stored.repeat_mode;
        queue.shuffle = stored.shuffle_enabled;
        queue.stopped = stored.current_item_id.is_none();
        queue.reorder();
        queue.loaded = true;
        Ok(())
    }

    pub async fn queue_snapshot(&self) -> Result<QueueSnapshot, PlayerServiceError> {
        self.load_queue().await?;
        Ok(self.queue.lock().await.snapshot())
    }

    pub async fn append_queue(
        self: &Arc<Self>,
        tracks: Vec<TrackId>,
    ) -> Result<QueueSnapshot, PlayerServiceError> {
        self.load_queue().await?;
        let mut queue = self.queue.lock().await;
        let items = self.catalog.append_items(&tracks).await?;
        // Appending preserves the existing shuffle traversal and item identities.
        let mut extra: Vec<_> = items.iter().map(|item| item.item_id).collect();
        if queue.shuffle {
            extra.shuffle(&mut rand::rng());
        }
        queue.order.extend(extra);
        queue.items.extend(items);
        queue.revision = queue.revision.wrapping_add(1);
        let _ = self.queue_events.send(queue.revision);
        let snapshot = queue.snapshot();
        drop(queue);
        self.refresh_next().await?;
        Ok(snapshot)
    }

    pub async fn replace_queue(
        self: &Arc<Self>,
        tracks: Vec<TrackId>,
    ) -> Result<QueueSnapshot, PlayerServiceError> {
        self.load_queue().await?;
        // Validate before destroying the previous queue or playback session.
        self.catalog.validate_tracks(&tracks).await?;
        let mut queue = self.queue.lock().await;
        queue.navigation.cancel();
        queue.invalidate_prewarm();
        self.controller.stop().await?;
        queue.items = self.catalog.replace_items(&tracks).await?;
        queue.current = None;
        queue.requested = None;
        queue.stopped = true;
        queue.revision = queue.revision.wrapping_add(1);
        let _ = self.queue_events.send(queue.revision);
        queue.reorder();
        Ok(queue.snapshot())
    }

    pub async fn remove_queue_items(
        self: &Arc<Self>,
        ids: Vec<PlaybackItemId>,
    ) -> Result<QueueSnapshot, PlayerServiceError> {
        self.load_queue().await?;
        let mut queue = self.queue.lock().await;
        if queue.requested.is_some_and(|id| ids.contains(&id))
            || queue.current.is_some_and(|id| ids.contains(&id))
        {
            queue.navigation.cancel();
            queue.invalidate_prewarm();
            self.controller.stop().await?;
            queue.current = None;
            queue.requested = None;
            queue.stopped = true;
        }
        self.catalog.remove_items(&ids).await?;
        queue.items.retain(|item| !ids.contains(&item.item_id));
        queue.order.retain(|id| !ids.contains(id));
        queue.revision = queue.revision.wrapping_add(1);
        let _ = self.queue_events.send(queue.revision);
        let snapshot = queue.snapshot();
        drop(queue);
        self.refresh_next().await?;
        Ok(snapshot)
    }

    pub async fn set_queue_mode(
        self: &Arc<Self>,
        repeat: RepeatMode,
        shuffle: bool,
    ) -> Result<QueueSnapshot, PlayerServiceError> {
        self.load_queue().await?;
        let mut queue = self.queue.lock().await;
        self.catalog.save_queue_mode(repeat, shuffle).await?;
        if queue.shuffle != shuffle {
            queue.shuffle = shuffle;
            queue.reorder();
        }
        queue.repeat = repeat;
        queue.revision = queue.revision.wrapping_add(1);
        let _ = self.queue_events.send(queue.revision);
        let snapshot = queue.snapshot();
        drop(queue);
        self.refresh_next().await?;
        Ok(snapshot)
    }

    pub async fn select_item(
        self: &Arc<Self>,
        id: PlaybackItemId,
        options: SwitchOptions,
    ) -> Result<(), PlayerServiceError> {
        self.load_queue().await?;
        let mut queue = self.queue.lock().await;
        let track = queue
            .items
            .iter()
            .find(|item| item.item_id == id)
            .ok_or(PlayerServiceError::PlaybackItemNotFound(id))?
            .track_id;
        let cancellation = queue.select(id);
        if queue.shuffle {
            queue.reorder();
        }
        let _ = self.queue_events.send(queue.revision);
        self.navigate(queue, id, track, cancellation, Some(options))
            .await
    }

    pub async fn next(self: &Arc<Self>) -> Result<(), PlayerServiceError> {
        self.move_cursor(false).await
    }

    pub async fn previous(self: &Arc<Self>) -> Result<(), PlayerServiceError> {
        self.move_cursor(true).await
    }

    async fn move_cursor(self: &Arc<Self>, previous: bool) -> Result<(), PlayerServiceError> {
        self.load_queue().await?;
        let mut queue = self.queue.lock().await;
        let target = queue.requested.or(queue.current).and_then(|current| {
            if previous {
                let index = queue.order.iter().position(|id| *id == current)?;
                index
                    .checked_sub(1)
                    .and_then(|index| queue.order.get(index))
                    .copied()
                    .or_else(|| {
                        (queue.repeat == RepeatMode::All)
                            .then(|| queue.order.last().copied())
                            .flatten()
                    })
            } else {
                queue.successor(current, false)
            }
        });
        let Some(id) = target else {
            if previous {
                return Ok(());
            }
            queue.navigation.cancel();
            queue.invalidate_prewarm();
            queue.stopped = true;
            self.controller.stop().await?;
            return Ok(());
        };
        let track = queue
            .items
            .iter()
            .find(|item| item.item_id == id)
            .expect("order belongs to queue")
            .track_id;
        let cancellation = queue.select(id);
        let _ = self.queue_events.send(queue.revision);
        self.navigate(queue, id, track, cancellation, None).await
    }

    async fn navigate(
        self: &Arc<Self>,
        mut queue: tokio::sync::MutexGuard<'_, QueueCoordinator>,
        id: PlaybackItemId,
        track: TrackId,
        cancellation: CancellationToken,
        options: Option<SwitchOptions>,
    ) -> Result<(), PlayerServiceError> {
        let explicit = options.is_some();
        let options = options.unwrap_or_default();
        let outcome = self.controller.advance_to_next(id, options).await?;
        if outcome == AdvanceOutcome::Accepted
            || (outcome == AdvanceOutcome::AlreadyCurrent && !explicit)
        {
            drop(queue);
            return Ok(());
        }
        queue.invalidate_prewarm();
        // This command is admitted in every state and only cancels the next slot.
        self.controller.set_next(None).await?;
        drop(queue);
        let item = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(PlayerServiceError::Superseded),
            result = self.materialize_item(id, track) => result?,
        };
        let queue = self.queue.lock().await;
        if cancellation.is_cancelled() || queue.requested != Some(id) {
            return Err(PlayerServiceError::Superseded);
        }
        // Lattice ask admits synchronously on its first poll. Hold the decision
        // lock through admission, then release it before waiting for preparation.
        let command = self.controller.switch_to(item, options);
        tokio::pin!(command);
        let admitted = futures_util::poll!(&mut command);
        drop(queue);
        let result = match admitted {
            Poll::Ready(result) => result,
            Poll::Pending => tokio::select! {
                biased;
                _ = cancellation.cancelled() => return Err(PlayerServiceError::Superseded),
                result = &mut command => result,
            },
        };
        if cancellation.is_cancelled() {
            return Err(PlayerServiceError::Superseded);
        }
        result?;
        Ok(())
    }

    pub async fn play(self: &Arc<Self>) -> Result<(), PlayerServiceError> {
        self.load_queue().await?;
        let snapshot = self.controller.snapshot().await?;
        if matches!(snapshot.state, PlaybackState::Idle | PlaybackState::Failed) {
            let queue = self.queue.lock().await;
            let target = queue
                .requested
                .or(queue.current)
                .or_else(|| queue.order.first().copied());
            drop(queue);
            if let Some(target) = target {
                return self.select_item(target, SwitchOptions::default()).await;
            }
        }
        self.controller.play().await?;
        Ok(())
    }

    pub async fn stop(self: &Arc<Self>) -> Result<(), PlayerServiceError> {
        self.load_queue().await?;
        let mut queue = self.queue.lock().await;
        queue.navigation.cancel();
        queue.invalidate_prewarm();
        queue.stopped = true;
        queue.requested = queue.current;
        queue.revision = queue.revision.wrapping_add(1);
        let _ = self.queue_events.send(queue.revision);
        self.controller.stop().await?;
        Ok(())
    }

    /// Replenishes one successor off the caller's critical path.
    pub(super) async fn refresh_next(self: &Arc<Self>) -> Result<(), PlayerServiceError> {
        let mut queue = self.queue.lock().await;
        if !queue.loaded || queue.stopped || queue.requested != queue.current {
            return Ok(());
        }
        let target = queue
            .current
            .and_then(|current| queue.successor(current, true));
        if queue.prepared == target {
            return Ok(());
        }
        queue.invalidate_prewarm();
        // Clear stale work before beginning potentially slow source resolution.
        self.controller.set_next(None).await?;
        let Some(id) = target else {
            return Ok(());
        };
        let track = queue
            .items
            .iter()
            .find(|item| item.item_id == id)
            .expect("successor belongs to queue")
            .track_id;
        queue.prepared = Some(id);
        let cancellation = queue.prewarm.clone();
        drop(queue);
        let service = Arc::clone(self);
        tokio::spawn(async move {
            let result = service
                .prepare_successor(id, track, cancellation.clone())
                .await;
            if let Err(error) = result {
                if cancellation.is_cancelled() {
                    return;
                }
                let mut queue = service.queue.lock().await;
                if queue.prepared == Some(id) {
                    queue.prepared = None;
                }
                tracing::warn!(%error, "successor preparation failed");
            }
        });
        Ok(())
    }

    async fn prepare_successor(
        &self,
        id: PlaybackItemId,
        track: TrackId,
        cancellation: CancellationToken,
    ) -> Result<(), PlayerServiceError> {
        let item = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(PlayerServiceError::Superseded),
            result = self.materialize_item(id, track) => result?,
        };
        let queue = self.queue.lock().await;
        if cancellation.is_cancelled() || queue.prepared != Some(id) {
            return Err(PlayerServiceError::Superseded);
        }
        let command = self.controller.set_next(Some(item));
        tokio::pin!(command);
        let admitted = futures_util::poll!(&mut command);
        drop(queue);
        match admitted {
            Poll::Ready(result) => result?,
            Poll::Pending => command.await?,
        }
        Ok(())
    }

    pub fn start_state_writer(self: &Arc<Self>) {
        if self.state_writer_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let weak = Arc::downgrade(self);
        let mut events = self.controller.subscribe_events();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                let event = tokio::select! {
                    event = events.recv() => match event {
                        Ok(event) => Some(event),
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => None,
                    },
                    _ = interval.tick() => None,
                };
                let Some(service) = weak.upgrade() else {
                    break;
                };
                if let Err(error) = service.observe_queue_event(event).await {
                    tracing::warn!(%error, "queue state synchronization failed");
                }
            }
        });
    }

    async fn observe_queue_event(
        self: &Arc<Self>,
        event: Option<PlaybackEvent>,
    ) -> Result<(), PlayerServiceError> {
        self.load_queue().await?;
        if let Some(PlaybackEvent::TrackChanged { item_id }) = event {
            let mut queue = self.queue.lock().await;
            if !queue.stopped && queue.items.iter().any(|item| item.item_id == item_id) {
                queue.current = Some(item_id);
                if queue.requested == Some(item_id) || queue.prepared == Some(item_id) {
                    queue.requested = Some(item_id);
                    // A boundary consumes the old successor, including repeat-one.
                    queue.prepared = None;
                }
                queue.revision = queue.revision.wrapping_add(1);
                let _ = self.queue_events.send(queue.revision);
            }
            drop(queue);
            self.refresh_next().await?;
        } else if let Some(PlaybackEvent::PlaybackEnded { item_id }) = event {
            let mut queue = self.queue.lock().await;
            if queue.current == Some(item_id) && queue.requested == Some(item_id) {
                queue.stopped = true;
                queue.invalidate_prewarm();
            }
        }
        // Serialize persistence with edits so a removed item cannot be saved back.
        if event.is_none()
            || matches!(
                event,
                Some(PlaybackEvent::TrackChanged { .. } | PlaybackEvent::PlaybackEnded { .. })
            )
        {
            let mut queue = self.queue.lock().await;
            let snapshot = self.controller.snapshot().await?;
            if event.is_none() && !queue.stopped {
                if let Some(item_id) = snapshot.current_item_id {
                    if queue.current != Some(item_id)
                        && (queue.requested == Some(item_id) || queue.prepared == Some(item_id))
                    {
                        queue.current = Some(item_id);
                        queue.requested = Some(item_id);
                        queue.prepared = None;
                        queue.revision = queue.revision.wrapping_add(1);
                        let _ = self.queue_events.send(queue.revision);
                    }
                } else if snapshot.state == PlaybackState::Idle && queue.requested == queue.current
                {
                    queue.stopped = true;
                    queue.invalidate_prewarm();
                }
            }
            let playing = snapshot.state == PlaybackState::Playing;
            self.catalog.save_runtime_state(snapshot, playing).await?;
            drop(queue);
            if event.is_none() {
                self.refresh_next().await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_one_only_repeats_automatic_succession_and_repeat_all_wraps() {
        let ids: Vec<_> = (1..=3).map(|id| PlaybackItemId::new(id).unwrap()).collect();
        let mut queue = QueueCoordinator {
            order: ids.clone(),
            repeat: RepeatMode::One,
            ..Default::default()
        };
        assert_eq!(queue.successor(ids[0], true), Some(ids[0]));
        assert_eq!(queue.successor(ids[0], false), Some(ids[1]));
        assert_eq!(queue.successor(ids[2], false), None);
        queue.repeat = RepeatMode::All;
        assert_eq!(queue.successor(ids[2], true), Some(ids[0]));
        queue.repeat = RepeatMode::Off;
        queue.shuffle = true;
        assert_eq!(queue.successor(ids[2], true), None);
    }
}
