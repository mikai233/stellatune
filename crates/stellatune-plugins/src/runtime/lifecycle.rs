use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

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
