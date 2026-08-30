use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};

use crate::config::engine::PlaybackState;
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::PumpAudioMessage;

impl Handler<PumpAudioMessage> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: PumpAudioMessage,
    ) -> Result<(), ActorError> {
        self.pump_scheduled = false;
        let next_delay = self
            .ensure_session()
            .ok()
            .and_then(|session| session.pump_turn());
        if let Some(turn) = next_delay {
            if *ctx.behavior() == PlaybackState::Recovering && !turn.recovering {
                self.transition_state(ctx, PlaybackState::Playing);
            }
            self.pump_scheduled = true;
            ctx.notify_after(turn.next_delay, PumpAudioMessage);
        }
        Ok(())
    }
}
