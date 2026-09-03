use crossbeam_channel::{Sender, TrySendError};
use stellatune_audio_core::{MediaTime, PlaybackControlError, PlaybackItem};
use tokio::sync::{broadcast, oneshot};

use crate::planner::PlaybackPolicies;

use super::event::{PlaybackEvent, PlaybackRuntimeSnapshot};

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
    pub(super) command_tx: Sender<Command>,
    pub(super) event_tx: broadcast::Sender<PlaybackEvent>,
}

impl PlaybackController {
    async fn request(&self, kind: CommandKind) -> Result<CommandReply, PlaybackControlError> {
        let (response, receiver) = oneshot::channel();
        self.command_tx
            .try_send(Command { kind, response })
            .map_err(|error| match error {
                TrySendError::Disconnected(_) => PlaybackControlError::Closed,
                TrySendError::Full(_) => PlaybackControlError::failed(
                    "runtime",
                    "playback command queue is full".to_owned(),
                ),
            })?;
        receiver.await.map_err(|_| PlaybackControlError::Closed)?
    }

    pub async fn switch(
        &self,
        item: PlaybackItem,
        options: SwitchOptions,
    ) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Switch { item, options })
            .await
            .map(|_| ())
    }

    pub async fn queue_next(&self, item: PlaybackItem) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::QueueNext { item })
            .await
            .map(|_| ())
    }

    pub async fn play(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Play).await.map(|_| ())
    }

    pub async fn pause(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Pause).await.map(|_| ())
    }

    pub async fn seek(&self, position: MediaTime) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Seek(position)).await.map(|_| ())
    }

    pub async fn stop(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Stop).await.map(|_| ())
    }

    pub async fn set_output_gain(
        &self,
        gain: f32,
        ramp: MediaTime,
    ) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::SetOutputGain {
            gain: gain.clamp(0.0, 1.0),
            ramp,
        })
        .await
        .map(|_| ())
    }

    pub async fn rebuild_output(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::RebuildOutput).await.map(|_| ())
    }

    pub async fn set_policies(
        &self,
        policies: PlaybackPolicies,
    ) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::SetPolicies(policies))
            .await
            .map(|_| ())
    }

    pub async fn snapshot(&self) -> Result<PlaybackRuntimeSnapshot, PlaybackControlError> {
        match self.request(CommandKind::Snapshot).await? {
            CommandReply::Snapshot(snapshot) => Ok(snapshot),
            CommandReply::Unit => Err(PlaybackControlError::failed(
                "runtime",
                "snapshot command returned no snapshot".to_owned(),
            )),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<PlaybackEvent> {
        self.event_tx.subscribe()
    }

    pub(super) async fn request_shutdown(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Shutdown).await.map(|_| ())
    }
}

pub(super) struct Command {
    pub(super) kind: CommandKind,
    pub(super) response: oneshot::Sender<Result<CommandReply, PlaybackControlError>>,
}

pub(super) enum CommandKind {
    Switch {
        item: PlaybackItem,
        options: SwitchOptions,
    },
    QueueNext {
        item: PlaybackItem,
    },
    Play,
    Pause,
    Seek(MediaTime),
    Stop,
    SetOutputGain {
        gain: f32,
        ramp: MediaTime,
    },
    SetPolicies(PlaybackPolicies),
    RebuildOutput,
    Snapshot,
    Shutdown,
}

pub(super) enum CommandReply {
    Unit,
    Snapshot(PlaybackRuntimeSnapshot),
}
