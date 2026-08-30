use super::{ActorContext, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct ShutdownMessage;

impl Handler<ShutdownMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        ctx: &mut ActorContext<'_, Self>,
        _message: ShutdownMessage,
    ) -> Result<(), ActorError> {
        tracing::info!("library actor exiting");
        ctx.request_stop();
        Ok(())
    }
}
