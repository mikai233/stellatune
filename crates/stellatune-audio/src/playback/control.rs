//! Typed, deadline-bound control of a running playback runtime.
//!
//! A [`PlaybackController`](crate::playback::control::PlaybackController) is
//! cheap to clone and does not own runtime lifetime. Each asynchronous method
//! waits for a typed actor reply up to the configured deadline. Dropping that
//! waiting future does not send a separate cancellation command; an
//! already-enqueued command may still execute.

use lattice_actor::{error::ActorCallError, handle::ActorHandle};
use stellatune_audio_core::{
    error::PlaybackControlError,
    playback::{MediaTime, PlaybackItem, PlaybackItemId},
};
use tokio::sync::broadcast;

use crate::planner::PlaybackPolicies;

use super::actor::{
    GetSnapshot, Pause, Play, PlaybackActor, RebuildOutput, Seek, SetOutputGain, SetPolicies,
    StopPlayback,
};
use super::event::{PlaybackEvent, PlaybackRuntimeSnapshot};
use super::navigation::{AdvanceToNext, SetNext, SwitchTo};
use super::runtime::PlaybackCommandTimeouts;

/// How an explicit switch interacts with the configured transition policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchTransition {
    /// Applies the active [`PlaybackPolicies`] transition when a track exists.
    UseConfiguredPolicy,
    /// Stops the current track and uses a short de-click envelope for the new one.
    ImmediateWithDeClick,
}

/// Options applied when switching to a playback item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchOptions {
    /// Whether a newly activated item starts playing immediately.
    pub autoplay: bool,
    /// How an existing current item transitions to the new item.
    pub transition: SwitchTransition,
}

impl Default for SwitchOptions {
    fn default() -> Self {
        Self {
            autoplay: true,
            transition: SwitchTransition::UseConfiguredPolicy,
        }
    }
}

/// A cloneable command endpoint for one [`PlaybackRuntime`](super::runtime::PlaybackRuntime).
///
/// Commands are serialized by the playback actor. Controller clones share the
/// same mailbox, event sender, and command deadlines. Dropping every controller
/// does not deterministically release the runtime; its owner should call
/// [`PlaybackRuntime::shutdown`](super::runtime::PlaybackRuntime::shutdown).
#[derive(Clone)]
pub struct PlaybackController {
    pub(super) actor: ActorHandle<PlaybackActor>,
    pub(super) event_tx: broadcast::Sender<PlaybackEvent>,
    pub(super) timeouts: PlaybackCommandTimeouts,
}

impl PlaybackController {
    /// Prepares `item` and makes it current according to `options`.
    ///
    /// With [`SwitchTransition::UseConfiguredPolicy`], an existing track keeps
    /// playing while the new item is prepared as its forced successor. An
    /// immediate switch tears down the current pipeline before preparation.
    /// If the item identity matches the successor, its prepared pipeline or
    /// pending task is reused and this method acknowledges the switch intent.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::CommandTimeout`] when preparation misses
    /// its deadline, [`PlaybackControlError::Closed`] when the runtime stops, or
    /// [`PlaybackControlError::Failed`] when planning or preparation fails.
    pub async fn switch_to(
        &self,
        item: PlaybackItem,
        options: SwitchOptions,
    ) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(SwitchTo { item, options }, self.timeouts.preparation)
            .await
            .map_err(|error| map_call_error("switch_to", error))?
    }

    /// Sets or clears the successor without interrupting the current item.
    ///
    /// `None` cancels the successor slot in any state. `Some(item)` requires
    /// an active item and replaces only a different successor. Repeating the
    /// same item ID preserves its existing pipeline or task. An overlap that
    /// already started is retained; the slot then refers to its successor.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::InvalidState`] when there is no active
    /// session, [`PlaybackControlError::CommandTimeout`] on preparation timeout,
    /// [`PlaybackControlError::Closed`] after runtime shutdown, or
    /// [`PlaybackControlError::Failed`] when planning or preparation fails.
    pub async fn set_next(&self, item: Option<PlaybackItem>) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(SetNext { item }, self.timeouts.preparation)
            .await
            .map_err(|error| map_call_error("set_next", error))?
    }

    /// Requests advancement to the expected successor without reopening its source.
    ///
    /// Acceptance includes a successor still preparing. The item becomes audible
    /// later, as reported by the item-boundary event. A missing or different
    /// successor returns [`AdvanceOutcome::Unavailable`].
    ///
    /// # Errors
    ///
    /// Returns a control error if the runtime closes, the request times out, or
    /// activation of the prepared pipeline fails.
    pub async fn advance_to_next(
        &self,
        expected_item_id: PlaybackItemId,
        options: SwitchOptions,
    ) -> Result<AdvanceOutcome, PlaybackControlError> {
        self.actor
            .ask(
                AdvanceToNext {
                    expected_item_id,
                    options,
                },
                self.timeouts.control,
            )
            .await
            .map_err(|error| map_call_error("advance_to_next", error))?
    }

    /// Starts or resumes the current item.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::InvalidState`] without an active item,
    /// [`PlaybackControlError::CommandTimeout`] when the command deadline is
    /// exceeded, [`PlaybackControlError::Closed`] after shutdown, or
    /// [`PlaybackControlError::Failed`] when the output cannot resume.
    pub async fn play(&self) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(Play, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("play", error))?
    }

    /// Pauses the current output without discarding queued PCM.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::InvalidState`] without an active item,
    /// [`PlaybackControlError::CommandTimeout`] when the command deadline is
    /// exceeded, [`PlaybackControlError::Closed`] after shutdown, or
    /// [`PlaybackControlError::Failed`] when the output cannot pause.
    pub async fn pause(&self) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(Pause, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("pause", error))?
    }

    /// Seeks the current item to an absolute media position.
    ///
    /// Seeking invalidates queued PCM, resets transforms and normalization, and
    /// applies the configured de-click envelope at the actual decoder result.
    /// The paused/playing intent is restored after an incremental seek.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::InvalidState`] without an active item,
    /// [`PlaybackControlError::Unsupported`] for an unseekable source,
    /// [`PlaybackControlError::CommandTimeout`] on command timeout,
    /// [`PlaybackControlError::Closed`] when superseded or stopped, or
    /// [`PlaybackControlError::Failed`] for decoder or sink failures.
    pub async fn seek(&self, position: MediaTime) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(Seek { position }, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("seek", error))?
    }

    /// Stops playback and clears current, queued, transition, and seek state.
    ///
    /// The runtime remains alive and accepts a later [`Self::switch_to`].
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::CommandTimeout`] when the command misses
    /// its deadline, [`PlaybackControlError::Closed`] after runtime shutdown, or
    /// [`PlaybackControlError::Failed`] for an actor or output failure.
    pub async fn stop(&self) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(StopPlayback, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("stop", error))?
    }

    /// Sets the final output gain, optionally ramping over `ramp`.
    ///
    /// `gain` is clamped to `0.0..=1.0`. The value is retained for future sink
    /// creation; when no track is active, the operation only updates that
    /// retained value.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::CommandTimeout`] when the command misses
    /// its deadline, [`PlaybackControlError::Closed`] after shutdown, or
    /// [`PlaybackControlError::Failed`] when the active sink rejects the change.
    pub async fn set_output_gain(
        &self,
        gain: f32,
        ramp: MediaTime,
    ) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(
                SetOutputGain {
                    gain: gain.clamp(0.0, 1.0),
                    ramp,
                },
                self.timeouts.control,
            )
            .await
            .map_err(|error| map_call_error("set_output_gain", error))?
    }

    /// Recreates the active output sink while preserving playback intent.
    ///
    /// This is a no-op when no item is active. A currently playing item resumes
    /// after the replacement sink opens; other states remain non-playing.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::CommandTimeout`] when output rebuilding
    /// misses its deadline, [`PlaybackControlError::Closed`] after shutdown, or
    /// [`PlaybackControlError::Failed`] when the sink cannot be recreated.
    pub async fn rebuild_output(&self) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(RebuildOutput, self.timeouts.output_rebuild)
            .await
            .map_err(|error| map_call_error("rebuild_output", error))?
    }

    /// Replaces policies used by subsequently planned and prepared tracks.
    ///
    /// Already-active track and transition state retain the policy captured by
    /// their executable plan.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::CommandTimeout`] when the command misses
    /// its deadline, [`PlaybackControlError::Closed`] after shutdown, or
    /// [`PlaybackControlError::Failed`] for actor infrastructure failure.
    pub async fn set_policies(
        &self,
        policies: PlaybackPolicies,
    ) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(SetPolicies { policies }, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("set_policies", error))?
    }

    /// Returns a point-in-time view of playback state and consumed position.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::CommandTimeout`] when the snapshot misses
    /// its deadline, [`PlaybackControlError::Closed`] after shutdown, or
    /// [`PlaybackControlError::Failed`] for actor infrastructure failure.
    pub async fn snapshot(&self) -> Result<PlaybackRuntimeSnapshot, PlaybackControlError> {
        self.actor
            .ask(GetSnapshot, self.timeouts.snapshot)
            .await
            .map_err(|error| map_call_error("snapshot", error))?
    }

    /// Subscribes to playback events emitted after this call.
    ///
    /// The returned Tokio broadcast receiver reports lag if it falls more than
    /// the configured event capacity behind. Use [`Self::snapshot`] to rebuild
    /// current state after lagging.
    pub fn subscribe_events(&self) -> broadcast::Receiver<PlaybackEvent> {
        self.event_tx.subscribe()
    }
}

fn map_call_error(operation: &'static str, error: ActorCallError) -> PlaybackControlError {
    match error {
        ActorCallError::DeadlineExceeded => PlaybackControlError::CommandTimeout { operation },
        ActorCallError::UnhandledInCurrentState => PlaybackControlError::InvalidState,
        ActorCallError::MailboxClosed
        | ActorCallError::LifecycleUnavailable { .. }
        | ActorCallError::ResponseDropped => PlaybackControlError::Closed,
        ActorCallError::MailboxFull
        | ActorCallError::ActorPanicked
        | ActorCallError::Handler(_)
        | ActorCallError::InvalidTimeout => PlaybackControlError::failed(
            "runtime",
            format!("playback command `{operation}` failed: {error}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::map_call_error;
    use lattice_actor::error::{ActorCallError, ActorError};
    use lattice_actor::traits::ActorLifecycleState;
    use stellatune_audio_core::error::{FailureStage, PlaybackControlError};

    #[test]
    fn maps_lattice_call_errors_to_control_contract() {
        assert_eq!(
            map_call_error("snapshot", ActorCallError::DeadlineExceeded),
            PlaybackControlError::CommandTimeout {
                operation: "snapshot"
            }
        );
        assert_eq!(
            map_call_error("play", ActorCallError::UnhandledInCurrentState),
            PlaybackControlError::InvalidState
        );
        for error in [
            ActorCallError::MailboxClosed,
            ActorCallError::LifecycleUnavailable {
                state: ActorLifecycleState::Stopped,
            },
            ActorCallError::ResponseDropped,
        ] {
            assert_eq!(map_call_error("play", error), PlaybackControlError::Closed);
        }
        for error in [
            ActorCallError::MailboxFull,
            ActorCallError::ActorPanicked,
            ActorCallError::Handler(ActorError::new("failed")),
            ActorCallError::InvalidTimeout,
        ] {
            assert!(matches!(
                map_call_error("play", error),
                PlaybackControlError::Failed(failure)
                    if failure.stage == FailureStage::Runtime
            ));
        }
    }
}

/// Result of atomically claiming a prepared or preparing successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvanceOutcome {
    /// The target is retained and advancement has been requested.
    Accepted,
    /// The expected item was already promoted before this command was handled.
    AlreadyCurrent,
    /// No successor with the expected queue-item identity exists.
    Unavailable,
}
