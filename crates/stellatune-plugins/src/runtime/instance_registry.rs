use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::lifecycle::GenerationGuard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    Decoder,
    Dsp,
    SourceCatalog,
    LyricsProvider,
    OutputSink,
}

#[derive(Debug)]
pub struct InstanceRecord {
    pub id: InstanceId,
    pub plugin_id: String,
    pub capability_type_id: String,
    pub kind: CapabilityKind,
    pub generation: Arc<GenerationGuard>,
}

#[derive(Debug, Default)]
pub struct InstanceRegistry {
    next_id: AtomicU64,
    inner: Mutex<HashMap<InstanceId, InstanceRecord>>,
}

impl InstanceRegistry {
    pub fn register(
        &self,
        plugin_id: String,
        capability_type_id: String,
        kind: CapabilityKind,
        generation: Arc<GenerationGuard>,
    ) -> InstanceId {
        let id = InstanceId(
            self.next_id
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1),
        );
        generation.inc_instance();
        let record = InstanceRecord {
            id,
            plugin_id,
            capability_type_id,
            kind,
            generation,
        };
        if let Ok(mut map) = self.inner.lock() {
            map.insert(id, record);
        }
        id
    }

    pub fn get(&self, id: InstanceId) -> Option<InstanceRecordView> {
        let map = self.inner.lock().ok()?;
        let r = map.get(&id)?;
        Some(InstanceRecordView {
            id: r.id,
            plugin_id: r.plugin_id.clone(),
            capability_type_id: r.capability_type_id.clone(),
            kind: r.kind,
            generation: Arc::clone(&r.generation),
        })
    }

    pub fn remove(&self, id: InstanceId) -> Option<InstanceRecord> {
        let mut map = self.inner.lock().ok()?;
        let record = map.remove(&id)?;
        record.generation.dec_instance();
        Some(record)
    }

    pub fn list_ids_for_plugin(&self, plugin_id: &str) -> Vec<InstanceId> {
        let Ok(map) = self.inner.lock() else {
            return Vec::new();
        };
        map.values()
            .filter(|r| r.plugin_id == plugin_id)
            .map(|r| r.id)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct InstanceRecordView {
    pub id: InstanceId,
    pub plugin_id: String,
    pub capability_type_id: String,
    pub kind: CapabilityKind,
    pub generation: Arc<GenerationGuard>,
}
