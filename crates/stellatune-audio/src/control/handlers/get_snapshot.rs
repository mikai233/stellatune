use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::GetSnapshotMessage;
use crate::types::EngineSnapshot;

impl Handler<GetSnapshotMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: GetSnapshotMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> EngineSnapshot {
        self.snapshot.clone()
    }
}
