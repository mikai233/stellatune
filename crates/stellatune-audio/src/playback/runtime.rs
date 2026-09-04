//! Playback actor configuration, startup, and deterministic shutdown.
//!
//! [`PlaybackRuntime`](crate::playback::runtime::PlaybackRuntime) owns one actor
//! lifecycle. The actor uses a dedicated one-worker Lattice pool, while each
//! active sink runs on its own OS thread. Dropping the runtime requests
//! best-effort shutdown; callers that require deterministic device release must
//! await
//! [`PlaybackRuntime::shutdown`](crate::playback::runtime::PlaybackRuntime::shutdown).

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
/// Deadlines applied to groups of playback commands.
pub struct PlaybackCommandTimeouts {
    /// Deadline for a state snapshot request.
    pub snapshot: Duration,
    /// Deadline for play, pause, seek, stop, gain, and policy commands.
    pub control: Duration,
    /// Deadline for rebuilding an output sink.
    pub output_rebuild: Duration,
    /// Deadline for switch and queued-item preparation.
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

/// Capacities, policies, and stage factories used to start a playback runtime.
pub struct PlaybackRuntimeConfig {
    /// The immutable stage registry captured by the actor.
    pub registry: StageRegistrySnapshot,
    /// Initial policies used when planning playback items.
    pub policies: PlaybackPolicies,
    /// Capacity of the bounded actor mailbox.
    pub command_capacity: usize,
    /// Maximum number of Lattice deferred operations in flight.
    pub preparation_capacity: usize,
    /// Maximum number of mailbox messages handled in one actor scheduling turn.
    pub actor_turn_budget: usize,
    /// Deadlines used by cloneable playback controllers.
    pub command_timeouts: PlaybackCommandTimeouts,
    /// Capacity of the bounded actor-to-sink PCM ring, measured in blocks.
    pub pcm_ring_blocks: usize,
    /// Target number of PCM frames decoded in one pump turn.
    pub block_frames: usize,
    /// Capacity of the playback event broadcast channel.
    pub event_capacity: usize,
}

impl PlaybackRuntimeConfig {
    /// Creates a runtime configuration with production defaults.
    ///
    /// The defaults use a 64-message mailbox, four deferred preparations, a
    /// turn budget of 16, eight PCM blocks, 1,024 frames per block, and 128
    /// retained broadcast events.
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

/// The owning lifecycle handle for one playback actor and its output workers.
///
/// Obtain cheap command endpoints with [`Self::controller`]. The runtime itself
/// is not cloneable so ownership of deterministic shutdown remains explicit.
pub struct PlaybackRuntime {
    controller: PlaybackController,
}

impl PlaybackRuntime {
    /// Starts a playback actor with `config` and returns its owning handle.
    ///
    /// Zero capacities and turn budgets are normalized to one.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::Failed`] when the actor cannot be
    /// spawned. Sink and decoder stages are created later during item
    /// preparation.
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

    /// Returns a cloneable controller connected to this runtime.
    pub fn controller(&self) -> PlaybackController {
        self.controller.clone()
    }

    /// Requests actor shutdown and waits until its stopping hook completes.
    ///
    /// The stopping hook cancels preparation, closes pending replies, resets
    /// pipeline stages, and joins the active sink worker.
    ///
    /// # Errors
    ///
    /// Returns [`PlaybackControlError::Failed`] when the stop request cannot be
    /// delivered or actor termination cannot be observed.
    ///
    /// # Examples
    ///
    /// ```
    /// use stellatune_audio::playback::runtime::PlaybackRuntime;
    /// use stellatune_audio_core::error::PlaybackControlError;
    ///
    /// async fn close_runtime(runtime: PlaybackRuntime) -> Result<(), PlaybackControlError> {
    ///     runtime.shutdown().await
    /// }
    /// ```
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
