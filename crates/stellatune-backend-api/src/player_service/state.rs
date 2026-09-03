use stellatune_audio_core::PlaybackItemId;

use super::error::PlayerServiceError;
use super::identity::TrackId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackQueueRecord {
    pub item_id: PlaybackItemId,
    pub track_id: TrackId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackStateRecord {
    pub schema_version: u32,
    pub queue: Vec<PlaybackQueueRecord>,
    pub current_item_id: Option<PlaybackItemId>,
    pub position_ms: u64,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub was_playing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub(super) fn parse(value: &str) -> Result<Self, PlayerServiceError> {
        match value {
            "off" => Ok(Self::Off),
            "all" => Ok(Self::All),
            "one" => Ok(Self::One),
            _ => Err(PlayerServiceError::IncompatiblePlaybackSchema),
        }
    }
}
