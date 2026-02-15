use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::{debug_metrics, enqueue_preload_task};

pub(crate) struct PreloadTrackMessage {
    pub(crate) path: String,
    pub(crate) position_ms: u64,
}

impl Message for PreloadTrackMessage {
    type Response = Result<(), String>;
}

impl Handler<PreloadTrackMessage> for ControlActor {
    fn handle(
        &mut self,
        message: PreloadTrackMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let state = &mut self.state;
        let path = message.path.trim().to_string();
        if path.is_empty()
            || (state.requested_preload_path.as_deref() == Some(path.as_str())
                && state.requested_preload_position_ms == message.position_ms)
        {
            return Ok(());
        }
        state.requested_preload_path = Some(path.clone());
        state.requested_preload_position_ms = message.position_ms;
        state.preload_token = state.preload_token.wrapping_add(1);
        debug_metrics::note_preload_request();
        enqueue_preload_task(state, path, message.position_ms, state.preload_token);
        Ok(())
    }
}
