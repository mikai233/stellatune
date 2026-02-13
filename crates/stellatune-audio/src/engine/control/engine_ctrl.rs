use std::sync::Arc;

use crate::engine::messages::PluginReloadSummary;
use tokio::sync::oneshot::Sender as OneshotSender;

use super::{
    DecodeCtrl, EngineCtrl, EngineState, Event, EventHub, InternalMsg, PlayerState,
    SessionStopMode, SharedTrackInfo, apply_dsp_chain, clear_runtime_query_instance_cache,
    clear_runtime_query_instance_cache_for_plugin, drop_output_pipeline, emit_position_event,
    lyrics_fetch_json_via_runtime, lyrics_search_json_via_runtime, next_position_session_id,
    output_sink_list_targets_json_via_runtime, parse_dsp_chain, set_state,
    source_list_items_json_via_runtime, stop_decode_session, sync_output_sink_with_active_session,
};

pub(super) fn handle_engine_ctrl(
    msg: EngineCtrl,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &crossbeam_channel::Sender<InternalMsg>,
    track_info: &SharedTrackInfo,
) {
    match msg {
        EngineCtrl::SetDspChain { chain } => on_engine_ctrl_set_dsp_chain(state, events, chain),
        EngineCtrl::SourceListItemsJson {
            plugin_id,
            type_id,
            config_json,
            request_json,
            resp_tx,
        } => on_engine_ctrl_source_list_items_json(
            state,
            plugin_id,
            type_id,
            config_json,
            request_json,
            resp_tx,
        ),
        EngineCtrl::LyricsSearchJson {
            plugin_id,
            type_id,
            query_json,
            resp_tx,
        } => on_engine_ctrl_lyrics_search_json(state, plugin_id, type_id, query_json, resp_tx),
        EngineCtrl::LyricsFetchJson {
            plugin_id,
            type_id,
            track_json,
            resp_tx,
        } => on_engine_ctrl_lyrics_fetch_json(state, plugin_id, type_id, track_json, resp_tx),
        EngineCtrl::OutputSinkListTargetsJson {
            plugin_id,
            type_id,
            config_json,
            resp_tx,
        } => on_engine_ctrl_output_sink_list_targets_json(
            state,
            plugin_id,
            type_id,
            config_json,
            resp_tx,
        ),
        EngineCtrl::QuiescePluginUsage { plugin_id, resp_tx } => {
            on_engine_ctrl_quiesce_plugin_usage(
                state,
                events,
                internal_tx,
                track_info,
                plugin_id,
                resp_tx,
            );
        }
        EngineCtrl::ReloadPlugins { dir } => {
            on_engine_ctrl_reload_plugins(state, events, internal_tx, dir);
        }
        EngineCtrl::SetLfeMode { mode } => on_engine_ctrl_set_lfe_mode(state, mode),
    }
}

fn on_engine_ctrl_set_dsp_chain(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    chain: Vec<stellatune_core::DspChainItem>,
) {
    let parsed = match parse_dsp_chain(chain) {
        Ok(parsed) => parsed,
        Err(message) => {
            events.emit(Event::Error { message });
            return;
        }
    };
    state.desired_dsp_chain = parsed;
    if state.session.is_some()
        && let Err(message) = apply_dsp_chain(state)
    {
        events.emit(Event::Error { message });
    }
}

fn on_engine_ctrl_source_list_items_json(
    state: &mut EngineState,
    plugin_id: String,
    type_id: String,
    config_json: String,
    request_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    let _ = resp_tx.send(source_list_items_json_via_runtime(
        state,
        &plugin_id,
        &type_id,
        config_json,
        request_json,
    ));
}

fn on_engine_ctrl_lyrics_search_json(
    state: &mut EngineState,
    plugin_id: String,
    type_id: String,
    query_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    let _ = resp_tx.send(lyrics_search_json_via_runtime(
        state, &plugin_id, &type_id, query_json,
    ));
}

fn on_engine_ctrl_lyrics_fetch_json(
    state: &mut EngineState,
    plugin_id: String,
    type_id: String,
    track_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    let _ = resp_tx.send(lyrics_fetch_json_via_runtime(
        state, &plugin_id, &type_id, track_json,
    ));
}

fn on_engine_ctrl_output_sink_list_targets_json(
    state: &mut EngineState,
    plugin_id: String,
    type_id: String,
    config_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    let _ = resp_tx.send(output_sink_list_targets_json_via_runtime(
        state,
        &plugin_id,
        &type_id,
        config_json,
    ));
}

fn on_engine_ctrl_reload_plugins(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &crossbeam_channel::Sender<InternalMsg>,
    dir: String,
) {
    handle_reload_plugins(state, events, internal_tx, dir);
}

fn on_engine_ctrl_set_lfe_mode(state: &mut EngineState, mode: stellatune_core::LfeMode) {
    state.lfe_mode = mode;
    if let Some(session) = state.session.as_ref() {
        let _ = session.ctrl_tx.send(DecodeCtrl::SetLfeMode { mode });
    }
}

fn on_engine_ctrl_quiesce_plugin_usage(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &crossbeam_channel::Sender<InternalMsg>,
    track_info: &SharedTrackInfo,
    plugin_id: String,
    resp_tx: OneshotSender<Result<(), String>>,
) {
    let result = quiesce_plugin_usage(state, events, internal_tx, track_info, &plugin_id);
    let _ = resp_tx.send(result);
}

fn quiesce_plugin_usage(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &crossbeam_channel::Sender<InternalMsg>,
    track_info: &SharedTrackInfo,
    plugin_id: &str,
) -> Result<(), String> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return Err("plugin_id is empty".to_string());
    }

    let had_session = state.session.is_some();
    if had_session {
        stop_decode_session(state, track_info, SessionStopMode::TearDownSink);
        drop_output_pipeline(state);
        state.position_ms = 0;
        state.wants_playback = false;
        state.play_request_started_at = None;
        state.pending_session_start = false;
        state.seek_position_guard = None;
        next_position_session_id(state);
        emit_position_event(state, events);
        set_state(state, events, PlayerState::Stopped);
    }

    let mut cleared_output_route = false;
    if state
        .desired_output_sink_route
        .as_ref()
        .is_some_and(|route| route.plugin_id == plugin_id)
    {
        state.desired_output_sink_route = None;
        state.output_sink_chunk_frames = 0;
        state.output_sink_negotiation_cache = None;
        state.cached_output_spec = None;
        state.output_spec_prewarm_inflight = false;
        state.output_spec_token = state.output_spec_token.wrapping_add(1);
        if let Err(message) = sync_output_sink_with_active_session(state, internal_tx) {
            return Err(format!(
                "clear output sink route for plugin `{plugin_id}`: {message}"
            ));
        }
        cleared_output_route = true;
    }

    let (source_removed, lyrics_removed, output_sink_removed) =
        clear_runtime_query_instance_cache_for_plugin(state, plugin_id);
    events.emit(Event::Log {
        message: format!(
            "plugin usage quiesced: plugin_id={} had_session={} cleared_output_route={} source_instances_removed={} lyrics_instances_removed={} output_sink_instances_removed={}",
            plugin_id,
            had_session,
            cleared_output_route,
            source_removed,
            lyrics_removed,
            output_sink_removed
        ),
    });

    Ok(())
}

fn handle_reload_plugins(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &crossbeam_channel::Sender<InternalMsg>,
    dir: String,
) {
    events.emit(Event::Log {
        message: format!("plugin reload requested: dir={} source=runtime_state", dir),
    });
    if let Some(worker) = state.decode_worker.as_ref() {
        worker.clear_promoted_preload();
    }
    state.preload_token = state.preload_token.wrapping_add(1);
    state.requested_preload_path = None;
    state.requested_preload_position_ms = 0;
    clear_runtime_query_instance_cache(state);
    let service = stellatune_plugins::runtime::handle::shared_runtime_service();
    let prev_count = service.active_plugin_ids().len();
    match service.reload_dir_from_state(&dir) {
        Ok(report) => on_plugin_reload_finished(
            state,
            events,
            internal_tx,
            PluginReloadSummary {
                dir,
                prev_count,
                loaded_ids: report
                    .loaded
                    .iter()
                    .map(|plugin| plugin.id.clone())
                    .collect(),
                loaded_count: report.loaded.len(),
                deactivated_count: report.deactivated.len(),
                unloaded_generations: report.reclaimed_leases,
                load_errors: report.errors.iter().map(ToString::to_string).collect(),
                fatal_error: None,
            },
        ),
        Err(error) => on_plugin_reload_finished(
            state,
            events,
            internal_tx,
            PluginReloadSummary {
                dir,
                prev_count,
                loaded_ids: Vec::new(),
                loaded_count: 0,
                deactivated_count: 0,
                unloaded_generations: 0,
                load_errors: Vec::new(),
                fatal_error: Some(error.to_string()),
            },
        ),
    }
}

pub(super) fn on_plugin_reload_finished(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &crossbeam_channel::Sender<InternalMsg>,
    summary: PluginReloadSummary,
) {
    if let Some(err) = summary.fatal_error {
        events.emit(Event::Error {
            message: format!("plugin runtime v2 reload failed: {err}"),
        });
    } else {
        let loaded_ids = summary.loaded_ids.join(", ");
        events.emit(Event::Log {
            message: format!(
                "plugins reloaded from {}: previous={} loaded={} deactivated={} errors={} reclaimed_leases={} [{}]",
                summary.dir,
                summary.prev_count,
                summary.loaded_count,
                summary.deactivated_count,
                summary.load_errors.len(),
                summary.unloaded_generations,
                loaded_ids
            ),
        });
        for err in summary.load_errors {
            events.emit(Event::Log {
                message: format!("plugin load error: {err}"),
            });
        }

        if state.session.is_some()
            && let Err(message) = sync_output_sink_with_active_session(state, internal_tx)
        {
            events.emit(Event::Error { message });
        }
        if state.session.is_some()
            && let Err(message) = apply_dsp_chain(state)
        {
            events.emit(Event::Error { message });
        }
    }
}
