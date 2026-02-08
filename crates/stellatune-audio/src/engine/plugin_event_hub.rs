use std::sync::Mutex;

use crossbeam_channel::{Receiver, Sender};
use stellatune_core::PluginRuntimeEvent;

pub(crate) struct PluginEventHub {
    subscribers: Mutex<Vec<Sender<PluginRuntimeEvent>>>,
}

impl PluginEventHub {
    pub(crate) fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn subscribe(&self) -> Receiver<PluginRuntimeEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.push(tx);
        }
        rx
    }

    pub(crate) fn emit(&self, event: PluginRuntimeEvent) {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.retain(|tx| tx.send(event.clone()).is_ok());
        }
    }
}
