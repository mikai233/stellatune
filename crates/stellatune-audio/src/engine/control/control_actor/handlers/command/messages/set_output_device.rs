use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::{
    SessionStopMode, drop_output_pipeline, ensure_output_spec_prewarm, stop_decode_session,
};
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct SetOutputDeviceMessage {
    pub(crate) backend: stellatune_core::AudioBackend,
    pub(crate) device_id: Option<String>,
}

impl Message for SetOutputDeviceMessage {
    type Response = Result<(), String>;
}

impl Handler<SetOutputDeviceMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SetOutputDeviceMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, internal_tx, track_info) =
            (&mut self.state, &self.internal_tx, &self.track_info);
        state.selected_backend = message.backend;
        state.selected_device_id = message.device_id;
        state.cached_output_spec = None;
        state.output_sink_negotiation_cache = None;
        state.output_spec_prewarm_inflight = false;
        state.output_spec_token = state.output_spec_token.wrapping_add(1);
        ensure_output_spec_prewarm(state, internal_tx);
        if state.session.is_some() {
            stop_decode_session(state, track_info, SessionStopMode::TearDownSink);
        }
        drop_output_pipeline(state);
        if state.wants_playback {
            state.pending_session_start = true;
        }
        Ok(())
    }
}
