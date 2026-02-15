use std::path::PathBuf;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;
use crate::runtime::model::RuntimeSyncReport;

pub(crate) struct ReloadDirDetailedFromStateMessage {
    pub dir: PathBuf,
}

impl Message for ReloadDirDetailedFromStateMessage {
    type Response = anyhow::Result<RuntimeSyncReport>;
}

impl Handler<ReloadDirDetailedFromStateMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: ReloadDirDetailedFromStateMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> anyhow::Result<RuntimeSyncReport> {
        let report = self.reload_dir_detailed_from_state(&message.dir);
        if let Ok(success) = &report {
            self.emit_reload_notifications(&success.load_report);
        }
        self.refresh_introspection_cache_snapshot();
        report
    }
}
