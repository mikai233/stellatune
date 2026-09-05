use stellatune_audio_core::playback::PlaybackItemId;
use thiserror::Error;

use super::identity::{SourceInstanceId, TrackId};

#[derive(Debug, Error)]
pub enum PlayerServiceError {
    #[error("playback navigation was superseded")]
    Superseded,
    #[error("playback storage schema is not the current hard-switch schema")]
    IncompatiblePlaybackSchema,
    #[error("invalid {identity} value {value}")]
    InvalidIdentity { identity: &'static str, value: u64 },
    #[error("provider id is empty or too long")]
    InvalidProviderId,
    #[error("provider track key is invalid")]
    InvalidProviderTrackKey,
    #[error("invalid resolved source: {0}")]
    InvalidSourceSpec(String),
    #[error("source catalog and track origin do not match")]
    CatalogBindingMismatch,
    #[error("source {0:?} was not found")]
    SourceNotFound(SourceInstanceId),
    #[error("track {0:?} was not found")]
    TrackNotFound(TrackId),
    #[error("playback item {0:?} was not found")]
    PlaybackItemNotFound(PlaybackItemId),
    #[error("source {0:?} is unavailable")]
    SourceUnavailable(SourceInstanceId),
    #[error("track {0:?} is unavailable")]
    TrackUnavailable(TrackId),
    #[error("resolver for source {0:?} is unavailable")]
    ResolverUnavailable(SourceInstanceId),
    #[error("playback state violates queue/current invariants")]
    PlaybackStateInvariant,
    #[error("source materialization failed: {0}")]
    Materialize(String),
    #[error("source resolution failed: {0}")]
    Resolve(String),
    #[error("local library resolution failed: {0}")]
    LocalLibrary(String),
    #[error("local library track {0} was not found")]
    LocalTrackNotFound(i64),
    #[error(transparent)]
    Storage(#[from] sqlx::Error),
    #[error(transparent)]
    Control(#[from] stellatune_audio_core::error::PlaybackControlError),
}
