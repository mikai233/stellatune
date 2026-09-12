use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use stellatune_audio_builtin_adapters::factories::{FileSourceFactory, HttpSourceFactory};
use stellatune_audio_core::{decoder::DecoderFactory, source::SourceFactory};

use super::error::PlayerServiceError;
use super::identity::ProviderTrackKey;
use super::source::{ResolvedSourceSpec, SourceCatalogEntry, SourceResolverSpec};

#[async_trait]
pub trait LocalTrackResolver: Send + Sync {
    async fn resolve_resource(
        &self,
        library_track_id: i64,
    ) -> Result<stellatune_library::catalog::LocalTrackResource, PlayerServiceError>;

    /// Projects metadata without opening audio sources; missing tracks are omitted.
    async fn resolve_metadata(
        &self,
        library_track_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, stellatune_library::TrackLite>, PlayerServiceError>
    {
        let mut tracks = std::collections::HashMap::new();
        for id in library_track_ids {
            match self.resolve_resource(*id).await {
                Ok(resource) => {
                    tracks.insert(
                        *id,
                        stellatune_library::TrackLite {
                            id: *id,
                            path: resource.path,
                            title: None,
                            artist: None,
                            album: None,
                            duration_ms: None,
                            cover_id: None,
                            is_segment: false,
                        },
                    );
                },
                Err(PlayerServiceError::LocalTrackNotFound(_)) => {},
                Err(error) => return Err(error),
            }
        }
        Ok(tracks)
    }
}

#[async_trait]
impl LocalTrackResolver for stellatune_library::LibraryHandle {
    async fn resolve_metadata(
        &self,
        library_track_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, stellatune_library::TrackLite>, PlayerServiceError>
    {
        self.get_tracks(library_track_ids.to_vec())
            .await
            .map_err(|error| PlayerServiceError::LocalLibrary(error.to_string()))
    }

    async fn resolve_resource(
        &self,
        library_track_id: i64,
    ) -> Result<stellatune_library::catalog::LocalTrackResource, PlayerServiceError> {
        self.catalog()
            .playback_resource(library_track_id)
            .await
            .map_err(|error| PlayerServiceError::LocalLibrary(error.to_string()))
    }
}

#[async_trait]
pub trait SourceResolver: Send + Sync {
    async fn resolve(
        &self,
        source: &SourceCatalogEntry,
        key: &ProviderTrackKey,
    ) -> Result<ResolvedSourceSpec, PlayerServiceError>;

    fn required_decoder(&self) -> Option<Arc<dyn DecoderFactory>> {
        None
    }
}

#[async_trait]
pub trait SourceResolverFactory: Send + Sync {
    fn create(
        &self,
        spec: &SourceResolverSpec,
    ) -> Result<Arc<dyn SourceResolver>, PlayerServiceError>;

    async fn resolve_local(&self, path: PathBuf) -> Result<ResolvedSourceSpec, PlayerServiceError> {
        Ok(ResolvedSourceSpec::File {
            path,
            media: Default::default(),
        })
    }
}

pub(crate) fn materialize_source(
    spec: ResolvedSourceSpec,
) -> Result<Arc<dyn SourceFactory>, PlayerServiceError> {
    match spec {
        ResolvedSourceSpec::File { path, media } => Ok(Arc::new(
            FileSourceFactory::new(path, media)
                .map_err(|error| PlayerServiceError::Materialize(error.to_string()))?,
        )),
        ResolvedSourceSpec::Http {
            url,
            headers,
            media,
            capabilities,
        } => Ok(Arc::new(
            HttpSourceFactory::new(url, headers, media, capabilities)
                .map_err(|error| PlayerServiceError::Materialize(error.to_string()))?,
        )),
    }
}
