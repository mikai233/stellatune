use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;

use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use stellatune_plugin_api::StHostVTable;

use crate::events::{PluginEventBus, new_runtime_event_bus};
use crate::load::RuntimeLoadReport;
use crate::runtime::introspection::RuntimeIntrospectionReadCache;
use crate::runtime::messages::WorkerControlMessage;
use crate::runtime::model::ModuleLease;
use crate::runtime::registry::PluginModuleLeaseSlotState;

mod introspection;
mod shadow;
mod sync;

pub(crate) mod handlers;

fn lease_id_of(lease: &Arc<ModuleLease>) -> u64 {
    Arc::as_ptr(lease) as usize as u64
}

pub(crate) struct PluginRuntimeActor {
    host: StHostVTable,
    event_bus: PluginEventBus,
    modules: HashMap<String, PluginModuleLeaseSlotState>,
    disabled_plugin_ids: HashSet<String>,
    introspection_cache_local: ArcSwap<RuntimeIntrospectionReadCache>,
    introspection_cache_dirty: AtomicBool,
    pub(crate) worker_control_subscribers: HashMap<String, Vec<Sender<WorkerControlMessage>>>,
    worker_control_seq: HashMap<String, u64>,
    introspection_cache: Arc<ArcSwap<RuntimeIntrospectionReadCache>>,
}

impl PluginRuntimeActor {
    pub(crate) fn new(
        host: StHostVTable,
        introspection_cache: Arc<ArcSwap<RuntimeIntrospectionReadCache>>,
    ) -> Self {
        let actor = Self {
            host,
            event_bus: new_runtime_event_bus(),
            modules: HashMap::new(),
            disabled_plugin_ids: HashSet::new(),
            introspection_cache_local: ArcSwap::from_pointee(
                RuntimeIntrospectionReadCache::default(),
            ),
            introspection_cache_dirty: AtomicBool::new(true),
            worker_control_subscribers: HashMap::new(),
            worker_control_seq: HashMap::new(),
            introspection_cache,
        };
        actor.refresh_introspection_cache_snapshot();
        actor
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

    pub(crate) fn emit_reload_notifications(&mut self, report: &RuntimeLoadReport) {
        for plugin in &report.loaded {
            self.emit_worker_recreate(&plugin.id, "plugin lease swapped");
        }
        for plugin_id in &report.deactivated {
            self.emit_worker_destroy(plugin_id, "plugin deactivated");
        }
    }

    pub(crate) fn refresh_introspection_cache_snapshot(&self) {
        self.introspection_cache
            .store(self.introspection_cache_snapshot());
    }
}

pub(crate) fn spawn_plugin_runtime_actor(
    host: StHostVTable,
    introspection_cache: Arc<ArcSwap<RuntimeIntrospectionReadCache>>,
) -> std::io::Result<(
    stellatune_runtime::thread_actor::ActorRef<PluginRuntimeActor>,
    JoinHandle<()>,
)> {
    let actor = PluginRuntimeActor::new(host, introspection_cache);
    stellatune_runtime::thread_actor::spawn_actor_named(actor, "stellatune-plugin-runtime-actor")
}
