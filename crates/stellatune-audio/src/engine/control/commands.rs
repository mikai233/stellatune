use std::sync::Arc;

use crossbeam_channel::Sender;

use super::{
    Command, DecodeCtrl, EngineState, Event, EventHub, InternalMsg, PlayerState, PluginEventHub,
    SharedTrackInfo, StartSessionArgs, apply_dsp_chain, debug_metrics, drop_output_pipeline,
    enqueue_preload_task, ensure_output_spec_prewarm, force_transition_gain_unity, handle_tick,
    maybe_fade_out_before_disrupt, output_backend_for_selected, parse_output_sink_route,
    resolve_output_spec_and_sink_chunk, set_state, start_session, stop_all_audio,
    stop_decode_session, sync_output_sink_with_active_session, track_ref_to_engine_token,
    track_ref_to_event_path,
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
use playback::{on_load_track, on_load_track_ref, on_pause, on_play, on_seek_ms, on_stop};
use preload::{on_preload_track, on_preload_track_ref};

struct CommandCtx<'a> {
    state: &'a mut EngineState,
    events: &'a Arc<EventHub>,
    plugin_events: &'a Arc<PluginEventHub>,
    internal_tx: &'a Sender<InternalMsg>,
    track_info: &'a SharedTrackInfo,
}

pub(super) fn handle_command(
    cmd: Command,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    plugin_events: &Arc<PluginEventHub>,
    internal_tx: &Sender<InternalMsg>,
    track_info: &SharedTrackInfo,
) -> bool {
    let mut ctx = CommandCtx {
        state,
        events,
        plugin_events,
        internal_tx,
        track_info,
    };

    match cmd {
        Command::LoadTrack { path } => on_load_track(&mut ctx, path),
        Command::LoadTrackRef { track } => on_load_track_ref(&mut ctx, track),
        Command::Play => on_play(&mut ctx),
        Command::Pause => on_pause(&mut ctx),
        Command::SeekMs { position_ms } => on_seek_ms(&mut ctx, position_ms),
        Command::SetVolume { volume } => on_set_volume(&mut ctx, volume),
        Command::SetLfeMode { mode } => on_set_lfe_mode(&mut ctx, mode),
        Command::Stop => on_stop(&mut ctx),
        Command::SetOutputDevice { backend, device_id } => {
            on_set_output_device(&mut ctx, backend, device_id)
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
        ),
        Command::SetOutputSinkRoute { route } => on_set_output_sink_route(&mut ctx, route),
        Command::ClearOutputSinkRoute => on_clear_output_sink_route(&mut ctx),
        Command::PreloadTrack { path, position_ms } => {
            on_preload_track(&mut ctx, path, position_ms)
        }
        Command::PreloadTrackRef { track, position_ms } => {
            on_preload_track_ref(&mut ctx, track, position_ms)
        }
        Command::RefreshDevices => on_refresh_devices(&mut ctx),
        Command::Shutdown => return on_shutdown(&mut ctx),
    }

    false
}

fn ui_volume_to_gain(ui: f32) -> f32 {
    if ui <= 0.0 {
        return 0.0;
    }
    const MIN_DB: f32 = -30.0;
    let db = MIN_DB * (1.0 - ui);
    10.0_f32.powf(db / 20.0)
}
