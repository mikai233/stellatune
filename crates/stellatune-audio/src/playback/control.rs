use lattice_actor::{error::ActorCallError, handle::ActorHandle};
use stellatune_audio_core::{MediaTime, PlaybackControlError, PlaybackItem};
use tokio::sync::broadcast;

use crate::planner::PlaybackPolicies;

use super::actor::{
    GetSnapshot, Pause, Play, PlaybackActor, QueueNextTrack, RebuildOutput, Seek, SetOutputGain,
    SetPolicies, StopPlayback, SwitchTrack,
};
use super::event::{PlaybackEvent, PlaybackRuntimeSnapshot};
use super::runtime::PlaybackCommandTimeouts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchTransition {
    UseConfiguredPolicy,
    ImmediateWithDeClick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchOptions {
    pub autoplay: bool,
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

#[derive(Clone)]
pub struct PlaybackController {
    pub(super) actor: ActorHandle<PlaybackActor>,
    pub(super) event_tx: broadcast::Sender<PlaybackEvent>,
    pub(super) timeouts: PlaybackCommandTimeouts,
}

impl PlaybackController {
    pub async fn switch(
        &self,
        item: PlaybackItem,
        options: SwitchOptions,
    ) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(SwitchTrack { item, options }, self.timeouts.preparation)
            .await
            .map_err(|error| map_call_error("switch", error))?
    }

    pub async fn queue_next(&self, item: PlaybackItem) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(QueueNextTrack { item }, self.timeouts.preparation)
            .await
            .map_err(|error| map_call_error("queue_next", error))?
    }

    pub async fn play(&self) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(Play, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("play", error))?
    }

    pub async fn pause(&self) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(Pause, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("pause", error))?
    }

    pub async fn seek(&self, position: MediaTime) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(Seek { position }, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("seek", error))?
    }

    pub async fn stop(&self) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(StopPlayback, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("stop", error))?
    }

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

    pub async fn rebuild_output(&self) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(RebuildOutput, self.timeouts.output_rebuild)
            .await
            .map_err(|error| map_call_error("rebuild_output", error))?
    }

    pub async fn set_policies(
        &self,
        policies: PlaybackPolicies,
    ) -> Result<(), PlaybackControlError> {
        self.actor
            .ask(SetPolicies { policies }, self.timeouts.control)
            .await
            .map_err(|error| map_call_error("set_policies", error))?
    }

    pub async fn snapshot(&self) -> Result<PlaybackRuntimeSnapshot, PlaybackControlError> {
        self.actor
            .ask(GetSnapshot, self.timeouts.snapshot)
            .await
            .map_err(|error| map_call_error("snapshot", error))?
    }

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
    use stellatune_audio_core::{FailureStage, PlaybackControlError};

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
