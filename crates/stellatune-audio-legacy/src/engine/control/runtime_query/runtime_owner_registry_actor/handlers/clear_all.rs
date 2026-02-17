use stellatune_runtime::tokio_actor::{ActorContext, CallError, Handler, Message};
use tracing::warn;

use crate::engine::control::runtime_query::OWNER_WORKER_CLEAR_TIMEOUT;
use crate::engine::control::runtime_query::lyrics_owner_actor::handlers::freeze::LyricsFreezeMessage;
use crate::engine::control::runtime_query::lyrics_owner_actor::handlers::shutdown::LyricsShutdownMessage;
use crate::engine::control::runtime_query::output_sink_owner_actor::handlers::freeze::OutputSinkFreezeMessage;
use crate::engine::control::runtime_query::output_sink_owner_actor::handlers::shutdown::OutputSinkShutdownMessage;
use crate::engine::control::runtime_query::runtime_owner_registry_actor::RuntimeOwnerRegistryActor;
use crate::engine::control::runtime_query::source_owner_actor::handlers::freeze::SourceFreezeMessage;
use crate::engine::control::runtime_query::source_owner_actor::handlers::shutdown::SourceShutdownMessage;

pub(crate) struct ClearAllRuntimeOwnersMessage;

impl Message for ClearAllRuntimeOwnersMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<ClearAllRuntimeOwnersMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        _message: ClearAllRuntimeOwnersMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        let source_freeze_refs: Vec<_> = self
            .source_tasks
            .iter()
            .filter_map(|(_, handle)| {
                if handle.frozen {
                    None
                } else {
                    Some(handle.actor_ref.clone())
                }
            })
            .collect();
        for handle in self.source_tasks.values_mut() {
            handle.frozen = true;
        }

        let removable_source_slots: Vec<_> = self
            .source_tasks
            .iter()
            .filter_map(|(slot, handle)| {
                if handle.active_streams == 0 {
                    Some(slot.clone())
                } else {
                    None
                }
            })
            .collect();
        let source_shutdown_refs: Vec<_> = removable_source_slots
            .iter()
            .filter_map(|slot| self.source_tasks.remove(slot).map(|h| h.actor_ref))
            .collect();
        self.source_stream_slots
            .retain(|_, slot| !removable_source_slots.iter().any(|s| s == slot));

        let lyrics_freeze_refs: Vec<_> = self
            .lyrics_tasks
            .iter()
            .filter_map(|(_, handle)| {
                if handle.frozen {
                    None
                } else {
                    Some(handle.actor_ref.clone())
                }
            })
            .collect();
        for handle in self.lyrics_tasks.values_mut() {
            handle.frozen = true;
        }

        let lyrics_slots: Vec<_> = self.lyrics_tasks.keys().cloned().collect();
        let lyrics_shutdown_refs: Vec<_> = lyrics_slots
            .iter()
            .filter_map(|slot| self.lyrics_tasks.remove(slot).map(|h| h.actor_ref))
            .collect();

        let output_sink_freeze_refs: Vec<_> = self
            .output_sink_tasks
            .iter()
            .filter_map(|(_, handle)| {
                if handle.frozen {
                    None
                } else {
                    Some(handle.actor_ref.clone())
                }
            })
            .collect();
        for handle in self.output_sink_tasks.values_mut() {
            handle.frozen = true;
        }

        let output_sink_slots: Vec<_> = self.output_sink_tasks.keys().cloned().collect();
        let output_sink_shutdown_refs: Vec<_> = output_sink_slots
            .iter()
            .filter_map(|slot| self.output_sink_tasks.remove(slot).map(|h| h.actor_ref))
            .collect();

        for actor_ref in source_freeze_refs {
            match actor_ref
                .call(SourceFreezeMessage, OWNER_WORKER_CLEAR_TIMEOUT)
                .await
            {
                Ok(()) => {},
                Err(CallError::Timeout) => {
                    warn!("source owner task freeze timeout");
                },
                Err(_) => {},
            }
        }
        for actor_ref in source_shutdown_refs {
            match actor_ref
                .call(SourceShutdownMessage, OWNER_WORKER_CLEAR_TIMEOUT)
                .await
            {
                Ok(()) => {},
                Err(CallError::Timeout) => {
                    warn!("source owner task shutdown timeout");
                },
                Err(_) => {},
            }
        }
        for actor_ref in lyrics_freeze_refs {
            match actor_ref
                .call(LyricsFreezeMessage, OWNER_WORKER_CLEAR_TIMEOUT)
                .await
            {
                Ok(()) => {},
                Err(CallError::Timeout) => {
                    warn!("lyrics owner task freeze timeout");
                },
                Err(_) => {},
            }
        }
        for actor_ref in lyrics_shutdown_refs {
            match actor_ref
                .call(LyricsShutdownMessage, OWNER_WORKER_CLEAR_TIMEOUT)
                .await
            {
                Ok(()) => {},
                Err(CallError::Timeout) => {
                    warn!("lyrics owner task shutdown timeout");
                },
                Err(_) => {},
            }
        }
        for actor_ref in output_sink_freeze_refs {
            match actor_ref
                .call(OutputSinkFreezeMessage, OWNER_WORKER_CLEAR_TIMEOUT)
                .await
            {
                Ok(()) => {},
                Err(CallError::Timeout) => {
                    warn!("output sink owner task freeze timeout");
                },
                Err(_) => {},
            }
        }
        for actor_ref in output_sink_shutdown_refs {
            match actor_ref
                .call(OutputSinkShutdownMessage, OWNER_WORKER_CLEAR_TIMEOUT)
                .await
            {
                Ok(()) => {},
                Err(CallError::Timeout) => {
                    warn!("output sink owner task shutdown timeout");
                },
                Err(_) => {},
            }
        }
    }
}
