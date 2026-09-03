use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use stellatune_audio::playback::control::{PlaybackController, SwitchOptions, SwitchTransition};
use stellatune_audio::playback::event::{PlaybackEvent, PlaybackState};
use stellatune_audio_core::{MediaHints, PlaybackItem, PlaybackItemId};

use super::catalog::PlayerCatalog;
use super::error::PlayerServiceError;
use super::identity::{
    ProviderId, ProviderTrackIdentityInput, SourceBinding, SourceInstanceId, TrackId,
};
use super::resolver::{
    LocalTrackResolver, SourceResolver, SourceResolverFactory, materialize_source,
};
use super::source::{ResolvedSourceSpec, SourceCatalogEntry, SourceResolverSpec, TrackOrigin};
use super::state::PlaybackStateRecord;
pub struct PlayerService {
    pub(super) catalog: PlayerCatalog,
    pub(super) controller: PlaybackController,
    local_resolver: Arc<dyn LocalTrackResolver>,
    resolver_factory: Arc<dyn SourceResolverFactory>,
    source_resolvers: tokio::sync::RwLock<HashMap<SourceInstanceId, Arc<dyn SourceResolver>>>,
    state_writer_started: AtomicBool,
}

impl PlayerService {
    pub fn new(
        catalog: PlayerCatalog,
        controller: PlaybackController,
        local_resolver: Arc<dyn LocalTrackResolver>,
        resolver_factory: Arc<dyn SourceResolverFactory>,
    ) -> Self {
        Self {
            catalog,
            controller,
            local_resolver,
            resolver_factory,
            source_resolvers: tokio::sync::RwLock::new(HashMap::new()),
            state_writer_started: AtomicBool::new(false),
        }
    }

    pub fn start_state_writer(&self) {
        if self.state_writer_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let catalog = self.catalog.clone();
        let controller = self.controller.clone();
        let mut events = controller.subscribe_events();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut dirty = false;
            let mut was_playing = false;
            loop {
                tokio::select! {
                    event = events.recv() => match event {
                        Ok(PlaybackEvent::StateChanged(state)) => {
                            was_playing = state == PlaybackState::Playing;
                            dirty = true;
                        },
                        Ok(PlaybackEvent::TrackChanged { .. }
                            | PlaybackEvent::PlaybackEnded { .. }
                            | PlaybackEvent::Position { .. }) => {
                            dirty = true;
                        },
                        Ok(_) => {},
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            dirty = true;
                        },
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    },
                    _ = interval.tick() => {
                        if !dirty {
                            continue;
                        }
                        if let Ok(snapshot) = controller.snapshot().await
                            && catalog.save_runtime_state(snapshot, was_playing).await.is_ok()
                        {
                            dirty = false;
                        }
                    },
                }
            }
        });
    }

    pub async fn register_resolver(
        &self,
        source: SourceInstanceId,
        resolver: Arc<dyn SourceResolver>,
    ) -> Result<(), PlayerServiceError> {
        let entry = self.catalog.source(source).await?;
        if !matches!(entry.binding, SourceBinding::Plugin { .. }) {
            return Err(PlayerServiceError::CatalogBindingMismatch);
        }
        self.source_resolvers.write().await.insert(source, resolver);
        Ok(())
    }

    pub async fn ensure_track(
        &self,
        input: ProviderTrackIdentityInput,
    ) -> Result<TrackId, PlayerServiceError> {
        self.catalog.ensure_track(input.try_into()?).await
    }

    pub async fn ensure_local_track(
        &self,
        library_track_id: i64,
    ) -> Result<TrackId, PlayerServiceError> {
        let source = self.catalog.ensure_local_source().await?;
        self.catalog
            .ensure_local_track(source, library_track_id)
            .await
    }

    pub async fn ensure_plugin_source(
        &self,
        provider_id: ProviderId,
        spec: SourceResolverSpec,
        resolver: Arc<dyn SourceResolver>,
    ) -> Result<SourceInstanceId, PlayerServiceError> {
        let source = self.catalog.ensure_plugin_source(provider_id, spec).await?;
        self.register_resolver(source, resolver).await?;
        Ok(source)
    }

    pub async fn switch_track(
        &self,
        track_id: TrackId,
        options: SwitchOptions,
    ) -> Result<PlaybackItemId, PlayerServiceError> {
        let item_id = self.catalog.enqueue(track_id).await?;
        let item = self.materialize_item(item_id, track_id).await?;
        self.controller.switch(item, options).await?;
        let snapshot = self.controller.snapshot().await?;
        self.catalog
            .save_runtime_state(snapshot, options.autoplay)
            .await?;
        Ok(item_id)
    }

    pub async fn queue_next(
        &self,
        track_id: TrackId,
    ) -> Result<PlaybackItemId, PlayerServiceError> {
        let item_id = self.catalog.enqueue(track_id).await?;
        let item = self.materialize_item(item_id, track_id).await?;
        self.controller.queue_next(item).await?;
        Ok(item_id)
    }

    pub async fn restore(&self) -> Result<PlaybackStateRecord, PlayerServiceError> {
        self.restore_with_policy(false).await
    }

    pub async fn restore_with_policy(
        &self,
        resume_playing_on_launch: bool,
    ) -> Result<PlaybackStateRecord, PlayerServiceError> {
        let state = self.catalog.load_state().await?;
        let Some(current_id) = state.current_item_id else {
            return Ok(state);
        };
        let current_index = state
            .queue
            .iter()
            .position(|entry| entry.item_id == current_id)
            .ok_or(PlayerServiceError::PlaybackStateInvariant)?;
        let current = &state.queue[current_index];
        let item = self
            .materialize_item(current.item_id, current.track_id)
            .await?;
        let capabilities = item.source.descriptor().capabilities;
        self.controller
            .switch(
                item,
                SwitchOptions {
                    autoplay: false,
                    transition: SwitchTransition::ImmediateWithDeClick,
                },
            )
            .await?;
        if state.position_ms > 0 && capabilities.byte_seekable && !capabilities.live {
            self.controller
                .seek(stellatune_audio_core::MediaTime::from_millis(
                    state.position_ms,
                ))
                .await?;
        }
        if let Some(next) = state.queue.get(current_index + 1) {
            let next_item = self.materialize_item(next.item_id, next.track_id).await?;
            self.controller.queue_next(next_item).await?;
        }
        if resume_playing_on_launch && state.was_playing {
            self.controller.play().await?;
        }
        Ok(state)
    }

    pub async fn local_path_for_item(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<Option<PathBuf>, PlayerServiceError> {
        let track_id = self.catalog.track_for_item(item_id).await?;
        let track = self.catalog.track(track_id).await?;
        match track.origin {
            TrackOrigin::LocalLibrary { library_track_id } => self
                .local_resolver
                .resolve_path(library_track_id)
                .await
                .map(Some),
            TrackOrigin::Provider(_) => Ok(None),
        }
    }

    pub async fn track_id_for_item(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<TrackId, PlayerServiceError> {
        self.catalog.track_for_item(item_id).await
    }

    pub async fn local_library_track_id_for_item(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<Option<i64>, PlayerServiceError> {
        let track_id = self.catalog.track_for_item(item_id).await?;
        let track = self.catalog.track(track_id).await?;
        Ok(match track.origin {
            TrackOrigin::LocalLibrary { library_track_id } => Some(library_track_id),
            TrackOrigin::Provider(_) => None,
        })
    }

    pub(super) async fn materialize_item(
        &self,
        item_id: PlaybackItemId,
        track_id: TrackId,
    ) -> Result<PlaybackItem, PlayerServiceError> {
        let track = self.catalog.track(track_id).await?;
        if track.tombstoned {
            return Err(PlayerServiceError::TrackUnavailable(track_id));
        }
        let source = self.catalog.source(track.source).await?;
        if source.tombstoned {
            return Err(PlayerServiceError::SourceUnavailable(track.source));
        }
        let (spec, required_decoder) = match track.origin {
            TrackOrigin::LocalLibrary { library_track_id } => {
                let path = self.local_resolver.resolve_path(library_track_id).await?;
                (
                    ResolvedSourceSpec::File {
                        path,
                        media: MediaHints::default(),
                    },
                    None,
                )
            },
            TrackOrigin::Provider(key) => {
                let resolver = self.resolver_for_source(&source).await?;
                (
                    resolver.resolve(&source, &key).await?,
                    resolver.required_decoder(),
                )
            },
        };
        Ok(PlaybackItem {
            id: item_id,
            source: materialize_source(spec)?,
            required_decoder,
        })
    }

    async fn resolver_for_source(
        &self,
        source: &SourceCatalogEntry,
    ) -> Result<Arc<dyn SourceResolver>, PlayerServiceError> {
        if let Some(resolver) = self.source_resolvers.read().await.get(&source.id).cloned() {
            return Ok(resolver);
        }
        let spec = source
            .resolver
            .as_ref()
            .ok_or(PlayerServiceError::ResolverUnavailable(source.id))?;
        let resolver = self.resolver_factory.create(spec)?;
        self.source_resolvers
            .write()
            .await
            .insert(source.id, Arc::clone(&resolver));
        Ok(resolver)
    }
}
