use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::runtime::{CapabilityKind, GenerationId};

use super::{CapabilityDescriptorInput, CapabilityDescriptorRecord, CapabilityId};

#[derive(Default)]
pub struct CapabilityRegistry {
    next_id: AtomicU64,
    inner: RwLock<HashMap<CapabilityId, CapabilityDescriptorRecord>>,
}

impl CapabilityRegistry {
    pub fn register_generation(
        &self,
        plugin_id: &str,
        generation: GenerationId,
        inputs: Vec<CapabilityDescriptorInput>,
    ) -> Vec<CapabilityDescriptorRecord> {
        let mut out = Vec::with_capacity(inputs.len());
        let Ok(mut map) = self.inner.write() else {
            return out;
        };
        for input in inputs {
            let id = CapabilityId(self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
            let record = CapabilityDescriptorRecord {
                id,
                plugin_id: plugin_id.to_string(),
                generation,
                kind: input.kind,
                type_id: input.type_id,
                display_name: input.display_name,
                config_schema_json: input.config_schema_json,
                default_config_json: input.default_config_json,
            };
            map.insert(id, record.clone());
            out.push(record);
        }
        out
    }

    pub fn remove_generation(&self, plugin_id: &str, generation: GenerationId) {
        let Ok(mut map) = self.inner.write() else {
            return;
        };
        map.retain(|_, v| !(v.plugin_id == plugin_id && v.generation == generation));
    }

    pub fn get(&self, id: CapabilityId) -> Option<CapabilityDescriptorRecord> {
        let map = self.inner.read().ok()?;
        map.get(&id).cloned()
    }

    pub fn find(
        &self,
        plugin_id: &str,
        generation: GenerationId,
        kind: CapabilityKind,
        type_id: &str,
    ) -> Option<CapabilityDescriptorRecord> {
        let map = self.inner.read().ok()?;
        map.values()
            .find(|v| {
                v.plugin_id == plugin_id
                    && v.generation == generation
                    && v.kind == kind
                    && v.type_id == type_id
            })
            .cloned()
    }

    pub fn list_for_generation(
        &self,
        plugin_id: &str,
        generation: GenerationId,
    ) -> Vec<CapabilityDescriptorRecord> {
        let Ok(map) = self.inner.read() else {
            return Vec::new();
        };
        map.values()
            .filter(|v| v.plugin_id == plugin_id && v.generation == generation)
            .cloned()
            .collect()
    }
}
