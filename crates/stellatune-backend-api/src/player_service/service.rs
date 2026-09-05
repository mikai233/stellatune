use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use stellatune_audio::playback::control::{PlaybackController, SwitchOptions, SwitchTransition};
use stellatune_audio_core::playback::{MediaTime, PlaybackItem, PlaybackItemId};

use super::catalog::PlayerCatalog;
use super::error::PlayerServiceError;
use super::identity::{
    ProviderId, ProviderTrackIdentityInput, SourceBinding, SourceInstanceId, TrackId,
};
use super::resolver::{
    LocalTrackResolver, SourceResolver, SourceResolverFactory, materialize_source,
};
use super::source::{SourceCatalogEntry, SourceResolverSpec, TrackOrigin};
use super::state::PlaybackStateRecord;
pub struct PlayerService {
    pub(super) catalog: PlayerCatalog,
    pub(super) controller: PlaybackController,
    local_resolver: Arc<dyn LocalTrackResolver>,
    resolver_factory: Arc<dyn SourceResolverFactory>,
    source_resolvers: tokio::sync::RwLock<HashMap<SourceInstanceId, Arc<dyn SourceResolver>>>,
    pub(super) state_writer_started: AtomicBool,
    pub(super) queue_events: tokio::sync::broadcast::Sender<u64>,
    pub(super) queue: tokio::sync::Mutex<super::queue::QueueCoordinator>,
}

impl PlayerService {
    pub async fn ensure_local_tracks(
        &self,
        library_ids: &[i64],
    ) -> Result<Vec<TrackId>, PlayerServiceError> {
        self.catalog.ensure_local_tracks(library_ids).await
    }

    /// Projects metadata for a captured queue using stable track identities.
    pub async fn queue_local_metadata(
        &self,
        tracks: &[TrackId],
    ) -> Result<HashMap<TrackId, (i64, Option<PathBuf>)>, PlayerServiceError> {
        let identities = self.catalog.local_library_ids(tracks).await?;
        let ids: Vec<_> = identities.values().copied().collect();
        let paths = self.local_resolver.resolve_paths(&ids).await?;
        Ok(identities
            .into_iter()
            .map(|(track, id)| (track, (id, paths.get(&id).cloned())))
            .collect())
    }

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
            queue_events: tokio::sync::broadcast::channel(128).0,
            queue: tokio::sync::Mutex::new(Default::default()),
        }
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

    pub async fn restore(self: &Arc<Self>) -> Result<PlaybackStateRecord, PlayerServiceError> {
        self.restore_with_policy(false).await
    }

    pub async fn restore_with_policy(
        self: &Arc<Self>,
        resume_playing_on_launch: bool,
    ) -> Result<PlaybackStateRecord, PlayerServiceError> {
        let state = self.catalog.load_state().await?;
        self.load_queue().await?;
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
            .switch_to(
                item,
                SwitchOptions {
                    autoplay: false,
                    transition: SwitchTransition::ImmediateWithDeClick,
                },
            )
            .await?;
        if state.position_ms > 0 && capabilities.byte_seekable && !capabilities.live {
            self.controller
                .seek(MediaTime::from_millis(state.position_ms))
                .await?;
        }
        if resume_playing_on_launch && state.was_playing {
            self.controller.play().await?;
        }
        self.refresh_next().await?;
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
                (self.resolver_factory.resolve_local(path).await?, None)
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
