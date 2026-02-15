pub(crate) mod handlers;

use std::sync::Arc;

use tracing::info;

use super::{
    EngineState, InternalDispatchTx, SharedTrackInfo, shutdown_decode_worker,
    shutdown_preload_worker, stop_all_audio,
};
use crate::engine::event_hub::EventHub;

pub(crate) struct ControlActor {
    pub(super) state: EngineState,
    pub(super) events: Arc<EventHub>,
    pub(super) track_info: SharedTrackInfo,
    pub(super) internal_tx: InternalDispatchTx,
    shutdown_done: bool,
}

impl ControlActor {
    pub(super) fn new(
        state: EngineState,
        events: Arc<EventHub>,
        track_info: SharedTrackInfo,
        internal_tx: InternalDispatchTx,
    ) -> Self {
        Self {
            state,
            events,
            track_info,
            internal_tx,
            shutdown_done: false,
        }
    }

    pub(crate) fn ensure_shutdown(&mut self) {
        if self.shutdown_done {
            return;
        }
        stop_all_audio(&mut self.state, &self.track_info);
        shutdown_decode_worker(&mut self.state);
        shutdown_preload_worker(&mut self.state);
        self.events.emit(crate::types::Event::Log {
            message: "control thread exited".to_string(),
        });
        info!("control thread exited");
        self.shutdown_done = true;
    }
}

impl Drop for ControlActor {
    fn drop(&mut self) {
        self.ensure_shutdown();
    }
}
