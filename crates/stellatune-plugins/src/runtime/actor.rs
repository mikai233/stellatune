use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;

use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use stellatune_plugin_api::StHostVTable;
use stellatune_runtime::thread_actor::{ActorRef, spawn_actor_named};

use crate::events::{PluginEventBus, new_runtime_event_bus};
use crate::load::RuntimeLoadReport;
use crate::runtime::introspection::RuntimeIntrospectionReadCache;
use crate::runtime::messages::WorkerControlMessage;
use crate::runtime::registry::PluginModuleLeaseSlotState;

mod introspection;
mod shadow;
mod sync;

pub(crate) mod handlers;

pub(crate) struct PluginRuntimeActor {
    host: StHostVTable,
    event_bus: PluginEventBus,
    modules: HashMap<String, PluginModuleLeaseSlotState>,
    disabled_plugin_ids: HashSet<String>,
    introspection_cache_local: ArcSwap<RuntimeIntrospectionReadCache>,
    introspection_cache_dirty: AtomicBool,
    pub(crate) worker_control_subscribers: HashMap<String, Vec<Sender<WorkerControlMessage>>>,
    worker_control_seq: HashMap<String, u64>,
    next_lease_id: u64,
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
            next_lease_id: 0,
            introspection_cache,
        };
        actor.refresh_introspection_cache_snapshot();
        actor
    }

    fn allocate_lease_id(&mut self) -> u64 {
        self.next_lease_id = self.next_lease_id.saturating_add(1);
        self.next_lease_id
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
        let (control_kind, control_seq, control_reason) = match &message {
            WorkerControlMessage::Recreate { reason, seq } => ("recreate", *seq, reason.as_str()),
            WorkerControlMessage::Destroy { reason, seq } => ("destroy", *seq, reason.as_str()),
        };
        let subscribers_before = self
            .worker_control_subscribers
            .get(plugin_id)
            .map(|subscribers| subscribers.len())
            .unwrap_or(0);
        let mut delivered = 0usize;
        let mut dropped = 0usize;
        let mut should_remove = false;
        if let Some(subscribers) = self.worker_control_subscribers.get_mut(plugin_id) {
            subscribers.retain(|tx| match tx.send(message.clone()) {
                Ok(()) => {
                    delivered = delivered.saturating_add(1);
                    true
                },
                Err(_) => {
                    dropped = dropped.saturating_add(1);
                    false
                },
            });
            should_remove = subscribers.is_empty();
        }
        if should_remove {
            self.worker_control_subscribers.remove(plugin_id);
        }
        let subscribers_after = self
            .worker_control_subscribers
            .get(plugin_id)
            .map(|subscribers| subscribers.len())
            .unwrap_or(0);
        tracing::debug!(
            plugin_id,
            control_kind,
            control_seq,
            control_reason,
            subscribers_before,
            subscribers_after,
            delivered,
            dropped,
            "worker control emitted"
        );
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
) -> std::io::Result<(ActorRef<PluginRuntimeActor>, JoinHandle<()>)> {
    let actor = PluginRuntimeActor::new(host, introspection_cache);
    spawn_actor_named(actor, "stellatune-plugin-runtime-actor")
}
