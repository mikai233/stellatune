use std::sync::Arc;

use lattice_actor::handle::ActorHandle;

use crate::engine::actor::PlaybackActor;
use crate::error::EngineError;
use crate::infra::event_hub::EventHub;
use crate::pipeline::runtime::dsp::control::SharedMasterGainHotControl;

mod control_ops;
mod pipeline_ops;
mod transport;

/// Shared handle for issuing engine commands and subscribing to runtime events.
///
/// Clones of this handle reference the same underlying control actor and event
/// hub. Methods are asynchronous when they require actor round-trips.
#[derive(Clone)]
pub struct EngineHandle {
    actor_ref: ActorHandle<PlaybackActor>,
    events: Arc<EventHub>,
    master_gain_hot_control: SharedMasterGainHotControl,
    timeout: std::time::Duration,
}

impl EngineHandle {
    pub(crate) fn new(
        actor_ref: ActorHandle<PlaybackActor>,
        events: Arc<EventHub>,
        master_gain_hot_control: SharedMasterGainHotControl,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            actor_ref,
            events,
            master_gain_hot_control,
            timeout,
        }
    }

    pub(crate) fn map_call_error(
        operation: &'static str,
        timeout: std::time::Duration,
        err: lattice_actor::error::ActorCallError,
    ) -> EngineError {
        EngineError::from_call_error(operation, timeout, err)
    }
}
