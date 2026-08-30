use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::config::engine::PlaybackState;
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::SwitchTrackMessage;
use crate::error::EngineError;

impl Responder<SwitchTrackMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: SwitchTrackMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        if *ctx.behavior() == PlaybackState::Reconfiguring {
            let _ = reply_to.send(Err(EngineError::PluginChangeInProgress {
                operation: "switch_track_token",
            }));
            return Ok(());
        }
        self.transition_state(ctx, PlaybackState::Preparing);
        let timeout = self.config.decode_command_timeout;
        let result = self.ensure_session().and_then(|worker| {
            worker
                .open(message.track_token, message.autoplay, timeout)
                .map_err(EngineError::from)
        });
        let next = if result.is_ok() {
            if message.autoplay {
                PlaybackState::Playing
            } else {
                PlaybackState::Ready
            }
        } else {
            self.pump_scheduled = false;
            self.current_track = None;
            self.position_ms = 0;
            PlaybackState::Idle
        };
        self.transition_state(ctx, next);
        if result.is_ok() && message.autoplay {
            self.schedule_pump(ctx, std::time::Duration::ZERO);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}
