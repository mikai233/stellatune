use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::load::RuntimePluginInfo;
use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct ListActivePluginsMessage;

impl Message for ListActivePluginsMessage {
    type Response = Vec<RuntimePluginInfo>;
}

impl Handler<ListActivePluginsMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        _message: ListActivePluginsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Vec<RuntimePluginInfo> {
        let mut plugin_ids = self
            .modules
            .iter()
            .filter(|(_, slot)| slot.current.is_some())
            .map(|(plugin_id, _)| plugin_id.clone())
            .collect::<Vec<_>>();
        plugin_ids.sort();
        let mut out = Vec::with_capacity(plugin_ids.len());
        for plugin_id in plugin_ids {
            let Some(slot) = self.modules.get(&plugin_id) else {
                continue;
            };
            let Some(current_entry) = slot.current.as_ref() else {
                continue;
            };
            let current = &current_entry.lease;
            let mut info = RuntimePluginInfo {
                id: plugin_id.clone(),
                name: plugin_id.clone(),
                metadata_json: current.metadata_json.clone(),
                root_dir: None,
                library_path: None,
            };
            info.name = current.plugin_name.clone();
            info.root_dir = Some(current.loaded.root_dir.clone());
            info.library_path = Some(current.loaded.library_path.clone());
            out.push(info);
        }
        out
    }
}
