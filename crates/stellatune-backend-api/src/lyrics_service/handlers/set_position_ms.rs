use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};

use crate::lyrics_service::LyricsServiceActor;

#[derive(lattice_actor::Message)]
pub(crate) struct SetPositionMsMessage {
    pub(crate) position_ms: u64,
}

impl Handler<SetPositionMsMessage> for LyricsServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: SetPositionMsMessage,
    ) -> Result<(), ActorError> {
        self.core.set_position_ms(message.position_ms);
        Ok(())
    }
}
