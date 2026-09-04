use std::time::Duration;

use lattice_actor::{
    mailbox::MailboxConfig,
    runtime::{ActorExecutionPolicy, ActorRuntime, ActorSpawnOptions},
    traits::StopReason,
};
use stellatune_audio_core::error::PlaybackControlError;
use tokio::sync::broadcast;

use crate::planner::{PlaybackPolicies, StageRegistrySnapshot};

use super::actor::PlaybackActor;
use super::control::PlaybackController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackCommandTimeouts {
    pub snapshot: Duration,
    pub control: Duration,
    pub output_rebuild: Duration,
    pub preparation: Duration,
}

impl Default for PlaybackCommandTimeouts {
    fn default() -> Self {
        Self {
            snapshot: Duration::from_secs(2),
            control: Duration::from_secs(5),
            output_rebuild: Duration::from_secs(10),
            preparation: Duration::from_secs(30),
        }
    }
}

pub struct PlaybackRuntimeConfig {
    pub registry: StageRegistrySnapshot,
    pub policies: PlaybackPolicies,
    pub command_capacity: usize,
    pub preparation_capacity: usize,
    pub actor_turn_budget: usize,
    pub command_timeouts: PlaybackCommandTimeouts,
    pub pcm_ring_blocks: usize,
    pub block_frames: usize,
    pub event_capacity: usize,
}

impl PlaybackRuntimeConfig {
    pub fn new(registry: StageRegistrySnapshot) -> Self {
        Self {
            registry,
            policies: PlaybackPolicies::default(),
            command_capacity: 64,
            preparation_capacity: 4,
            actor_turn_budget: 16,
            command_timeouts: PlaybackCommandTimeouts::default(),
            pcm_ring_blocks: 8,
            block_frames: 1024,
            event_capacity: 128,
        }
    }
}

pub struct PlaybackRuntime {
    controller: PlaybackController,
}

impl PlaybackRuntime {
    pub fn start(config: PlaybackRuntimeConfig) -> Result<Self, PlaybackControlError> {
        let mailbox = MailboxConfig::bounded(config.command_capacity.max(1))
            .with_deferred_capacity(config.preparation_capacity.max(1))
            .with_turn_budget(config.actor_turn_budget.max(1));
        let timeouts = config.command_timeouts;
        let (event_tx, _) = broadcast::channel(config.event_capacity.max(1));
        let actor = PlaybackActor::new(config, event_tx.clone());
        let handle = ActorRuntime::default()
            .spawn_actor(
                actor,
                ActorSpawnOptions {
                    mailbox,
                    execution: Some(ActorExecutionPolicy::DedicatedThreadPool { worker_count: 1 }),
                    ..ActorSpawnOptions::default()
                },
            )
            .map_err(|error| PlaybackControlError::failed("runtime", error.to_string()))?;
        Ok(Self {
            controller: PlaybackController {
                actor: handle,
                event_tx,
                timeouts,
            },
        })
    }

    pub fn controller(&self) -> PlaybackController {
        self.controller.clone()
    }

    pub async fn shutdown(self) -> Result<(), PlaybackControlError> {
        let mut terminated = self.controller.actor.subscribe_terminated();
        self.controller
            .actor
            .stop(StopReason::Requested)
            .map_err(|error| PlaybackControlError::failed("runtime", error.to_string()))?;
        terminated
            .recv()
            .await
            .map_err(|error| PlaybackControlError::failed("runtime", error.to_string()))?;
        Ok(())
    }
}

impl Drop for PlaybackRuntime {
    fn drop(&mut self) {
        let _ = self.controller.actor.stop(StopReason::Requested);
    }
}
