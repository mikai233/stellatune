use stellatune_audio_core::{MediaTime, PlaybackFailure, PlaybackItemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Preparing,
    Recovering,
    Ready,
    Playing,
    Paused,
    Buffering,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackRuntimeSnapshot {
    pub state: PlaybackState,
    pub current_item_id: Option<PlaybackItemId>,
    pub consumed_position: MediaTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackEvent {
    StateChanged(PlaybackState),
    TrackChanged {
        item_id: PlaybackItemId,
    },
    PlaybackEnded {
        item_id: PlaybackItemId,
    },
    Position {
        item_id: PlaybackItemId,
        position: MediaTime,
    },
    Buffering {
        item_id: PlaybackItemId,
        active: bool,
    },
    Failed(PlaybackFailure),
}
