use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};
use tokio::time::Instant;

use crate::LibraryEvent;
use crate::worker::watch::{WatchTaskActor, apply_fs_changes};

#[derive(lattice_actor::Message)]
pub(crate) struct WatchTickMessage;

impl Handler<WatchTickMessage> for WatchTaskActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: WatchTickMessage,
    ) -> Result<(), ActorError> {
        let Some(deadline) = self.debounce_deadline else {
            return Ok(());
        };
        if Instant::now() < deadline {
            return Ok(());
        }
        if self.dirty.is_empty() {
            self.debounce_deadline = None;
            return Ok(());
        }

        let batch = self.dirty.drain().collect::<Vec<_>>();
        self.debounce_deadline = None;
        match apply_fs_changes(
            &self.pool,
            &self.events,
            &self.cover_dir,
            &self.excluded,
            batch,
            &self.metadata_provider,
        )
        .await
        {
            Ok(true) => self.events.emit(LibraryEvent::Changed),
            Ok(false) => {},
            Err(err) => self.events.emit(LibraryEvent::Log {
                message: format!("fs sync error: {err:#}"),
            }),
        }
        Ok(())
    }
}
