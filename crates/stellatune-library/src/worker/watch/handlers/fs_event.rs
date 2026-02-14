use std::time::Duration;

use tokio::time::Instant;

use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::{WATCH_DEBOUNCE_MS, WatchTaskActor};

pub(crate) struct WatchFsEventMessage {
    pub(crate) result: notify::Result<notify::Event>,
}

impl Message for WatchFsEventMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<WatchFsEventMessage> for WatchTaskActor {
    async fn handle(&mut self, message: WatchFsEventMessage, _ctx: &mut ActorContext<Self>) -> () {
        let event = match message.result {
            Ok(v) => v,
            Err(_) => return,
        };
        for path in event.paths {
            let raw = path.to_string_lossy().to_string();
            if !raw.trim().is_empty() {
                self.dirty.insert(raw);
            }
        }
        self.debounce_deadline = Some(Instant::now() + Duration::from_millis(WATCH_DEBOUNCE_MS));
    }
}
