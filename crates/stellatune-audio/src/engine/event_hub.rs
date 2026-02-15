use tokio::sync::broadcast;

use crate::types::Event;

pub(crate) struct EventHub {
    tx: broadcast::Sender<Event>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub(crate) fn emit(&self, event: Event) {
        let _ = self.tx.send(event);
    }
}
