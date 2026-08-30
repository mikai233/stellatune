use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};

use crate::LibraryEvent;
use crate::worker::watch::{WatchTaskActor, refresh_watch_state};

#[derive(lattice_actor::Message)]
pub(crate) struct WatchRefreshMessage;

impl Handler<WatchRefreshMessage> for WatchTaskActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: WatchRefreshMessage,
    ) -> Result<(), ActorError> {
        let Some(watcher) = self.watcher.as_mut() else {
            return Ok(());
        };
        if let Err(err) =
            refresh_watch_state(&self.pool, watcher, &mut self.watched, &mut self.excluded).await
        {
            self.events.emit(LibraryEvent::Log {
                message: format!("fs watcher refresh failed: {err:#}"),
            });
        }
        Ok(())
    }
}
