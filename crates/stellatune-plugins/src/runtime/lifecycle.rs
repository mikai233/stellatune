use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenerationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GenerationState {
    Active = 1,
    Draining = 2,
    Unloaded = 3,
}

impl GenerationState {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Active,
            2 => Self::Draining,
            3 => Self::Unloaded,
            _ => Self::Draining,
        }
    }
}

#[derive(Debug)]
pub struct GenerationGuard {
    id: GenerationId,
    state: AtomicU8,
    live_instances: AtomicUsize,
    inflight_calls: AtomicUsize,
}

impl GenerationGuard {
    pub fn new_active(id: GenerationId) -> Arc<Self> {
        Arc::new(Self {
            id,
            state: AtomicU8::new(GenerationState::Active as u8),
            live_instances: AtomicUsize::new(0),
            inflight_calls: AtomicUsize::new(0),
        })
    }

    pub fn id(&self) -> GenerationId {
        self.id
    }

    pub fn state(&self) -> GenerationState {
        GenerationState::from_u8(self.state.load(Ordering::Acquire))
    }

    pub fn mark_draining(&self) {
        self.state
            .store(GenerationState::Draining as u8, Ordering::Release);
    }

    pub fn mark_unloaded(&self) {
        self.state
            .store(GenerationState::Unloaded as u8, Ordering::Release);
    }

    pub fn inc_instance(&self) {
        self.live_instances.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_instance(&self) {
        let _ = self
            .live_instances
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_sub(1))
            });
    }

    pub fn inc_inflight_call(&self) {
        self.inflight_calls.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_inflight_call(&self) {
        let _ = self
            .inflight_calls
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_sub(1))
            });
    }

    pub fn live_instances(&self) -> usize {
        self.live_instances.load(Ordering::Acquire)
    }

    pub fn inflight_calls(&self) -> usize {
        self.inflight_calls.load(Ordering::Acquire)
    }

    pub fn can_unload_now(&self) -> bool {
        self.live_instances() == 0 && self.inflight_calls() == 0
    }
}

#[derive(Debug, Default)]
pub struct PluginSlotLifecycle {
    active: Option<Arc<GenerationGuard>>,
    draining: Vec<Arc<GenerationGuard>>,
}

impl PluginSlotLifecycle {
    pub fn active(&self) -> Option<Arc<GenerationGuard>> {
        self.active.as_ref().map(Arc::clone)
    }

    pub fn activate_new_generation(&mut self, next: Arc<GenerationGuard>) {
        if let Some(cur) = self.active.take() {
            cur.mark_draining();
            self.draining.push(cur);
        }
        self.active = Some(next);
    }

    pub fn deactivate_active(&mut self) -> Option<Arc<GenerationGuard>> {
        let cur = self.active.take()?;
        cur.mark_draining();
        self.draining.push(Arc::clone(&cur));
        Some(cur)
    }

    pub fn draining(&self) -> &[Arc<GenerationGuard>] {
        &self.draining
    }

    /// Remove and return generations ready for unload.
    pub fn collect_ready_for_unload(&mut self) -> Vec<Arc<GenerationGuard>> {
        let mut ready = Vec::new();
        let mut i = 0usize;
        while i < self.draining.len() {
            if self.draining[i].can_unload_now() {
                let g = self.draining.swap_remove(i);
                g.mark_unloaded();
                ready.push(g);
            } else {
                i += 1;
            }
        }
        ready
    }
}

#[derive(Debug, Default)]
pub struct LifecycleStore {
    inner: Mutex<std::collections::HashMap<String, PluginSlotLifecycle>>,
}

impl LifecycleStore {
    pub fn activate_generation(&self, plugin_id: &str, generation: Arc<GenerationGuard>) {
        if let Ok(mut map) = self.inner.lock() {
            map.entry(plugin_id.to_string())
                .or_default()
                .activate_new_generation(generation);
        }
    }

    pub fn active_generation(&self, plugin_id: &str) -> Option<Arc<GenerationGuard>> {
        let map = self.inner.lock().ok()?;
        map.get(plugin_id).and_then(PluginSlotLifecycle::active)
    }

    pub fn deactivate_plugin(&self, plugin_id: &str) -> Option<Arc<GenerationGuard>> {
        let Ok(mut map) = self.inner.lock() else {
            return None;
        };
        map.get_mut(plugin_id)
            .and_then(PluginSlotLifecycle::deactivate_active)
    }

    pub fn collect_ready_for_unload(&self, plugin_id: &str) -> Vec<Arc<GenerationGuard>> {
        let Ok(mut map) = self.inner.lock() else {
            return Vec::new();
        };
        let Some(slot) = map.get_mut(plugin_id) else {
            return Vec::new();
        };
        slot.collect_ready_for_unload()
    }
}
