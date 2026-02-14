use std::sync::atomic::Ordering;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::{
    DecodeCtrl, DisruptFadeKind, PlayerState, maybe_fade_out_before_disrupt, set_state,
};
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct PauseMessage;

impl Message for PauseMessage {
    type Response = Result<(), String>;
}

impl Handler<PauseMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: PauseMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, events) = (&mut self.state, &self.events);
        if let Some(session) = state.session.as_ref() {
            maybe_fade_out_before_disrupt(state, DisruptFadeKind::TrackSwitch);
            session.output_enabled.store(false, Ordering::Release);
            let _ = session.ctrl_tx.send(DecodeCtrl::Pause);
        }
        state.wants_playback = false;
        state.play_request_started_at = None;
        state.pending_session_start = false;
        set_state(state, events, PlayerState::Paused);
        Ok(())
    }
}
