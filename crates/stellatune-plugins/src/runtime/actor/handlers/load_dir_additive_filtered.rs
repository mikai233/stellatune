use std::collections::HashSet;
use std::path::PathBuf;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::load::RuntimeLoadReport;
use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct LoadDirAdditiveFilteredMessage {
    pub dir: PathBuf,
    pub disabled_ids: HashSet<String>,
}

impl Message for LoadDirAdditiveFilteredMessage {
    type Response = anyhow::Result<RuntimeLoadReport>;
}

impl Handler<LoadDirAdditiveFilteredMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: LoadDirAdditiveFilteredMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> anyhow::Result<RuntimeLoadReport> {
        let result = self.load_dir_additive_filtered(&message.dir, &message.disabled_ids);
        self.refresh_introspection_cache_snapshot();
        result
    }
}
