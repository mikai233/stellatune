use anyhow::{Result, anyhow};

use crate::runtime::instance_registry::InstanceId;
use crate::runtime::messages::WorkerControlMessage;
use crate::runtime::update::InstanceUpdateResult;
use crate::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome, WorkerConfigurableInstance,
    WorkerInstanceController, WorkerInstanceFactory,
};

#[derive(Debug, Clone)]
struct MockFactory;

#[derive(Debug, Clone)]
struct MockInstance {
    config: String,
}

impl WorkerConfigurableInstance for MockInstance {
    fn apply_config_update_json(&mut self, new_config_json: &str) -> Result<InstanceUpdateResult> {
        match new_config_json {
            "hot" => {
                self.config = new_config_json.to_string();
                Ok(InstanceUpdateResult::Applied {
                    instance_id: InstanceId(1),
                    revision: 1,
                })
            },
            "recreate" => Ok(InstanceUpdateResult::RequiresRecreate {
                instance_id: InstanceId(1),
                revision: 2,
                reason: Some("topology changed".to_string()),
            }),
            "reject" => Ok(InstanceUpdateResult::Rejected {
                instance_id: InstanceId(1),
                revision: 3,
                reason: "unsupported".to_string(),
            }),
            "failed" => Ok(InstanceUpdateResult::Failed {
                instance_id: InstanceId(1),
                revision: 4,
                error: "apply failed".to_string(),
            }),
            other => Err(anyhow!("unknown update config: {other}")),
        }
    }
}

impl WorkerInstanceFactory for MockFactory {
    type Instance = MockInstance;

    fn create_instance(&self, config_json: &str) -> Result<Self::Instance> {
        if config_json == "factory_err" {
            return Err(anyhow!("factory failed"));
        }
        Ok(MockInstance {
            config: config_json.to_string(),
        })
    }
}

#[test]
fn controller_creates_on_first_pending_apply() {
    let mut ctrl = WorkerInstanceController::new(MockFactory, "init");
    let outcome = ctrl.apply_pending().expect("pending create should work");
    assert_eq!(outcome, WorkerApplyPendingOutcome::Created);
    assert!(ctrl.instance().is_some());
    assert_eq!(ctrl.current_config_json(), Some("init"));
}

#[test]
fn controller_hot_apply_updates_current_config() {
    let mut ctrl = WorkerInstanceController::new(MockFactory, "init");
    let _ = ctrl.apply_pending().expect("must create");
    let outcome = ctrl
        .apply_config_update("hot")
        .expect("hot apply must work");
    assert_eq!(outcome, WorkerConfigUpdateOutcome::Applied { revision: 1 });
    assert_eq!(ctrl.current_config_json(), Some("hot"));
    assert!(!ctrl.has_pending_recreate());
}

#[test]
fn controller_requires_recreate_then_recreates() {
    let mut ctrl = WorkerInstanceController::new(MockFactory, "init");
    let _ = ctrl.apply_pending().expect("must create");
    let outcome = ctrl
        .apply_config_update("recreate")
        .expect("recreate plan should be valid");
    assert_eq!(
        outcome,
        WorkerConfigUpdateOutcome::RequiresRecreate {
            revision: 2,
            reason: Some("topology changed".to_string())
        }
    );
    assert!(ctrl.has_pending_recreate());
    let pending = ctrl.apply_pending().expect("recreate should succeed");
    assert_eq!(pending, WorkerApplyPendingOutcome::Recreated);
}

#[test]
fn controller_reject_keeps_running_instance() {
    let mut ctrl = WorkerInstanceController::new(MockFactory, "init");
    let _ = ctrl.apply_pending().expect("must create");
    let outcome = ctrl
        .apply_config_update("reject")
        .expect("reject path works");
    assert_eq!(
        outcome,
        WorkerConfigUpdateOutcome::Rejected {
            revision: 3,
            reason: "unsupported".to_string()
        }
    );
    assert!(!ctrl.has_pending_recreate());
    assert!(ctrl.instance().is_some());
}

#[test]
fn controller_destroy_message_deduplicates_by_seq() {
    let mut ctrl = WorkerInstanceController::new(MockFactory, "init");
    let _ = ctrl.apply_pending().expect("must create");

    ctrl.on_control_message(WorkerControlMessage::Destroy {
        reason: "disable".to_string(),
        seq: 2,
    });
    ctrl.on_control_message(WorkerControlMessage::Recreate {
        reason: "stale".to_string(),
        seq: 1,
    });

    assert!(ctrl.has_pending_destroy());
    let outcome = ctrl.apply_pending().expect("destroy should work");
    assert_eq!(outcome, WorkerApplyPendingOutcome::Destroyed);
    assert!(ctrl.instance().is_none());
}
