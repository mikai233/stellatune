use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::{PlayerState, emit_position_event};

const SEEK_POSITION_GUARD_TIMEOUT_MS: u64 = 1200;
const SEEK_POSITION_ACCEPT_TOLERANCE_MS: i64 = 400;

pub(crate) struct PositionInternalMessage {
    pub(crate) path: String,
    pub(crate) ms: i64,
}

impl Message for PositionInternalMessage {
    type Response = ();
}

impl Handler<PositionInternalMessage> for ControlActor {
    fn handle(&mut self, message: PositionInternalMessage, _ctx: &mut ActorContext<Self>) {
        if self.state.session.is_none() {
            return;
        }
        if self.state.current_track.as_deref() != Some(message.path.as_str()) {
            return;
        }
        if let Some(guard) = self.state.seek_position_guard.as_ref() {
            let elapsed_ms = guard.requested_at.elapsed().as_millis() as u64;
            let forward = guard.target_ms >= guard.origin_ms;
            let reached_target = if forward {
                message.ms
                    >= guard
                        .target_ms
                        .saturating_sub(SEEK_POSITION_ACCEPT_TOLERANCE_MS)
            } else {
                message.ms
                    <= guard
                        .target_ms
                        .saturating_add(SEEK_POSITION_ACCEPT_TOLERANCE_MS)
            };
            if reached_target || elapsed_ms >= SEEK_POSITION_GUARD_TIMEOUT_MS {
                self.state.seek_position_guard = None;
            } else {
                return;
            }
        }
        if self.state.player_state == PlayerState::Buffering
            && self.state.position_ms <= 1000
            && message.ms > 5000
        {
            return;
        }
        self.state.position_ms = message.ms;
        emit_position_event(&self.state, &self.events);
    }
}
