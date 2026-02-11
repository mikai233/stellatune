use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use serde_json::json;
use tokio::sync::Mutex;

use super::ApplyStateReport;

#[derive(Debug, Clone)]
pub struct CoalescedApplyResult {
    pub report: ApplyStateReport,
    pub coalesced_requests: u64,
    pub execution_loops: u64,
}

#[derive(Debug, Clone)]
struct ApplyStateStatusSnapshot {
    phase: &'static str,
    request_id: u64,
    latest_requested_request_id: u64,
    last_completed_request_id: u64,
    last_started_at_ms: u64,
    last_finished_at_ms: u64,
    last_report: ApplyStateReport,
}

impl Default for ApplyStateStatusSnapshot {
    fn default() -> Self {
        Self {
            phase: "idle",
            request_id: 0,
            latest_requested_request_id: 0,
            last_completed_request_id: 0,
            last_started_at_ms: 0,
            last_finished_at_ms: 0,
            last_report: ApplyStateReport::empty_completed(),
        }
    }
}

struct ApplyStateCoordinator {
    next_request_id: AtomicU64,
    exec_lock: Mutex<()>,
    snapshot: std::sync::Mutex<ApplyStateStatusSnapshot>,
}

impl ApplyStateCoordinator {
    fn new() -> Self {
        Self {
            next_request_id: AtomicU64::new(1),
            exec_lock: Mutex::new(()),
            snapshot: std::sync::Mutex::new(ApplyStateStatusSnapshot::default()),
        }
    }

    fn request(&self) -> u64 {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.latest_requested_request_id =
                snapshot.latest_requested_request_id.max(request_id);
        }
        request_id
    }

    fn mark_applying(&self, request_id: u64) {
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.phase = "applying";
            snapshot.request_id = request_id;
            snapshot.last_started_at_ms = now_unix_ms();
            snapshot.last_finished_at_ms = 0;
        }
    }

    fn mark_finished(&self, request_id: u64, result: &Result<ApplyStateReport>) {
        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.request_id = request_id;
            snapshot.last_completed_request_id = request_id;
            snapshot.last_finished_at_ms = now_unix_ms();
            match result {
                Ok(report) => {
                    snapshot.phase = if report.errors.is_empty() {
                        "applied"
                    } else {
                        "failed"
                    };
                    snapshot.last_report = report.clone();
                }
                Err(err) => {
                    snapshot.phase = "failed";
                    let mut report = ApplyStateReport::empty_completed();
                    report.phase = "failed";
                    report.errors = vec![err.to_string()];
                    snapshot.last_report = report;
                }
            }
        }
    }

    fn latest_requested(&self) -> u64 {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.latest_requested_request_id)
            .unwrap_or(0)
    }

    fn last_completed(&self) -> u64 {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.last_completed_request_id)
            .unwrap_or(0)
    }

    fn last_report(&self) -> ApplyStateReport {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.last_report.clone())
            .unwrap_or_else(|_| ApplyStateReport::empty_completed())
    }

    fn status_json(&self) -> String {
        let snapshot = self.snapshot.lock().map(|s| s.clone()).unwrap_or_default();
        json!({
            "phase": snapshot.phase,
            "request_id": snapshot.request_id,
            "latest_requested_request_id": snapshot.latest_requested_request_id,
            "last_completed_request_id": snapshot.last_completed_request_id,
            "last_started_at_ms": snapshot.last_started_at_ms,
            "last_finished_at_ms": snapshot.last_finished_at_ms,
            "last_loaded": snapshot.last_report.loaded,
            "last_deactivated": snapshot.last_report.deactivated,
            "last_unloaded_generations": snapshot.last_report.unloaded_generations,
            "last_error_count": snapshot.last_report.errors.len(),
            "last_errors": snapshot.last_report.errors,
            "last_plan_discovered": snapshot.last_report.plan_discovered,
            "last_plan_disabled": snapshot.last_report.plan_disabled,
            "last_plan_actions_total": snapshot.last_report.plan_actions_total,
            "last_plan_load_new": snapshot.last_report.plan_load_new,
            "last_plan_reload_changed": snapshot.last_report.plan_reload_changed,
            "last_plan_deactivate": snapshot.last_report.plan_deactivate,
            "last_plan_ms": snapshot.last_report.plan_ms,
            "last_execute_ms": snapshot.last_report.execute_ms,
            "last_total_ms": snapshot.last_report.total_ms,
            "last_coalesced_requests": snapshot.last_report.coalesced_requests,
            "last_execution_loops": snapshot.last_report.execution_loops,
            "last_action_outcomes": snapshot.last_report.action_outcomes,
        })
        .to_string()
    }
}

fn coordinator() -> &'static ApplyStateCoordinator {
    static COORDINATOR: OnceLock<ApplyStateCoordinator> = OnceLock::new();
    COORDINATOR.get_or_init(ApplyStateCoordinator::new)
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn status_json() -> String {
    coordinator().status_json()
}

pub async fn run_coalesced<F, Fut>(mut run_once: F) -> Result<CoalescedApplyResult>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<ApplyStateReport>>,
{
    coordinator().request();
    let _guard = coordinator().exec_lock.lock().await;
    let mut coalesced_requests = 0_u64;
    let mut execution_loops = 0_u64;

    loop {
        let target_request_id = coordinator().latest_requested();
        if target_request_id != 0 && coordinator().last_completed() >= target_request_id {
            let mut report = coordinator().last_report();
            report.coalesced_requests = coalesced_requests;
            report.execution_loops = execution_loops;
            return Ok(CoalescedApplyResult {
                report,
                coalesced_requests,
                execution_loops,
            });
        }

        coordinator().mark_applying(target_request_id);
        execution_loops = execution_loops.saturating_add(1);
        let result = run_once().await;
        coordinator().mark_finished(target_request_id, &result);
        let latest = coordinator().latest_requested();
        if latest <= target_request_id {
            let mut report = result?;
            report.coalesced_requests = coalesced_requests;
            report.execution_loops = execution_loops;
            return Ok(CoalescedApplyResult {
                report,
                coalesced_requests,
                execution_loops,
            });
        }
        coalesced_requests =
            coalesced_requests.saturating_add(latest.saturating_sub(target_request_id));
    }
}
