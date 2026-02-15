pub(super) mod handlers;

use crossbeam_channel::Sender;

use crate::engine::control::InternalDispatch;

pub(super) struct PreloadActor {
    pub(super) internal_tx: Sender<InternalDispatch>,
}

pub(super) fn spawn_preload_actor(
    internal_tx: Sender<InternalDispatch>,
) -> std::io::Result<(
    stellatune_runtime::thread_actor::ActorRef<PreloadActor>,
    std::thread::JoinHandle<()>,
)> {
    stellatune_runtime::thread_actor::spawn_actor_named(
        PreloadActor { internal_tx },
        "stellatune-preload-actor",
    )
}
