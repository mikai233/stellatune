use super::{InstanceId, InstanceUpdateCoordinator, InstanceUpdateDecision, InstanceUpdateResult};

#[test]
fn coordinator_assigns_monotonic_revision() {
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
    assert!(req2.requested_revision > req1.requested_revision);
    let result = updates.finish_applied(&req2);
    assert_eq!(
        result,
        InstanceUpdateResult::Applied {
            instance_id: id,
            revision: req2.requested_revision
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
            revision: req.requested_revision,
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
            revision: req2.requested_revision,
            reason: "unsupported fields".to_string()
        }
    );
}
