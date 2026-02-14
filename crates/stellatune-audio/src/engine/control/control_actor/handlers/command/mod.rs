use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::super::{
    DecodeCtrl, DisruptFadeKind, EngineState, Event, ManualSwitchTiming, PlayerState,
    SeekPositionGuard, SessionStopMode, SharedTrackInfo, debug_metrics, drop_output_pipeline,
    emit_position_event, enqueue_preload_task, ensure_output_spec_prewarm,
    flush_pending_plugin_disables, force_transition_gain_unity, maybe_fade_out_before_disrupt,
    next_position_session_id, output_backend_for_selected, parse_output_sink_route, set_state,
    stop_all_audio, stop_decode_session, sync_output_sink_with_active_session,
    track_ref_to_engine_token, track_ref_to_event_path,
};
use super::super::ControlActor;

pub(crate) mod messages;

fn ui_volume_to_gain(ui: f32) -> f32 {
    if ui <= 0.0 {
        return 0.0;
    }
    const MIN_DB: f32 = -30.0;
    let db = MIN_DB * (1.0 - ui);
    10.0_f32.powf(db / 20.0)
}

pub(crate) struct ControlCommandMessage {
    pub(crate) command: stellatune_core::Command,
}

impl Message for ControlCommandMessage {
    type Response = Result<super::super::super::CommandResponse, String>;
}

impl Handler<ControlCommandMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ControlCommandMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<super::super::super::CommandResponse, String> {
        let state = &mut self.state;

        let response = match message.command {
            stellatune_core::Command::SetLfeMode { mode } => {
                state.lfe_mode = mode;
                if let Some(session) = state.session.as_ref() {
                    let _ = session.ctrl_tx.send(DecodeCtrl::SetLfeMode { mode });
                }
                Ok(super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::SwitchTrackRef { .. }
            | stellatune_core::Command::Play
            | stellatune_core::Command::Pause
            | stellatune_core::Command::SeekMs { .. }
            | stellatune_core::Command::Stop
            | stellatune_core::Command::PreloadTrack { .. }
            | stellatune_core::Command::PreloadTrackRef { .. }
            | stellatune_core::Command::SetVolume { .. }
            | stellatune_core::Command::SetOutputDevice { .. }
            | stellatune_core::Command::SetOutputOptions { .. }
            | stellatune_core::Command::SetOutputSinkRoute { .. }
            | stellatune_core::Command::ClearOutputSinkRoute
            | stellatune_core::Command::RefreshDevices => {
                Err("command path moved to dedicated handlers".to_string())
            }
            stellatune_core::Command::Shutdown => {
                Err("command path moved to dedicated handlers".to_string())
            }
        };

        if let Err(error_message) = &response {
            self.events.emit(Event::Error {
                message: error_message.clone(),
            });
        }

        response
    }
}
