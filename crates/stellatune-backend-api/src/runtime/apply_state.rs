use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use serde_json::json;
use stellatune_runtime::tokio_actor::{ActorRef, Handler, Message};
use tokio::sync::Mutex;
use tracing::debug;

use super::ApplyStateReport;

mod handlers;

use self::handlers::get_last_completed::GetLastCompletedRequestIdMessage;
use self::handlers::get_last_report::GetLastReportMessage;
use self::handlers::get_latest_requested::GetLatestRequestedRequestIdMessage;
use self::handlers::mark_applying::MarkApplyingMessage;
use self::handlers::mark_finished::MarkFinishedMessage;
use self::handlers::request::RegisterRequestMessage;
use self::handlers::status_json::GetStatusJsonMessage;

const APPLY_STATE_ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct CoalescedApplyResult {
    pub report: ApplyStateReport,
    pub coalesced_requests: u64,
    pub execution_loops: u64,
}

#[derive(Debug, Clone)]
pub(super) struct ApplyStateStatusSnapshot {
    pub(super) phase: &'static str,
    pub(super) request_id: u64,
    pub(super) latest_requested_request_id: u64,
    pub(super) last_completed_request_id: u64,
    pub(super) last_started_at_ms: u64,
    pub(super) last_finished_at_ms: u64,
    pub(super) last_report: ApplyStateReport,
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

#[derive(Debug, Clone)]
pub(super) enum ApplyStateRunResult {
    Success(ApplyStateReport),
    Failure(String),
}

pub(super) struct ApplyStateCoordinatorActor {
    pub(super) next_request_id: u64,
    pub(super) snapshot: ApplyStateStatusSnapshot,
}

struct ApplyStateCoordinatorRuntime {
    actor_ref: ActorRef<ApplyStateCoordinatorActor>,
    exec_lock: Mutex<()>,
}

fn coordinator_runtime() -> &'static ApplyStateCoordinatorRuntime {
    static RUNTIME: OnceLock<ApplyStateCoordinatorRuntime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        let (actor_ref, _join) =
            stellatune_runtime::tokio_actor::spawn_actor(ApplyStateCoordinatorActor {
                next_request_id: 1,
                snapshot: ApplyStateStatusSnapshot::default(),
            });
        ApplyStateCoordinatorRuntime {
            actor_ref,
            exec_lock: Mutex::new(()),
        }
    })
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn coordinator_call<M>(message: M) -> Result<M::Response>
where
    M: Message,
    ApplyStateCoordinatorActor: Handler<M>,
{
    match coordinator_runtime()
        .actor_ref
        .call(message, APPLY_STATE_ACTOR_CALL_TIMEOUT)
        .await
    {
        Ok(response) => Ok(response),
        Err(err) => Err(anyhow!("apply_state coordinator unavailable: {err:?}")),
    }
}

pub(super) fn snapshot_status_json(snapshot: &ApplyStateStatusSnapshot) -> String {
    json!({
        "phase": snapshot.phase,
        "request_id": snapshot.request_id,
        "latest_requested_request_id": snapshot.latest_requested_request_id,
        "last_completed_request_id": snapshot.last_completed_request_id,
        "last_started_at_ms": snapshot.last_started_at_ms,
        "last_finished_at_ms": snapshot.last_finished_at_ms,
        "last_loaded": snapshot.last_report.loaded,
        "last_deactivated": snapshot.last_report.deactivated,
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

pub async fn status_json() -> String {
    match coordinator_call(GetStatusJsonMessage).await {
        Ok(status) => status,
        Err(err) => json!({
            "phase": "failed",
            "last_error_count": 1,
            "last_errors": [format!("apply_state status unavailable: {err:#}")],
        })
        .to_string(),
    }
}

pub async fn run_coalesced<F, Fut>(mut run_once: F) -> Result<CoalescedApplyResult>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<ApplyStateReport>>,
{
    let started = Instant::now();
    let _ = coordinator_call(RegisterRequestMessage).await?;
    let _guard = coordinator_runtime().exec_lock.lock().await;
    let mut coalesced_requests = 0_u64;
    let mut execution_loops = 0_u64;

    loop {
        let target_request_id = coordinator_call(GetLatestRequestedRequestIdMessage).await?;
        let last_completed = coordinator_call(GetLastCompletedRequestIdMessage).await?;

        if target_request_id != 0 && last_completed >= target_request_id {
            let mut report = coordinator_call(GetLastReportMessage).await?;
            report.coalesced_requests = coalesced_requests;
            report.execution_loops = execution_loops;
            let total_elapsed = started.elapsed();
            if total_elapsed.as_millis() > 200 {
                debug!(
                    elapsed_ms = total_elapsed.as_millis() as u64,
                    loops = execution_loops,
                    "apply state coalesced completed slowly (satisfied by other)"
                );
            }
            return Ok(CoalescedApplyResult {
                report,
                coalesced_requests,
                execution_loops,
            });
        }

        coordinator_call(MarkApplyingMessage {
            request_id: target_request_id,
        })
        .await?;
        execution_loops = execution_loops.saturating_add(1);
        let loop_started = Instant::now();
        let result = run_once().await;
        let loop_elapsed = loop_started.elapsed();
        if loop_elapsed.as_millis() > 100 {
            debug!(
                loop_index = execution_loops,
                elapsed_ms = loop_elapsed.as_millis() as u64,
                "apply state loop iteration was slow"
            );
        }

        let run_result = match &result {
            Ok(report) => ApplyStateRunResult::Success(report.clone()),
            Err(err) => ApplyStateRunResult::Failure(err.to_string()),
        };
        coordinator_call(MarkFinishedMessage {
            request_id: target_request_id,
            result: run_result,
        })
        .await?;

        let latest = coordinator_call(GetLatestRequestedRequestIdMessage).await?;
        if latest <= target_request_id {
            let mut report = result?;
            report.coalesced_requests = coalesced_requests;
            report.execution_loops = execution_loops;
            let total_elapsed = started.elapsed();
            if total_elapsed.as_millis() > 200 {
                debug!(
                    elapsed_ms = total_elapsed.as_millis() as u64,
                    loops = execution_loops,
                    "apply state coalesced completed slowly"
                );
            }
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

pub(super) fn now_ms_for_actor() -> u64 {
    now_unix_ms()
}
