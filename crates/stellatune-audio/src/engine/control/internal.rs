use std::sync::Arc;

use crossbeam_channel::Sender;

use super::{
    EngineState, Event, EventHub, InternalMsg, PlayerState, SessionStopMode, SharedTrackInfo,
    debug_metrics, drop_output_pipeline, ensure_output_spec_prewarm, event_path_from_engine_token,
    on_plugin_reload_finished, set_state, stop_all_audio, stop_decode_session,
};

mod errors;
mod output_spec;
mod preload;

use errors::{on_eof, on_error, on_output_error, on_position};
use output_spec::{on_output_spec_failed, on_output_spec_ready};
use preload::{PreloadReadyArgs, on_preload_failed, on_preload_ready};

struct InternalCtx<'a> {
    state: &'a mut EngineState,
    events: &'a Arc<EventHub>,
    internal_tx: &'a Sender<InternalMsg>,
    track_info: &'a SharedTrackInfo,
}

pub(super) fn handle_internal(
    msg: InternalMsg,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
    track_info: &SharedTrackInfo,
) {
    let mut ctx = InternalCtx {
        state,
        events,
        internal_tx,
        track_info,
    };

    match msg {
        InternalMsg::Eof => on_eof(&mut ctx),
        InternalMsg::Error(message) => on_error(&mut ctx, message),
        InternalMsg::OutputError(message) => on_output_error(&mut ctx, message),
        InternalMsg::Position { path, ms } => on_position(&mut ctx, path, ms),
        InternalMsg::OutputSpecReady {
            spec,
            took_ms,
            token,
        } => on_output_spec_ready(&mut ctx, spec, took_ms, token),
        InternalMsg::OutputSpecFailed {
            message,
            took_ms,
            token,
        } => on_output_spec_failed(&mut ctx, message, took_ms, token),
        InternalMsg::PreloadReady {
            path,
            position_ms,
            decoder,
            track_info,
            chunk,
            took_ms,
            token,
        } => on_preload_ready(
            &mut ctx,
            PreloadReadyArgs {
                path,
                position_ms,
                decoder,
                track_info,
                chunk,
                took_ms,
                token,
            },
        ),
        InternalMsg::PreloadFailed {
            path,
            position_ms,
            message,
            took_ms,
            token,
        } => on_preload_failed(&mut ctx, path, position_ms, message, took_ms, token),
        InternalMsg::PluginsReloadFinished { summary } => {
            on_plugin_reload_finished(ctx.state, ctx.events, ctx.internal_tx, summary)
        }
    }
}
