use std::sync::atomic::Ordering;
use std::time::Instant;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::super::tick::ControlTickMessage;
use super::super::{
    DecodeCtrl, PlayerState, ensure_output_spec_prewarm, force_transition_gain_unity, set_state,
};
use super::emit_and_err;
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct PlayMessage;

impl Message for PlayMessage {
    type Response = Result<(), String>;
}

impl Handler<PlayMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: PlayMessage,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, events, internal_tx) = (&mut self.state, &self.events, &self.internal_tx);
        let Some(path) = state.current_track.clone() else {
            return emit_and_err(events, "no track loaded");
        };

        let requested_at = Instant::now();
        state.wants_playback = true;
        state.play_request_started_at = Some(requested_at);
        if let Some(timing) = state.manual_switch_timing.as_mut()
            && timing.to_track == path
            && timing.play_requested_at.is_none()
        {
            timing.play_requested_at = Some(requested_at);
        }

        if state.session.is_none() {
            state.pending_session_start = true;
            set_state(state, events, PlayerState::Buffering);
            ensure_output_spec_prewarm(state, internal_tx);
            let _ = ctx.actor_ref().cast(ControlTickMessage);
            return Ok(());
        }

        if let Some(session) = state.session.as_ref() {
            force_transition_gain_unity(Some(session));
            session.output_enabled.store(false, Ordering::Release);
            let _ = session.ctrl_tx.send(DecodeCtrl::Play);
        }

        set_state(state, events, PlayerState::Buffering);
        let _ = ctx.actor_ref().cast(ControlTickMessage);
        Ok(())
    }
}
