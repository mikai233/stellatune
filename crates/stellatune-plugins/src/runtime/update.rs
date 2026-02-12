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
    pub requested_revision: u64,
    pub decision: InstanceUpdateDecision,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceUpdateResult {
    Applied {
        instance_id: InstanceId,
        revision: u64,
    },
    RequiresRecreate {
        instance_id: InstanceId,
        revision: u64,
        reason: Option<String>,
    },
    Rejected {
        instance_id: InstanceId,
        revision: u64,
        reason: String,
    },
    Failed {
        instance_id: InstanceId,
        revision: u64,
        error: String,
    },
}

#[derive(Debug, Default)]
pub struct InstanceUpdateCoordinator {
    next_revision: AtomicU64,
}

impl InstanceUpdateCoordinator {
    pub fn next_revision(&self) -> u64 {
        self.next_revision.fetch_add(1, Ordering::Relaxed) + 1
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
            requested_revision: self.next_revision(),
            decision,
            reason,
        }
    }

    pub fn finish_applied(&self, req: &InstanceUpdateRequest) -> InstanceUpdateResult {
        InstanceUpdateResult::Applied {
            instance_id: req.instance_id,
            revision: req.requested_revision,
        }
    }

    pub fn finish_requires_recreate(
        &self,
        req: &InstanceUpdateRequest,
        reason: Option<String>,
    ) -> InstanceUpdateResult {
        InstanceUpdateResult::RequiresRecreate {
            instance_id: req.instance_id,
            revision: req.requested_revision,
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
            revision: req.requested_revision,
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
            revision: req.requested_revision,
            error,
        }
    }
}

#[cfg(test)]
#[path = "tests/update_tests.rs"]
mod tests;
