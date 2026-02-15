use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::{
    SessionStopMode, drop_output_pipeline, force_transition_gain_unity, stop_decode_session,
};
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct SetOutputOptionsMessage {
    pub(crate) match_track_sample_rate: bool,
    pub(crate) gapless_playback: bool,
    pub(crate) seek_track_fade: bool,
    pub(crate) resample_quality: crate::types::ResampleQuality,
}

impl Message for SetOutputOptionsMessage {
    type Response = Result<(), String>;
}

impl Handler<SetOutputOptionsMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SetOutputOptionsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, track_info) = (&mut self.state, &self.track_info);
        if !message.seek_track_fade {
            force_transition_gain_unity(state.session.as_ref());
        }
        state.seek_track_fade = message.seek_track_fade;

        let changed = state.match_track_sample_rate != message.match_track_sample_rate
            || state.gapless_playback != message.gapless_playback
            || state.resample_quality != message.resample_quality;
        if changed {
            state.match_track_sample_rate = message.match_track_sample_rate;
            state.gapless_playback = message.gapless_playback;
            state.resample_quality = message.resample_quality;
            if state.session.is_some() {
                stop_decode_session(state, track_info, SessionStopMode::TearDownSink);
                if state.wants_playback {
                    state.pending_session_start = true;
                }
            }
            if !state.gapless_playback {
                drop_output_pipeline(state);
            }
        }
        Ok(())
    }
}
