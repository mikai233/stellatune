use std::sync::Arc;

use crate::event_hub::EventHub;
use crate::types::{EngineConfig, EngineSnapshot, Event, PlayerState};
use crate::worker::decode_loop::DecodeLoopWorker;

pub(crate) struct ControlActor {
    pub(crate) events: Arc<EventHub>,
    pub(crate) config: EngineConfig,
    pub(crate) snapshot: EngineSnapshot,
    pub(crate) worker: Option<DecodeLoopWorker>,
}

impl ControlActor {
    pub(crate) fn new(events: Arc<EventHub>, config: EngineConfig) -> Self {
        Self {
            events,
            config,
            snapshot: EngineSnapshot::default(),
            worker: None,
        }
    }

    pub(crate) fn ensure_worker(&mut self) -> Result<&mut DecodeLoopWorker, String> {
        self.worker
            .as_mut()
            .ok_or_else(|| "decode loop is not installed".to_string())
    }

    pub(crate) fn emit_error(&self, message: String) {
        self.events.emit(Event::Error { message });
    }

    pub(crate) fn update_state(&mut self, next: PlayerState) {
        if self.snapshot.state == next {
            return;
        }
        self.snapshot.state = next;
        self.events.emit(Event::StateChanged { state: next });
    }

    pub(crate) fn update_position(&mut self, position_ms: i64) {
        self.snapshot.position_ms = position_ms.max(0);
        self.events.emit(Event::Position {
            position_ms: self.snapshot.position_ms,
        });
    }
}
