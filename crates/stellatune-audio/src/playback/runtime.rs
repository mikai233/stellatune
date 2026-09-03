use std::sync::Mutex;
use std::thread::JoinHandle;

use stellatune_audio_core::PlaybackControlError;
use tokio::sync::{broadcast, oneshot};

use crate::planner::{PlaybackPolicies, StageRegistrySnapshot};

use super::actor::actor_loop;
use super::control::{Command, CommandKind, PlaybackController};

pub struct PlaybackRuntimeConfig {
    pub registry: StageRegistrySnapshot,
    pub policies: PlaybackPolicies,
    pub command_capacity: usize,
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
            pcm_ring_blocks: 8,
            block_frames: 1024,
            event_capacity: 128,
        }
    }
}

pub struct PlaybackRuntime {
    controller: PlaybackController,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl PlaybackRuntime {
    pub fn start(config: PlaybackRuntimeConfig) -> Result<Self, PlaybackControlError> {
        let (command_tx, command_rx) = crossbeam_channel::bounded(config.command_capacity.max(1));
        let (event_tx, _) = broadcast::channel(config.event_capacity.max(1));
        let controller = PlaybackController {
            command_tx,
            event_tx: event_tx.clone(),
        };
        let join = std::thread::Builder::new()
            .name("stellatune-playback-actor".to_owned())
            .spawn(move || actor_loop(config, command_rx, event_tx))
            .map_err(|error| PlaybackControlError::failed("runtime", error.to_string()))?;
        Ok(Self {
            controller,
            join: Mutex::new(Some(join)),
        })
    }

    pub fn controller(&self) -> PlaybackController {
        self.controller.clone()
    }

    pub async fn shutdown(self) -> Result<(), PlaybackControlError> {
        self.controller.request_shutdown().await?;
        if let Some(join) = self.join.lock().expect("runtime join poisoned").take() {
            join.join().map_err(|_| {
                PlaybackControlError::failed(
                    "runtime",
                    "playback actor panicked during shutdown".to_owned(),
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for PlaybackRuntime {
    fn drop(&mut self) {
        if self.join.lock().expect("runtime join poisoned").is_some() {
            let (response, _) = oneshot::channel();
            let _ = self.controller.command_tx.try_send(Command {
                kind: CommandKind::Shutdown,
                response,
            });
        }
    }
}
