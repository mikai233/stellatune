use tokio::sync::broadcast;

use crate::config::engine::Event;

pub(crate) struct EventHub {
    tx: broadcast::Sender<Event>,
}

impl EventHub {
    pub(crate) fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity.max(1));
        Self { tx }
    }

    pub(crate) fn emit(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}
