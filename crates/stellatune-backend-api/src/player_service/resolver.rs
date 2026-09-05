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
    async fn resolve_path(&self, library_track_id: i64) -> Result<PathBuf, PlayerServiceError>;

    /// Projects available paths without opening audio sources; missing tracks are omitted.
    async fn resolve_paths(
        &self,
        library_track_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, PathBuf>, PlayerServiceError> {
        let mut paths = std::collections::HashMap::new();
        for id in library_track_ids {
            match self.resolve_path(*id).await {
                Ok(path) => {
                    paths.insert(*id, path);
                },
                Err(PlayerServiceError::LocalTrackNotFound(_)) => {},
                Err(error) => return Err(error),
            }
        }
        Ok(paths)
    }
}

#[async_trait]
impl LocalTrackResolver for stellatune_library::LibraryHandle {
    async fn resolve_paths(
        &self,
        library_track_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, PathBuf>, PlayerServiceError> {
        self.get_track_paths(library_track_ids.to_vec())
            .await
            .map(|paths| {
                paths
                    .into_iter()
                    .map(|(id, path)| (id, PathBuf::from(path)))
                    .collect()
            })
            .map_err(|error| PlayerServiceError::LocalLibrary(error.to_string()))
    }

    async fn resolve_path(&self, library_track_id: i64) -> Result<PathBuf, PlayerServiceError> {
        self.get_track(library_track_id)
            .await
            .map_err(|error| PlayerServiceError::LocalLibrary(error.to_string()))?
            .map(|track| PathBuf::from(track.path))
            .ok_or(PlayerServiceError::LocalTrackNotFound(library_track_id))
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

pub trait SourceResolverFactory: Send + Sync {
    fn create(
        &self,
        spec: &SourceResolverSpec,
    ) -> Result<Arc<dyn SourceResolver>, PlayerServiceError>;
}

pub(super) fn materialize_source(
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
