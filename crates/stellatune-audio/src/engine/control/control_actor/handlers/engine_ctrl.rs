use std::sync::Arc;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tokio::sync::oneshot::Sender as OneshotSender;

use super::super::super::{
    DecodeCtrl, EngineState, Event, EventHub, InternalMsg, apply_dsp_chain,
    clear_runtime_query_instance_cache, lyrics_fetch_json_via_runtime_async,
    lyrics_search_json_via_runtime_async, output_sink_list_targets_json_via_runtime,
    parse_dsp_chain, source_list_items_json_via_runtime_async,
    sync_output_sink_with_active_session,
};
use super::super::ControlActor;
use crate::engine::messages::EngineCtrl;
use crate::engine::messages::PluginReloadSummary;

pub(crate) struct ControlEngineCtrlMessage {
    pub(crate) message: EngineCtrl,
}

impl Message for ControlEngineCtrlMessage {
    type Response = ();
}

impl Handler<ControlEngineCtrlMessage> for ControlActor {
    fn handle(&mut self, message: ControlEngineCtrlMessage, _ctx: &mut ActorContext<Self>) {
        match message.message {
            EngineCtrl::SetDspChain { chain } => {
                on_engine_ctrl_set_dsp_chain(&mut self.state, &self.events, chain);
            }
            EngineCtrl::SourceListItemsJson {
                plugin_id,
                type_id,
                config_json,
                request_json,
                resp_tx,
            } => on_engine_ctrl_source_list_items_json(
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
            } => on_engine_ctrl_lyrics_search_json(plugin_id, type_id, query_json, resp_tx),
            EngineCtrl::LyricsFetchJson {
                plugin_id,
                type_id,
                track_json,
                resp_tx,
            } => on_engine_ctrl_lyrics_fetch_json(plugin_id, type_id, track_json, resp_tx),
            EngineCtrl::OutputSinkListTargetsJson {
                plugin_id,
                type_id,
                config_json,
                resp_tx,
            } => on_engine_ctrl_output_sink_list_targets_json(
                &mut self.state,
                plugin_id,
                type_id,
                config_json,
                resp_tx,
            ),
            EngineCtrl::SchedulePluginDisable { plugin_id, resp_tx } => {
                on_engine_ctrl_schedule_plugin_disable(&mut self.state, plugin_id, resp_tx);
            }
            EngineCtrl::ReloadPlugins { dir } => {
                on_engine_ctrl_reload_plugins(
                    &mut self.state,
                    &self.events,
                    &self.internal_tx,
                    dir,
                );
            }
            EngineCtrl::SetLfeMode { mode } => {
                on_engine_ctrl_set_lfe_mode(&mut self.state, mode);
            }
        }
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
    plugin_id: String,
    type_id: String,
    config_json: String,
    request_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    source_list_items_json_via_runtime_async(
        plugin_id,
        type_id,
        config_json,
        request_json,
        resp_tx,
    );
}

fn on_engine_ctrl_lyrics_search_json(
    plugin_id: String,
    type_id: String,
    query_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    lyrics_search_json_via_runtime_async(plugin_id, type_id, query_json, resp_tx);
}

fn on_engine_ctrl_lyrics_fetch_json(
    plugin_id: String,
    type_id: String,
    track_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    lyrics_fetch_json_via_runtime_async(plugin_id, type_id, track_json, resp_tx);
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
    let prev_count = stellatune_runtime::block_on(service.active_plugin_ids()).len();
    match stellatune_runtime::block_on(service.reload_dir_from_state(&dir)) {
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

fn on_engine_ctrl_set_lfe_mode(state: &mut EngineState, mode: stellatune_core::LfeMode) {
    state.lfe_mode = mode;
    if let Some(session) = state.session.as_ref() {
        let _ = session.ctrl_tx.send(DecodeCtrl::SetLfeMode { mode });
    }
}

fn on_engine_ctrl_schedule_plugin_disable(
    state: &mut EngineState,
    plugin_id: String,
    resp_tx: OneshotSender<Result<bool, String>>,
) {
    let result = schedule_plugin_disable(state, &plugin_id);
    let _ = resp_tx.send(result);
}

fn schedule_plugin_disable(state: &mut EngineState, plugin_id: &str) -> Result<bool, String> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return Err("plugin_id is empty".to_string());
    }

    state.pending_disable_plugins.insert(plugin_id.to_string());
    Ok(true)
}

fn on_plugin_reload_finished(
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
