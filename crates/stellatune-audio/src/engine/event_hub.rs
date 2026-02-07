use std::sync::Mutex;

use crossbeam_channel::{Receiver, Sender};

use stellatune_core::Event;

pub(crate) struct EventHub {
    subscribers: Mutex<Vec<Sender<Event>>>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = crossbeam_channel::unbounded();
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.push(tx);
        }
        rx
    }

    pub(crate) fn emit(&self, event: Event) {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.retain(|tx| tx.send(event.clone()).is_ok());
        }
    }
}
