use std::sync::Arc;

use stellatune_runtime::thread_actor::ActorRef;

use crate::engine::actor::ControlActor;
use crate::error::EngineError;
use crate::infra::event_hub::EventHub;
use crate::pipeline::runtime::dsp::control::SharedMasterGainHotControl;

mod control_ops;
mod pipeline_ops;
mod transport;

#[derive(Clone)]
pub struct EngineHandle {
    actor_ref: ActorRef<ControlActor>,
    events: Arc<EventHub>,
    master_gain_hot_control: SharedMasterGainHotControl,
    timeout: std::time::Duration,
}

impl EngineHandle {
    pub(crate) fn new(
        actor_ref: ActorRef<ControlActor>,
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
        err: stellatune_runtime::thread_actor::CallError,
    ) -> EngineError {
        EngineError::from_call_error(operation, timeout, err)
    }
}
