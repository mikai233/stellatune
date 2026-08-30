use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::config::engine::EngineSnapshot;
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::GetSnapshotMessage;

impl Responder<GetSnapshotMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: GetSnapshotMessage,
        reply_to: ReplyTo<EngineSnapshot>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(self.snapshot(*ctx.behavior()));
        Ok(())
    }
}
