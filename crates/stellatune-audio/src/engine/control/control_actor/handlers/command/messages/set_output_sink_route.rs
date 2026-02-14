use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::{
    PlayerState, SessionStopMode, drop_output_pipeline, ensure_output_spec_prewarm,
    parse_output_sink_route, set_state, stop_decode_session, sync_output_sink_with_active_session,
};
use super::emit_and_err;
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct SetOutputSinkRouteMessage {
    pub(crate) route: stellatune_core::OutputSinkRoute,
}

impl Message for SetOutputSinkRouteMessage {
    type Response = Result<(), String>;
}

impl Handler<SetOutputSinkRouteMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SetOutputSinkRouteMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, events, internal_tx, track_info) = (
            &mut self.state,
            &self.events,
            &self.internal_tx,
            &self.track_info,
        );
        let parsed_route = match parse_output_sink_route(message.route) {
            Ok(route) => route,
            Err(err) => return emit_and_err(events, err),
        };
        let mode_changed = state.desired_output_sink_route.is_none();
        let route_changed = state.desired_output_sink_route.as_ref() != Some(&parsed_route);
        state.desired_output_sink_route = Some(parsed_route);
        if mode_changed || route_changed {
            state.output_sink_chunk_frames = 0;
            state.output_sink_negotiation_cache = None;
            state.cached_output_spec = None;
            state.output_spec_prewarm_inflight = false;
            state.output_spec_token = state.output_spec_token.wrapping_add(1);
            ensure_output_spec_prewarm(state, internal_tx);
            let resume_playback = state.wants_playback;
            if state.session.is_some() {
                stop_decode_session(state, track_info, SessionStopMode::TearDownSink);
                drop_output_pipeline(state);
            }
            if resume_playback {
                state.pending_session_start = true;
                set_state(state, events, PlayerState::Buffering);
            }
        }
        if let Err(err) = sync_output_sink_with_active_session(state, internal_tx) {
            return emit_and_err(events, err);
        }
        Ok(())
    }
}
