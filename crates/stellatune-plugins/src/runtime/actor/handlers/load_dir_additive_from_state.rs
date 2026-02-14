use std::path::PathBuf;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::load::RuntimeLoadReport;
use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct LoadDirAdditiveFromStateMessage {
    pub dir: PathBuf,
}

impl Message for LoadDirAdditiveFromStateMessage {
    type Response = anyhow::Result<RuntimeLoadReport>;
}

impl Handler<LoadDirAdditiveFromStateMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: LoadDirAdditiveFromStateMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> anyhow::Result<RuntimeLoadReport> {
        let result = self.load_dir_additive_from_state(&message.dir);
        self.refresh_introspection_cache_snapshot();
        result
    }
}
