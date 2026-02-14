use stellatune_core::{LibraryCommand, LibraryEvent};
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LibraryServiceActor;

pub(crate) struct LibraryCommandMessage {
    pub(crate) command: LibraryCommand,
}

impl Message for LibraryCommandMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<LibraryCommandMessage> for LibraryServiceActor {
    async fn handle(&mut self, message: LibraryCommandMessage, ctx: &mut ActorContext<Self>) -> () {
        let is_shutdown = matches!(message.command, LibraryCommand::Shutdown);
        if let Err(err) = self.worker.handle_command(message.command).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }

        if is_shutdown {
            tracing::info!("library actor exiting");
            ctx.stop();
        }
    }
}
