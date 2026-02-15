use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct SetTrackLikedMessage {
    pub(crate) track_id: i64,
    pub(crate) liked: bool,
}

impl Message for SetTrackLikedMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<SetTrackLikedMessage> for LibraryServiceActor {
    async fn handle(&mut self, message: SetTrackLikedMessage, _ctx: &mut ActorContext<Self>) -> () {
        if let Err(err) = self
            .worker
            .set_track_liked(message.track_id, message.liked)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
