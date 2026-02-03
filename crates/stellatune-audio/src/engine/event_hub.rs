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
        self.subscribers
            .lock()
            .expect("event hub mutex poisoned")
            .push(tx);
        rx
    }

    pub(crate) fn emit(&self, event: Event) {
        let mut subs = self.subscribers.lock().expect("event hub mutex poisoned");
        subs.retain(|tx| tx.send(event.clone()).is_ok());
    }
}
