use std::sync::Arc;

use crossbeam_channel::Sender;
use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::super::{
    DecodeCtrl, DisruptFadeKind, EngineState, Event, EventHub, InternalMsg, ManualSwitchTiming,
    PlayerState, SeekPositionGuard, SessionStopMode, SharedTrackInfo, debug_metrics,
    drop_output_pipeline, emit_position_event, enqueue_preload_task, ensure_output_spec_prewarm,
    flush_pending_plugin_disables, force_transition_gain_unity, maybe_fade_out_before_disrupt,
    next_position_session_id, output_backend_for_selected, parse_output_sink_route, set_state,
    stop_all_audio, stop_decode_session, sync_output_sink_with_active_session,
    track_ref_to_engine_token, track_ref_to_event_path,
};
use super::super::ControlActor;

mod lifecycle;
mod output;
mod playback;
mod preload;

struct CommandCtx<'a> {
    state: &'a mut EngineState,
    events: &'a Arc<EventHub>,
    internal_tx: &'a Sender<InternalMsg>,
    track_info: &'a SharedTrackInfo,
    actor_ref: stellatune_runtime::thread_actor::ActorRef<ControlActor>,
}

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
        ctx: &mut ActorContext<Self>,
    ) -> Result<super::super::super::CommandResponse, String> {
        let mut command_ctx = CommandCtx {
            state: &mut self.state,
            events: &self.events,
            internal_tx: &self.internal_tx,
            track_info: &self.track_info,
            actor_ref: ctx.actor_ref(),
        };

        let response = match message.command {
            stellatune_core::Command::SwitchTrackRef { track, lazy } => {
                playback::on_switch_track_ref(&mut command_ctx, track, lazy)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::Play => playback::on_play(&mut command_ctx)
                .map(|_| super::super::super::CommandResponse::Ack),
            stellatune_core::Command::Pause => playback::on_pause(&mut command_ctx)
                .map(|_| super::super::super::CommandResponse::Ack),
            stellatune_core::Command::SeekMs { position_ms } => {
                playback::on_seek_ms(&mut command_ctx, position_ms)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::SetVolume { volume } => {
                output::on_set_volume(&mut command_ctx, volume)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::SetLfeMode { mode } => {
                output::on_set_lfe_mode(&mut command_ctx, mode)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::Stop => playback::on_stop(&mut command_ctx)
                .map(|_| super::super::super::CommandResponse::Ack),
            stellatune_core::Command::SetOutputDevice { backend, device_id } => {
                output::on_set_output_device(&mut command_ctx, backend, device_id)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::SetOutputOptions {
                match_track_sample_rate,
                gapless_playback,
                seek_track_fade,
            } => output::on_set_output_options(
                &mut command_ctx,
                match_track_sample_rate,
                gapless_playback,
                seek_track_fade,
            )
            .map(|_| super::super::super::CommandResponse::Ack),
            stellatune_core::Command::SetOutputSinkRoute { route } => {
                output::on_set_output_sink_route(&mut command_ctx, route)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::ClearOutputSinkRoute => {
                output::on_clear_output_sink_route(&mut command_ctx)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::PreloadTrack { path, position_ms } => {
                preload::on_preload_track(&mut command_ctx, path, position_ms)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::PreloadTrackRef { track, position_ms } => {
                preload::on_preload_track_ref(&mut command_ctx, track, position_ms)
                    .map(|_| super::super::super::CommandResponse::Ack)
            }
            stellatune_core::Command::RefreshDevices => {
                output::on_refresh_devices(&mut command_ctx)
                    .map(|devices| super::super::super::CommandResponse::OutputDevices { devices })
            }
            stellatune_core::Command::Shutdown => {
                lifecycle::on_shutdown(&mut command_ctx);
                self.ensure_shutdown();
                ctx.stop();
                return Ok(super::super::super::CommandResponse::Ack);
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
