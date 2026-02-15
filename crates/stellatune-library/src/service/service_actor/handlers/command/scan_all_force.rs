use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct ScanAllForceMessage;

impl Message for ScanAllForceMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<ScanAllForceMessage> for LibraryServiceActor {
    async fn handle(&mut self, _message: ScanAllForceMessage, _ctx: &mut ActorContext<Self>) -> () {
        if let Err(err) = self.worker.scan_all(true).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
