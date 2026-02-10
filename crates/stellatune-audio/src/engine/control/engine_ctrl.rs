use std::sync::Arc;

use crossbeam_channel::Sender;

use super::{
    DecodeCtrl, EngineCtrl, EngineState, Event, EventHub, InternalMsg, apply_dsp_chain,
    clear_runtime_query_instance_cache, lyrics_fetch_json_via_runtime,
    lyrics_search_json_via_runtime, output_sink_list_targets_json_via_runtime, parse_dsp_chain,
    source_list_items_json_via_runtime, sync_output_sink_with_active_session,
};

pub(super) fn handle_engine_ctrl(
    msg: EngineCtrl,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
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
        EngineCtrl::ReloadPlugins { dir } => {
            on_engine_ctrl_reload_plugins(state, events, internal_tx, dir, Vec::new());
        }
        EngineCtrl::ReloadPluginsWithDisabled { dir, disabled_ids } => {
            on_engine_ctrl_reload_plugins(state, events, internal_tx, dir, disabled_ids);
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
    resp_tx: Sender<Result<String, String>>,
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
    resp_tx: Sender<Result<String, String>>,
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
    resp_tx: Sender<Result<String, String>>,
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
    resp_tx: Sender<Result<String, String>>,
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
    internal_tx: &Sender<InternalMsg>,
    dir: String,
    disabled_ids: Vec<String>,
) {
    handle_reload_plugins(state, events, internal_tx, dir, disabled_ids);
}

fn on_engine_ctrl_set_lfe_mode(state: &mut EngineState, mode: stellatune_core::LfeMode) {
    state.lfe_mode = mode;
    if let Some(session) = state.session.as_ref() {
        let _ = session.ctrl_tx.send(DecodeCtrl::SetLfeMode { mode });
    }
}

fn handle_reload_plugins(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
    dir: String,
    disabled_ids: Vec<String>,
) {
    let disabled = disabled_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    events.emit(Event::Log {
        message: format!(
            "plugin reload requested: dir={} disabled_count={}",
            dir,
            disabled.len()
        ),
    });
    if let Some(worker) = state.decode_worker.as_ref() {
        worker.clear_promoted_preload();
    }
    state.preload_token = state.preload_token.wrapping_add(1);
    state.requested_preload_path = None;
    state.requested_preload_position_ms = 0;
    clear_runtime_query_instance_cache(state);
    let prev_count = match stellatune_plugins::shared_runtime_service().lock() {
        Ok(service) => service.active_plugin_ids().len(),
        Err(_) => {
            events.emit(Event::Error {
                message: "plugin runtime v2 mutex poisoned".to_string(),
            });
            return;
        }
    };
    let runtime_report = match stellatune_plugins::shared_runtime_service().lock() {
        Ok(service) => service
            .reload_dir_filtered(&dir, &disabled)
            .map_err(|e| e.to_string()),
        Err(_) => Err("plugin runtime v2 mutex poisoned".to_string()),
    };
    match runtime_report {
        Ok(v2) => {
            let loaded_ids = v2
                .loaded
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            events.emit(Event::Log {
                message: format!(
                    "plugins reloaded from {}: previous={} loaded={} deactivated={} errors={} unloaded_generations={} [{}]",
                    dir,
                    prev_count,
                    v2.loaded.len(),
                    v2.deactivated.len(),
                    v2.errors.len(),
                    v2.unloaded_generations,
                    loaded_ids
                ),
            });
            for err in v2.errors {
                events.emit(Event::Log {
                    message: format!("plugin load error: {err:#}"),
                });
            }

            if state.session.is_some()
                && let Err(message) = sync_output_sink_with_active_session(state, internal_tx)
            {
                events.emit(Event::Error { message });
            }
            if let Some(ctrl_tx) = state.session.as_ref().map(|s| s.ctrl_tx.clone()) {
                let _ = ctrl_tx.send(DecodeCtrl::RefreshDecoder);
                if let Err(message) = apply_dsp_chain(state) {
                    events.emit(Event::Error { message });
                }
            }
        }
        Err(err) => events.emit(Event::Error {
            message: format!("plugin runtime v2 reload failed: {err}"),
        }),
    }
}
