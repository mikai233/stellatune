use std::collections::HashMap;

use crossbeam_channel::Sender;
use stellatune_plugin_api::StHostVTable;
use tokio::sync::mpsc;

use crate::service::PluginRuntimeService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerControlMessage {
    Recreate { reason: String, seq: u64 },
    Destroy { reason: String, seq: u64 },
}

pub(crate) type RuntimeActorTask = Box<dyn FnOnce(&mut RuntimeActorState) + Send + 'static>;

pub(crate) struct RuntimeActorState {
    pub(crate) service: PluginRuntimeService,
    pub(crate) worker_control_subscribers: HashMap<String, Vec<Sender<WorkerControlMessage>>>,
    worker_control_seq: HashMap<String, u64>,
}

impl RuntimeActorState {
    pub(crate) fn new(host: StHostVTable) -> Self {
        Self {
            service: PluginRuntimeService::new(host),
            worker_control_subscribers: HashMap::new(),
            worker_control_seq: HashMap::new(),
        }
    }

    pub(crate) fn register_worker_control_sender(
        &mut self,
        plugin_id: String,
        sender: Sender<WorkerControlMessage>,
    ) {
        self.worker_control_subscribers
            .entry(plugin_id)
            .or_default()
            .push(sender);
    }

    fn next_worker_control_seq(&mut self, plugin_id: &str) -> u64 {
        let seq = self
            .worker_control_seq
            .entry(plugin_id.to_string())
            .or_insert(0);
        *seq = seq.saturating_add(1);
        *seq
    }

    fn emit_worker_control(&mut self, plugin_id: &str, message: WorkerControlMessage) {
        let mut should_remove = false;
        if let Some(subscribers) = self.worker_control_subscribers.get_mut(plugin_id) {
            subscribers.retain(|tx| tx.send(message.clone()).is_ok());
            should_remove = subscribers.is_empty();
        }
        if should_remove {
            self.worker_control_subscribers.remove(plugin_id);
        }
    }

    pub(crate) fn emit_worker_recreate(&mut self, plugin_id: &str, reason: &str) {
        let seq = self.next_worker_control_seq(plugin_id);
        self.emit_worker_control(
            plugin_id,
            WorkerControlMessage::Recreate {
                reason: reason.to_string(),
                seq,
            },
        );
    }

    pub(crate) fn emit_worker_destroy(&mut self, plugin_id: &str, reason: &str) {
        let seq = self.next_worker_control_seq(plugin_id);
        self.emit_worker_control(
            plugin_id,
            WorkerControlMessage::Destroy {
                reason: reason.to_string(),
                seq,
            },
        );
    }

    pub(crate) fn emit_worker_destroy_all(&mut self, reason: &str) {
        let plugin_ids = self
            .worker_control_subscribers
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for plugin_id in plugin_ids {
            self.emit_worker_destroy(&plugin_id, reason);
        }
    }
}

pub(crate) async fn run_plugin_runtime_actor(
    mut rx: mpsc::UnboundedReceiver<RuntimeActorTask>,
    host: StHostVTable,
) {
    let mut state = RuntimeActorState::new(host);
    while let Some(task) = rx.recv().await {
        task(&mut state);
    }
}
