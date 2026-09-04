//! Playback state snapshots and ordered broadcast events.
//!
//! State and item events originate from the playback actor. Position events
//! use sink-consumed frames, not decoded or merely queued frames. Receivers may
//! lag because events use a bounded Tokio broadcast channel; callers can obtain
//! a fresh
//! [`PlaybackRuntimeSnapshot`](crate::playback::event::PlaybackRuntimeSnapshot)
//! through the controller.

use stellatune_audio_core::{
    error::PlaybackFailure,
    playback::{MediaTime, PlaybackItemId},
};

/// The externally observable behavior of the playback actor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaybackState {
    /// No current item or prepared output exists.
    #[default]
    Idle,
    /// A current item is being opened and its pipeline is being built.
    Preparing,
    /// A recoverable failure is reopening and seeking the current item.
    Recovering,
    /// A current item is prepared but not advancing output.
    Ready,
    /// The current item is advancing toward the output device.
    Playing,
    /// Output is paused without discarding the current pipeline.
    Paused,
    /// Playback is waiting for source, decoder, seek, or output progress.
    Buffering,
    /// The current playback attempt ended in a terminal failure.
    Failed,
}

/// A point-in-time projection of runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackRuntimeSnapshot {
    /// The actor's current playback state.
    pub state: PlaybackState,
    /// The active item, or `None` while idle or before initial activation.
    pub current_item_id: Option<PlaybackItemId>,
    /// The active item's position derived from sink-consumed frames.
    pub consumed_position: MediaTime,
}

/// An ordered notification emitted by the playback runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackEvent {
    /// The actor entered a different [`PlaybackState`].
    StateChanged(PlaybackState),
    /// A newly activated item became the output stream's current item.
    ///
    /// For a queued transition, this is emitted only after the sink consumes
    /// the item boundary marker.
    TrackChanged {
        /// The newly current item.
        item_id: PlaybackItemId,
    },
    /// The current item and its drained output reached natural completion.
    PlaybackEnded {
        /// The completed item.
        item_id: PlaybackItemId,
    },
    /// Sink consumption advanced far enough to publish a position update.
    Position {
        /// The item whose position advanced.
        item_id: PlaybackItemId,
        /// The item-relative, sink-consumed media position.
        position: MediaTime,
    },
    /// The current item entered or left a temporary buffering condition.
    Buffering {
        /// The affected item.
        item_id: PlaybackItemId,
        /// `true` when buffering begins and `false` when progress resumes.
        active: bool,
    },
    /// Playback encountered a contextual pipeline or runtime failure.
    Failed(PlaybackFailure),
}
