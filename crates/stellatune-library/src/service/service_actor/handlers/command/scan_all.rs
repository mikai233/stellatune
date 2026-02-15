use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct ScanAllMessage;

impl Message for ScanAllMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<ScanAllMessage> for LibraryServiceActor {
    async fn handle(&mut self, _message: ScanAllMessage, _ctx: &mut ActorContext<Self>) -> () {
        if let Err(err) = self.worker.scan_all(false).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
