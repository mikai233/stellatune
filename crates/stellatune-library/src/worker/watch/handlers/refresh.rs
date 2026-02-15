use crate::LibraryEvent;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::{WatchTaskActor, refresh_watch_state};

pub(crate) struct WatchRefreshMessage;

impl Message for WatchRefreshMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<WatchRefreshMessage> for WatchTaskActor {
    async fn handle(&mut self, _message: WatchRefreshMessage, _ctx: &mut ActorContext<Self>) -> () {
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        if let Err(err) =
            refresh_watch_state(&self.pool, watcher, &mut self.watched, &mut self.excluded).await
        {
            self.events.emit(LibraryEvent::Log {
                message: format!("fs watcher refresh failed: {err:#}"),
            });
        }
    }
}
