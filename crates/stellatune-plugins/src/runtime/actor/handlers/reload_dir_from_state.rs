use std::path::PathBuf;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::load::RuntimeLoadReport;
use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct ReloadDirFromStateMessage {
    pub dir: PathBuf,
}

impl Message for ReloadDirFromStateMessage {
    type Response = anyhow::Result<RuntimeLoadReport>;
}

impl Handler<ReloadDirFromStateMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: ReloadDirFromStateMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> anyhow::Result<RuntimeLoadReport> {
        let report = self.reload_dir_from_state(&message.dir);
        if let Ok(success) = &report {
            self.emit_reload_notifications(success);
        }
        self.refresh_introspection_cache_snapshot();
        report
    }
}
