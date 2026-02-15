use std::sync::atomic::Ordering;
use std::time::Instant;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::super::tick::ControlTickMessage;
use super::super::{
    DecodeCtrl, DisruptFadeKind, PlayerState, SeekPositionGuard, emit_position_event,
    maybe_fade_out_before_disrupt, next_position_session_id, set_state,
};
use crate::engine::control::Event;
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct SeekMsMessage {
    pub(crate) position_ms: u64,
}

impl Message for SeekMsMessage {
    type Response = Result<(), String>;
}

impl Handler<SeekMsMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SeekMsMessage,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, events) = (&mut self.state, &self.events);
        if state.current_track.is_none() {
            let message = "no track loaded".to_string();
            events.emit(Event::Error {
                message: message.clone(),
            });
            return Err(message);
        }

        maybe_fade_out_before_disrupt(state, DisruptFadeKind::Seek);
        let target_ms = (message.position_ms as i64).max(0);
        let origin_ms = state.position_ms.max(0);
        state.seek_position_guard = Some(SeekPositionGuard {
            target_ms,
            origin_ms,
            requested_at: Instant::now(),
        });
        next_position_session_id(state);
        state.position_ms = target_ms;
        emit_position_event(state, events);

        if let Some(session) = state.session.as_ref() {
            session.output_enabled.store(false, Ordering::Release);
            let _ = session.ctrl_tx.send(DecodeCtrl::SeekMs {
                position_ms: target_ms,
            });
        }

        if state.wants_playback
            && matches!(
                state.player_state,
                PlayerState::Playing | PlayerState::Buffering
            )
        {
            set_state(state, events, PlayerState::Buffering);
            state.play_request_started_at = Some(Instant::now());
            let _ = ctx.actor_ref().cast(ControlTickMessage);
        }
        Ok(())
    }
}
