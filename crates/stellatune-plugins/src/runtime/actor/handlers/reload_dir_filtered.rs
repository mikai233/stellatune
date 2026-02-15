use std::collections::HashSet;
use std::path::PathBuf;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::load::RuntimeLoadReport;
use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct ReloadDirFilteredMessage {
    pub dir: PathBuf,
    pub disabled_ids: HashSet<String>,
}

impl Message for ReloadDirFilteredMessage {
    type Response = anyhow::Result<RuntimeLoadReport>;
}

impl Handler<ReloadDirFilteredMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: ReloadDirFilteredMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> anyhow::Result<RuntimeLoadReport> {
        let report = self.reload_dir_filtered(&message.dir, &message.disabled_ids);
        if let Ok(success) = &report {
            self.emit_reload_notifications(success);
        }
        self.refresh_introspection_cache_snapshot();
        report
    }
}
