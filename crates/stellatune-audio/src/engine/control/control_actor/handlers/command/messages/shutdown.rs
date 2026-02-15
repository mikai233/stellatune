use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::stop_all_audio;

pub(crate) struct ShutdownMessage;

impl Message for ShutdownMessage {
    type Response = Result<(), String>;
}

impl Handler<ShutdownMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: ShutdownMessage,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        stop_all_audio(&mut self.state, &self.track_info);
        self.state.wants_playback = false;
        self.state.play_request_started_at = None;
        self.state.pending_session_start = false;
        self.ensure_shutdown();
        ctx.stop();
        Ok(())
    }
}
