pub(super) mod handlers;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::engine::control::RuntimeInstanceSlotKey;
use crate::engine::control::runtime_query::lyrics_owner_actor::LyricsOwnerActor;
use crate::engine::control::runtime_query::output_sink_owner_actor::OutputSinkOwnerActor;
use crate::engine::control::runtime_query::source_owner_actor::SourceOwnerActor;

pub(super) struct SourceOwnerTaskHandle {
    pub(super) actor_ref: stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>,
    pub(super) active_streams: usize,
    pub(super) frozen: bool,
}

pub(super) struct LyricsOwnerTaskHandle {
    pub(super) actor_ref: stellatune_runtime::tokio_actor::ActorRef<LyricsOwnerActor>,
    pub(super) frozen: bool,
}

pub(super) struct OutputSinkOwnerTaskHandle {
    pub(super) actor_ref: stellatune_runtime::tokio_actor::ActorRef<OutputSinkOwnerActor>,
    pub(super) frozen: bool,
}

pub(super) struct RuntimeOwnerRegistryActor {
    pub(super) source_tasks: HashMap<RuntimeInstanceSlotKey, SourceOwnerTaskHandle>,
    pub(super) lyrics_tasks: HashMap<RuntimeInstanceSlotKey, LyricsOwnerTaskHandle>,
    pub(super) output_sink_tasks: HashMap<RuntimeInstanceSlotKey, OutputSinkOwnerTaskHandle>,
    pub(super) source_stream_slots: HashMap<u64, RuntimeInstanceSlotKey>,
    pub(super) next_source_stream_id: u64,
}

impl RuntimeOwnerRegistryActor {
    pub(super) fn new() -> Self {
        Self {
            source_tasks: HashMap::new(),
            lyrics_tasks: HashMap::new(),
            output_sink_tasks: HashMap::new(),
            source_stream_slots: HashMap::new(),
            next_source_stream_id: 1,
        }
    }
}

pub(crate) enum SourceCloseTarget {
    MissingStream,
    MissingTask,
    Ready {
        slot: RuntimeInstanceSlotKey,
        actor_ref: stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>,
    },
}

pub(super) fn shared_runtime_owner_registry_actor()
-> stellatune_runtime::tokio_actor::ActorRef<RuntimeOwnerRegistryActor> {
    static REGISTRY_ACTOR: OnceLock<
        stellatune_runtime::tokio_actor::ActorRef<RuntimeOwnerRegistryActor>,
    > = OnceLock::new();
    REGISTRY_ACTOR
        .get_or_init(|| {
            let (actor_ref, _join) =
                stellatune_runtime::tokio_actor::spawn_actor(RuntimeOwnerRegistryActor::new());
            actor_ref
        })
        .clone()
}
