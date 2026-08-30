use std::time::Duration;

use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};
use tokio::time::Instant;

use crate::worker::watch::{WATCH_DEBOUNCE_MS, WatchTaskActor};

#[derive(lattice_actor::Message)]
pub(crate) struct WatchFsEventMessage {
    pub(crate) result: notify::Result<notify::Event>,
}

impl Handler<WatchFsEventMessage> for WatchTaskActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: WatchFsEventMessage,
    ) -> Result<(), ActorError> {
        let event = match message.result {
            Ok(value) => value,
            Err(_) => return Ok(()),
        };
        for path in event.paths {
            let raw = path.to_string_lossy().to_string();
            if !raw.trim().is_empty() {
                self.dirty.insert(raw);
            }
        }
        self.debounce_deadline = Some(Instant::now() + Duration::from_millis(WATCH_DEBOUNCE_MS));
        Ok(())
    }
}
