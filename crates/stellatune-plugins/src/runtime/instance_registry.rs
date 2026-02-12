use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(pub u64);

#[derive(Debug, Default)]
pub struct InstanceRegistry {
    next_id: AtomicU64,
}

impl InstanceRegistry {
    pub fn register(&self) -> InstanceId {
        InstanceId(
            self.next_id
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1),
        )
    }
}
