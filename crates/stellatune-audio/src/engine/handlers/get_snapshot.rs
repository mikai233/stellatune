use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::config::engine::EngineSnapshot;
use crate::engine::actor::ControlActor;
use crate::engine::messages::GetSnapshotMessage;

impl Handler<GetSnapshotMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: GetSnapshotMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> EngineSnapshot {
        self.snapshot.clone()
    }
}
