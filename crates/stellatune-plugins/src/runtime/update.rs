use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::instance_registry::InstanceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceUpdateDecision {
    HotApply,
    Recreate,
    Reject,
}

#[derive(Debug, Clone)]
pub struct InstanceUpdateRequest {
    pub instance_id: InstanceId,
    pub config_json: String,
    pub requested_generation: u64,
    pub decision: InstanceUpdateDecision,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceUpdateResult {
    Applied {
        instance_id: InstanceId,
        generation: u64,
    },
    RequiresRecreate {
        instance_id: InstanceId,
        generation: u64,
        reason: Option<String>,
    },
    Rejected {
        instance_id: InstanceId,
        generation: u64,
        reason: String,
    },
    Failed {
        instance_id: InstanceId,
        generation: u64,
        error: String,
    },
}

#[derive(Debug, Default)]
pub struct InstanceUpdateCoordinator {
    next_generation: AtomicU64,
}

impl InstanceUpdateCoordinator {
    pub fn next_generation(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn begin(
        &self,
        instance_id: InstanceId,
        config_json: String,
        decision: InstanceUpdateDecision,
        reason: Option<String>,
    ) -> InstanceUpdateRequest {
        InstanceUpdateRequest {
            instance_id,
            config_json,
            requested_generation: self.next_generation(),
            decision,
            reason,
        }
    }

    pub fn finish_applied(&self, req: &InstanceUpdateRequest) -> InstanceUpdateResult {
        InstanceUpdateResult::Applied {
            instance_id: req.instance_id,
            generation: req.requested_generation,
        }
    }

    pub fn finish_requires_recreate(
        &self,
        req: &InstanceUpdateRequest,
        reason: Option<String>,
    ) -> InstanceUpdateResult {
        InstanceUpdateResult::RequiresRecreate {
            instance_id: req.instance_id,
            generation: req.requested_generation,
            reason,
        }
    }

    pub fn finish_rejected(
        &self,
        req: &InstanceUpdateRequest,
        reason: String,
    ) -> InstanceUpdateResult {
        InstanceUpdateResult::Rejected {
            instance_id: req.instance_id,
            generation: req.requested_generation,
            reason,
        }
    }

    pub fn finish_failed(
        &self,
        req: &InstanceUpdateRequest,
        error: String,
    ) -> InstanceUpdateResult {
        InstanceUpdateResult::Failed {
            instance_id: req.instance_id,
            generation: req.requested_generation,
            error,
        }
    }

    pub fn complete(&self, _instance_id: InstanceId) {}
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
                reason: request
                    .reason
                    .clone()
                    .unwrap_or_else(|| "update rejected by actor".to_string()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InstanceId, InstanceUpdateCoordinator, InstanceUpdateDecision, InstanceUpdateResult,
    };

    #[test]
    fn coordinator_assigns_monotonic_generation() {
        let updates = InstanceUpdateCoordinator::default();
        let id = InstanceId(42);
        let req1 = updates.begin(
            id,
            "{\"gain\":1.0}".to_string(),
            InstanceUpdateDecision::HotApply,
            None,
        );
        let req2 = updates.begin(
            id,
            "{\"gain\":2.0}".to_string(),
            InstanceUpdateDecision::HotApply,
            None,
        );
        assert!(req2.requested_generation > req1.requested_generation);
        let result = updates.finish_applied(&req2);
        assert_eq!(
            result,
            InstanceUpdateResult::Applied {
                instance_id: id,
                generation: req2.requested_generation
            }
        );
    }

    #[test]
    fn coordinator_marks_recreate_and_rejected() {
        let updates = InstanceUpdateCoordinator::default();
        let id = InstanceId(7);
        let req = updates.begin(
            id,
            "{}".to_string(),
            InstanceUpdateDecision::Recreate,
            Some("resource topology changed".to_string()),
        );
        let recreate = updates.finish_requires_recreate(&req, req.reason.clone());
        assert_eq!(
            recreate,
            InstanceUpdateResult::RequiresRecreate {
                instance_id: id,
                generation: req.requested_generation,
                reason: Some("resource topology changed".to_string())
            }
        );

        let req2 = updates.begin(
            id,
            "{}".to_string(),
            InstanceUpdateDecision::Reject,
            Some("unsupported fields".to_string()),
        );
        let rejected = updates.finish_rejected(&req2, "unsupported fields".to_string());
        assert_eq!(
            rejected,
            InstanceUpdateResult::Rejected {
                instance_id: id,
                generation: req2.requested_generation,
                reason: "unsupported fields".to_string()
            }
        );
    }
}
