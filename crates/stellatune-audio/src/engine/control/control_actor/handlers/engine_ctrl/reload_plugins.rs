use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::{
    Event, apply_dsp_chain, clear_runtime_query_instance_cache,
    sync_output_sink_with_active_session,
};
use crate::engine::messages::PluginReloadSummary;

pub(crate) struct ReloadPluginsMessage {
    pub(crate) dir: String,
}

impl Message for ReloadPluginsMessage {
    type Response = ();
}

impl Handler<ReloadPluginsMessage> for ControlActor {
    fn handle(&mut self, message: ReloadPluginsMessage, _ctx: &mut ActorContext<Self>) {
        self.events.emit(Event::Log {
            message: format!(
                "plugin reload requested: dir={} source=runtime_state",
                message.dir
            ),
        });

        if let Some(worker) = self.state.decode_worker.as_ref() {
            worker.clear_promoted_preload();
        }
        self.state.preload_token = self.state.preload_token.wrapping_add(1);
        self.state.requested_preload_path = None;
        self.state.requested_preload_position_ms = 0;
        clear_runtime_query_instance_cache(&mut self.state);

        let service = stellatune_plugins::runtime::handle::shared_runtime_service();
        let prev_count = stellatune_runtime::block_on(service.active_plugin_ids()).len();
        let summary =
            match stellatune_runtime::block_on(service.reload_dir_from_state(&message.dir)) {
                Ok(report) => PluginReloadSummary {
                    dir: message.dir,
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
                Err(error) => PluginReloadSummary {
                    dir: message.dir,
                    prev_count,
                    loaded_ids: Vec::new(),
                    loaded_count: 0,
                    deactivated_count: 0,
                    unloaded_generations: 0,
                    load_errors: Vec::new(),
                    fatal_error: Some(error.to_string()),
                },
            };

        if let Some(err) = summary.fatal_error {
            self.events.emit(Event::Error {
                message: format!("plugin runtime v2 reload failed: {err}"),
            });
            return;
        }

        let loaded_ids = summary.loaded_ids.join(", ");
        self.events.emit(Event::Log {
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
            self.events.emit(Event::Log {
                message: format!("plugin load error: {err}"),
            });
        }

        if self.state.session.is_some()
            && let Err(message) =
                sync_output_sink_with_active_session(&mut self.state, &self.internal_tx)
        {
            self.events.emit(Event::Error { message });
        }
        if self.state.session.is_some()
            && let Err(message) = apply_dsp_chain(&mut self.state)
        {
            self.events.emit(Event::Error { message });
        }
    }
}
