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

#[derive(Debug, Default)]
pub struct InstanceRegistry {
    next_id: AtomicU64,
    inner: Mutex<HashMap<InstanceId, Arc<GenerationGuard>>>,
}

impl InstanceRegistry {
    pub fn register(&self, generation: Arc<GenerationGuard>) -> InstanceId {
        let id = InstanceId(
            self.next_id
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1),
        );
        generation.inc_instance();
        if let Ok(mut map) = self.inner.lock() {
            map.insert(id, generation);
        }
        id
    }

    pub fn generation_of(&self, id: InstanceId) -> Option<Arc<GenerationGuard>> {
        let map = self.inner.lock().ok()?;
        map.get(&id).map(Arc::clone)
    }

    pub fn remove(&self, id: InstanceId) -> bool {
        let Ok(mut map) = self.inner.lock() else {
            return false;
        };
        let Some(generation) = map.remove(&id) else {
            return false;
        };
        generation.dec_instance();
        true
    }
}
