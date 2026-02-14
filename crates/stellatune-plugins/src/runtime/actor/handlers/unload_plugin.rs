use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::load::RuntimeLoadReport;
use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct UnloadPluginMessage {
    pub plugin_id: String,
}

impl Message for UnloadPluginMessage {
    type Response = RuntimeLoadReport;
}

impl Handler<UnloadPluginMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: UnloadPluginMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> RuntimeLoadReport {
        let report = self.unload_plugin(&message.plugin_id);
        self.emit_worker_destroy(&message.plugin_id, "plugin unloaded");
        self.refresh_introspection_cache_snapshot();
        report
    }
}
