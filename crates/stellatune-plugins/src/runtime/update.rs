use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::instance_registry::InstanceId;

#[derive(Debug, Clone)]
pub struct InstanceUpdateRequest {
    pub instance_id: InstanceId,
    pub config_json: String,
    pub requested_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceUpdateDecision {
    HotApply,
    Recreate,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceUpdateResult {
    Applied {
        instance_id: InstanceId,
        generation: u64,
    },
    Recreated {
        old_instance_id: InstanceId,
        new_instance_id: InstanceId,
        generation: u64,
    },
    Rejected {
        instance_id: InstanceId,
        generation: u64,
        reason: String,
    },
}

#[derive(Debug, Default)]
pub struct InstanceUpdateCoordinator {
    next_generation: AtomicU64,
    in_flight: Mutex<Vec<InstanceUpdateRequest>>,
}

impl InstanceUpdateCoordinator {
    pub fn next_generation(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn enqueue(&self, instance_id: InstanceId, config_json: String) -> InstanceUpdateRequest {
        let req = InstanceUpdateRequest {
            instance_id,
            config_json,
            requested_generation: self.next_generation(),
        };
        if let Ok(mut q) = self.in_flight.lock() {
            q.push(req.clone());
        }
        req
    }

    pub fn complete(&self, instance_id: InstanceId) {
        if let Ok(mut q) = self.in_flight.lock() {
            q.retain(|r| r.instance_id != instance_id);
        }
    }

    pub fn pending_for_instance(&self, instance_id: InstanceId) -> Option<InstanceUpdateRequest> {
        let q = self.in_flight.lock().ok()?;
        q.iter().find(|r| r.instance_id == instance_id).cloned()
    }
}

pub trait InstanceUpdateActor: Send + Sync + 'static {
    fn plan(&self, request: &InstanceUpdateRequest) -> InstanceUpdateDecision;
    fn apply_hot(&self, request: &InstanceUpdateRequest) -> anyhow::Result<InstanceUpdateResult>;
    fn apply_recreate(
        &self,
        request: &InstanceUpdateRequest,
    ) -> anyhow::Result<InstanceUpdateResult>;
}

#[derive(Clone)]
pub struct SharedUpdateActor {
    inner: Arc<dyn InstanceUpdateActor>,
}

impl SharedUpdateActor {
    pub fn new(inner: Arc<dyn InstanceUpdateActor>) -> Self {
        Self { inner }
    }

    pub fn dispatch(
        &self,
        request: &InstanceUpdateRequest,
    ) -> anyhow::Result<InstanceUpdateResult> {
        match self.inner.plan(request) {
            InstanceUpdateDecision::HotApply => self.inner.apply_hot(request),
            InstanceUpdateDecision::Recreate => self.inner.apply_recreate(request),
            InstanceUpdateDecision::Reject => Ok(InstanceUpdateResult::Rejected {
                instance_id: request.instance_id,
                generation: request.requested_generation,
                reason: "update rejected by actor".to_string(),
            }),
        }
    }
}
