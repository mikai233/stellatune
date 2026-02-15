use tokio::time::Instant;

use crate::LibraryEvent;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::worker::watch::{WatchTaskActor, apply_fs_changes};

pub(crate) struct WatchTickMessage;

impl Message for WatchTickMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<WatchTickMessage> for WatchTaskActor {
    async fn handle(&mut self, _message: WatchTickMessage, _ctx: &mut ActorContext<Self>) -> () {
        let Some(deadline) = self.debounce_deadline else {
            return;
        };
        if Instant::now() < deadline {
            return;
        }

        if self.dirty.is_empty() {
            self.debounce_deadline = None;
            return;
        }

        let batch = self.dirty.drain().collect::<Vec<_>>();
        self.debounce_deadline = None;
        match apply_fs_changes(
            &self.pool,
            &self.events,
            &self.cover_dir,
            &self.excluded,
            batch,
        )
        .await
        {
            Ok(true) => self.events.emit(LibraryEvent::Changed),
            Ok(false) => {},
            Err(err) => self.events.emit(LibraryEvent::Log {
                message: format!("fs sync error: {err:#}"),
            }),
        }
    }
}
