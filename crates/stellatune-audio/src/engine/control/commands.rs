use std::sync::Arc;

use crossbeam_channel::Sender;

use super::{
    Command, CommandResponse, DecodeCtrl, EngineState, Event, EventHub, InternalMsg, PlayerState,
    SessionStopMode, SharedTrackInfo, debug_metrics, drop_output_pipeline, enqueue_preload_task,
    ensure_output_spec_prewarm, force_transition_gain_unity, handle_tick,
    maybe_fade_out_before_disrupt, output_backend_for_selected, parse_output_sink_route, set_state,
    stop_all_audio, stop_decode_session, sync_output_sink_with_active_session,
    track_ref_to_engine_token, track_ref_to_event_path,
};

mod lifecycle;
mod output;
mod playback;
mod preload;

use lifecycle::on_shutdown;
use output::{
    on_clear_output_sink_route, on_refresh_devices, on_set_lfe_mode, on_set_output_device,
    on_set_output_options, on_set_output_sink_route, on_set_volume,
};
use playback::{on_pause, on_play, on_seek_ms, on_stop, on_switch_track_ref};
use preload::{on_preload_track, on_preload_track_ref};

struct CommandCtx<'a> {
    state: &'a mut EngineState,
    events: &'a Arc<EventHub>,
    internal_tx: &'a Sender<InternalMsg>,
    track_info: &'a SharedTrackInfo,
}

pub(super) struct CommandHandleResult {
    pub(super) should_shutdown: bool,
    pub(super) response: Result<CommandResponse, String>,
}

pub(super) fn handle_command(
    cmd: Command,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
    track_info: &SharedTrackInfo,
) -> CommandHandleResult {
    let mut ctx = CommandCtx {
        state,
        events,
        internal_tx,
        track_info,
    };

    let response = match cmd {
        Command::SwitchTrackRef { track, lazy } => {
            on_switch_track_ref(&mut ctx, track, lazy).map(|_| CommandResponse::Ack)
        }
        Command::Play => on_play(&mut ctx).map(|_| CommandResponse::Ack),
        Command::Pause => on_pause(&mut ctx).map(|_| CommandResponse::Ack),
        Command::SeekMs { position_ms } => {
            on_seek_ms(&mut ctx, position_ms).map(|_| CommandResponse::Ack)
        }
        Command::SetVolume { volume } => {
            on_set_volume(&mut ctx, volume).map(|_| CommandResponse::Ack)
        }
        Command::SetLfeMode { mode } => {
            on_set_lfe_mode(&mut ctx, mode).map(|_| CommandResponse::Ack)
        }
        Command::Stop => on_stop(&mut ctx).map(|_| CommandResponse::Ack),
        Command::SetOutputDevice { backend, device_id } => {
            on_set_output_device(&mut ctx, backend, device_id).map(|_| CommandResponse::Ack)
        }
        Command::SetOutputOptions {
            match_track_sample_rate,
            gapless_playback,
            seek_track_fade,
        } => on_set_output_options(
            &mut ctx,
            match_track_sample_rate,
            gapless_playback,
            seek_track_fade,
        )
        .map(|_| CommandResponse::Ack),
        Command::SetOutputSinkRoute { route } => {
            on_set_output_sink_route(&mut ctx, route).map(|_| CommandResponse::Ack)
        }
        Command::ClearOutputSinkRoute => {
            on_clear_output_sink_route(&mut ctx).map(|_| CommandResponse::Ack)
        }
        Command::PreloadTrack { path, position_ms } => {
            on_preload_track(&mut ctx, path, position_ms).map(|_| CommandResponse::Ack)
        }
        Command::PreloadTrackRef { track, position_ms } => {
            on_preload_track_ref(&mut ctx, track, position_ms).map(|_| CommandResponse::Ack)
        }
        Command::RefreshDevices => {
            on_refresh_devices(&mut ctx).map(|devices| CommandResponse::OutputDevices { devices })
        }
        Command::Shutdown => {
            on_shutdown(&mut ctx);
            return CommandHandleResult {
                should_shutdown: true,
                response: Ok(CommandResponse::Ack),
            };
        }
    };

    if let Err(message) = &response {
        events.emit(Event::Error {
            message: message.clone(),
        });
    }

    CommandHandleResult {
        should_shutdown: false,
        response,
    }
}

fn ui_volume_to_gain(ui: f32) -> f32 {
    if ui <= 0.0 {
        return 0.0;
    }
    const MIN_DB: f32 = -30.0;
    let db = MIN_DB * (1.0 - ui);
    10.0_f32.powf(db / 20.0)
}
