use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::load::RuntimeLoadReport;
use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct ShutdownAndCleanupMessage;

impl Message for ShutdownAndCleanupMessage {
    type Response = RuntimeLoadReport;
}

impl Handler<ShutdownAndCleanupMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        _message: ShutdownAndCleanupMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> RuntimeLoadReport {
        let report = self.shutdown_and_cleanup();
        let plugin_ids = self
            .worker_control_subscribers
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for plugin_id in plugin_ids {
            self.emit_worker_destroy(&plugin_id, "plugin runtime shutdown");
        }
        self.refresh_introspection_cache_snapshot();
        report
    }
}
