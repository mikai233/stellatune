use crate::player_service::error::PlayerServiceError;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub(super) struct ApiError(pub StatusCode, pub &'static str, pub String);
impl ApiError {
    pub fn bad(message: impl ToString) -> Self {
        Self(StatusCode::BAD_REQUEST, "invalidInput", message.to_string())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"code": self.1, "message": self.2}))).into_response()
    }
}
impl From<PlayerServiceError> for ApiError {
    fn from(error: PlayerServiceError) -> Self {
        use PlayerServiceError::*;
        let (status, code) = match &error {
            Control(
                stellatune_audio_core::error::PlaybackControlError::InvalidState
                | stellatune_audio_core::error::PlaybackControlError::Unsupported,
            ) => (StatusCode::BAD_REQUEST, "invalidCommand"),
            Control(stellatune_audio_core::error::PlaybackControlError::Failed(_)) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "playbackFailed")
            },
            InvalidIdentity { .. }
            | InvalidProviderId
            | InvalidProviderTrackKey
            | InvalidSourceSpec(_)
            | CatalogBindingMismatch => (StatusCode::BAD_REQUEST, "invalidInput"),
            TrackNotFound(_)
            | SourceNotFound(_)
            | PlaybackItemNotFound(_)
            | LocalTrackNotFound(_)
            | PluginCapabilityNotFound(_) => (StatusCode::NOT_FOUND, "notFound"),
            Control(_)
            | Superseded
            | SourceUnavailable(_)
            | TrackUnavailable(_)
            | ResolverUnavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internalError"),
        };
        Self(status, code, error.to_string())
    }
}
impl From<stellatune_audio_core::error::PlaybackControlError> for ApiError {
    fn from(error: stellatune_audio_core::error::PlaybackControlError) -> Self {
        PlayerServiceError::Control(error).into()
    }
}
