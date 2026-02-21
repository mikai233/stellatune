use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};

use parking_lot::RwLock;

use crate::runtime::model::RuntimePluginDirective;

#[derive(Default)]
pub(crate) struct PluginDirectiveHub {
    subscriptions: RwLock<BTreeMap<String, Vec<Sender<RuntimePluginDirective>>>>,
}

impl PluginDirectiveHub {
    pub(crate) fn subscribe_plugin(
        &self,
        plugin_id: &str,
    ) -> Option<Receiver<RuntimePluginDirective>> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return None;
        }
        let (tx, rx) = mpsc::channel::<RuntimePluginDirective>();
        let mut state = self.subscriptions.write();
        state.entry(plugin_id.to_string()).or_default().push(tx);
        Some(rx)
    }

    pub(crate) fn notify_plugin(&self, plugin_id: &str, directive: RuntimePluginDirective) {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return;
        }
        let mut subscriptions = self.subscriptions.write();
        let Some(list) = subscriptions.get_mut(plugin_id) else {
            return;
        };
        list.retain(|sender| sender.send(directive.clone()).is_ok());
        if list.is_empty() {
            subscriptions.remove(plugin_id);
        }
    }
}
