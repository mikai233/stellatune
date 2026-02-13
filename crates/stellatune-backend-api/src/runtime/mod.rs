use std::sync::{Arc, Mutex, OnceLock};
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use anyhow::{Result, anyhow};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::{Duration, Instant};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use tokio::time::sleep;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::LocalTime;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugins::runtime::handle::SharedPluginRuntimeService;

mod apply_state;
mod bus;
mod control;
mod engine;
mod router;
#[cfg(all(
    test,
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
mod tests;
mod types;
pub use engine::shared_runtime_engine;

#[derive(Clone)]
struct TeeWriter {
    file: Option<Arc<Mutex<std::fs::File>>>,
}

impl TeeWriter {
    fn new(file: Option<Arc<Mutex<std::fs::File>>>) -> Self {
        Self { file }
    }
}

impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = io::stderr().write_all(buf);
        if let Some(file) = &self.file
            && let Ok(mut guard) = file.lock()
        {
            let _ = guard.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = io::stderr().flush();
        if let Some(file) = &self.file
            && let Ok(mut guard) = file.lock()
        {
            let _ = guard.flush();
        }
        Ok(())
    }
}

fn tracing_log_file_path() -> PathBuf {
    std::env::temp_dir().join("stellatune").join("tracing.log")
}

fn open_tracing_log_file() -> Option<Arc<Mutex<std::fs::File>>> {
    let path = tracing_log_file_path();
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return None;
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .ok()?;
    Some(Arc::new(Mutex::new(file)))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub type SharedPluginRuntime = SharedPluginRuntimeService;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub type SharedPluginRuntime = ();

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn shared_plugin_runtime() -> SharedPluginRuntime {
    stellatune_plugins::runtime::handle::shared_runtime_service()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn shared_plugin_runtime() -> SharedPluginRuntime {}

fn install_panic_hook() {
    static PANIC_HOOK_INIT: OnceLock<()> = OnceLock::new();
    PANIC_HOOK_INIT.get_or_init(|| {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let location = panic_info
                .location()
                .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
                .unwrap_or_else(|| "<unknown>".to_string());
            let payload = if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
                (*message).to_string()
            } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
                message.clone()
            } else {
                "<non-string panic payload>".to_string()
            };
            let backtrace = std::backtrace::Backtrace::force_capture();
            tracing::error!(
                target: "stellatune::panic",
                %location,
                %payload,
                backtrace = %backtrace,
                "unhandled panic"
            );
            previous_hook(panic_info);
        }));
    });
}

pub fn init_tracing() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                EnvFilter::new("debug")
            } else {
                EnvFilter::new("info")
            }
        });
        let file = open_tracing_log_file();
        let writer = move || TeeWriter::new(file.clone());
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_timer(LocalTime::rfc_3339())
            .with_target(true)
            .with_thread_names(true)
            .with_thread_ids(true)
            .with_writer(writer)
            .try_init()
            .ok();
        install_panic_hook();
    });
}

pub fn register_plugin_runtime_engine(engine: stellatune_audio::EngineHandle) {
    router::register_plugin_runtime_engine(engine);
}

pub fn register_plugin_runtime_library(library: stellatune_library::LibraryHandle) {
    router::register_plugin_runtime_library(library);
}

pub fn subscribe_plugin_runtime_events_global()
-> tokio::sync::broadcast::Receiver<stellatune_core::PluginRuntimeEvent> {
    router::subscribe_plugin_runtime_events_global()
}

fn cleanup_plugin_runtime_for_restart(reason: &'static str) {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.active_plugin_ids();
    plugin_ids.sort();
    let report = service.shutdown_and_cleanup();
    let remaining_retired_leases: usize = plugin_ids
        .iter()
        .filter_map(|plugin_id| service.plugin_lease_state(plugin_id))
        .map(|state| state.retired_lease_ids.len())
        .sum();

    if remaining_retired_leases == 0 && report.errors.is_empty() {
        tracing::info!(
            reason,
            active_plugins_before_cleanup = plugin_ids.len(),
            deactivated = report.deactivated.len(),
            reclaimed_leases = report.reclaimed_leases,
            errors = report.errors.len(),
            "plugin runtime cleanup attempted"
        );
    } else {
        tracing::warn!(
            reason,
            active_plugins_before_cleanup = plugin_ids.len(),
            deactivated = report.deactivated.len(),
            reclaimed_leases = report.reclaimed_leases,
            remaining_retired_leases,
            errors = report.errors.len(),
            "plugin runtime cleanup attempted with leftovers"
        );
    }
}

pub async fn runtime_prepare_hot_restart() {
    engine::runtime_prepare_hot_restart().await;
    cleanup_plugin_runtime_for_restart("prepare_hot_restart");
}

pub async fn runtime_shutdown() {
    engine::runtime_shutdown().await;
    cleanup_plugin_runtime_for_restart("shutdown");
}

#[derive(Debug, Clone)]
pub struct DisableReport {
    pub plugin_id: String,
    pub phase: &'static str,
    pub deactivated_lease_id: Option<u64>,
    pub reclaimed_leases: usize,
    pub remaining_retired_leases: usize,
    pub timed_out: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EnableReport {
    pub plugin_id: String,
    pub phase: &'static str,
}

#[derive(Debug, Clone)]
pub struct ApplyStateReport {
    pub phase: &'static str,
    pub loaded: usize,
    pub deactivated: usize,
    pub reclaimed_leases: usize,
    pub errors: Vec<String>,
    pub plan_discovered: usize,
    pub plan_disabled: usize,
    pub plan_actions_total: usize,
    pub plan_load_new: usize,
    pub plan_reload_changed: usize,
    pub plan_deactivate: usize,
    pub plan_ms: u64,
    pub execute_ms: u64,
    pub total_ms: u64,
    pub action_outcomes: Vec<String>,
    pub coalesced_requests: u64,
    pub execution_loops: u64,
}

impl ApplyStateReport {
    pub(crate) fn empty_completed() -> Self {
        Self {
            phase: "completed",
            loaded: 0,
            deactivated: 0,
            reclaimed_leases: 0,
            errors: Vec::new(),
            plan_discovered: 0,
            plan_disabled: 0,
            plan_actions_total: 0,
            plan_load_new: 0,
            plan_reload_changed: 0,
            plan_deactivate: 0,
            plan_ms: 0,
            execute_ms: 0,
            total_ms: 0,
            action_outcomes: Vec::new(),
            coalesced_requests: 0,
            execution_loops: 0,
        }
    }
}

pub fn plugin_runtime_apply_state_status_json() -> String {
    apply_state::status_json()
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
const DEFAULT_PLUGIN_DISABLE_TIMEOUT_MS: u64 = 3_000;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
const PLUGIN_DISABLE_POLL_INTERVAL_MS: u64 = 20;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub async fn plugin_runtime_disable(
    library: &stellatune_library::LibraryHandle,
    plugin_id: String,
    timeout_ms: u64,
) -> Result<DisableReport> {
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }

    let timeout_ms = if timeout_ms == 0 {
        DEFAULT_PLUGIN_DISABLE_TIMEOUT_MS
    } else {
        timeout_ms
    };
    let started_at = Instant::now();
    let mut report = DisableReport {
        plugin_id: plugin_id.clone(),
        phase: "freeze",
        deactivated_lease_id: None,
        reclaimed_leases: 0,
        remaining_retired_leases: 0,
        timed_out: false,
        errors: Vec::new(),
    };

    tracing::info!(plugin_id, timeout_ms, "plugin_disable_begin");

    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    library.plugin_set_enabled(plugin_id.clone(), false).await?;

    report.phase = "quiesce";
    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    if let Err(err) = shared_runtime_engine()
        .quiesce_plugin_usage(plugin_id.clone())
        .await
    {
        report.errors.push(err);
    }

    report.phase = "deactivate";
    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    let service = shared_plugin_runtime();
    report.deactivated_lease_id = service
        .current_plugin_lease_info(&plugin_id)
        .map(|v| v.lease_id);
    let unload_report = service.unload_plugin(&plugin_id);
    report.reclaimed_leases = report
        .reclaimed_leases
        .saturating_add(unload_report.reclaimed_leases);
    report.errors.extend(
        unload_report
            .errors
            .into_iter()
            .map(|err| format!("{err:#}")),
    );

    report.phase = "collect";
    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        report.reclaimed_leases = report
            .reclaimed_leases
            .saturating_add(service.collect_retired_module_leases_by_refcount());
        report.remaining_retired_leases = service
            .plugin_lease_state(&plugin_id)
            .map(|state| state.retired_lease_ids.len())
            .unwrap_or(0);
        let done = report.remaining_retired_leases == 0;

        if done {
            break;
        }
        if Instant::now() >= deadline {
            report.timed_out = true;
            tracing::warn!(
                plugin_id,
                remaining_retired_leases = report.remaining_retired_leases,
                "plugin_disable_timeout"
            );
            break;
        }
        sleep(Duration::from_millis(PLUGIN_DISABLE_POLL_INTERVAL_MS)).await;
    }

    report.phase = "cleanup";
    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    service.cleanup_shadow_copies_now();

    report.phase = "completed";
    tracing::info!(
        plugin_id,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        deactivated_lease_id = report.deactivated_lease_id,
        reclaimed_leases = report.reclaimed_leases,
        remaining_retired_leases = report.remaining_retired_leases,
        timed_out = report.timed_out,
        errors = report.errors.len(),
        "plugin_disable_end"
    );
    Ok(report)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub async fn plugin_runtime_disable(
    _library: &stellatune_library::LibraryHandle,
    plugin_id: String,
    _timeout_ms: u64,
) -> anyhow::Result<DisableReport> {
    Ok(DisableReport {
        plugin_id,
        phase: "completed",
        deactivated_lease_id: None,
        reclaimed_leases: 0,
        remaining_retired_leases: 0,
        timed_out: false,
        errors: Vec::new(),
    })
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub async fn plugin_runtime_enable(
    library: &stellatune_library::LibraryHandle,
    plugin_id: String,
) -> Result<EnableReport> {
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }
    library.plugin_set_enabled(plugin_id.clone(), true).await?;
    Ok(EnableReport {
        plugin_id,
        phase: "completed",
    })
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub async fn plugin_runtime_enable(
    _library: &stellatune_library::LibraryHandle,
    plugin_id: String,
) -> anyhow::Result<EnableReport> {
    Ok(EnableReport {
        plugin_id,
        phase: "completed",
    })
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub async fn plugin_runtime_apply_state(
    library: &stellatune_library::LibraryHandle,
) -> Result<ApplyStateReport> {
    let result = apply_state::run_coalesced(|| async {
        let plugins_dir = library.plugins_dir_path().to_path_buf();
        let disabled_ids = library
            .list_disabled_plugin_ids()
            .await?
            .into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<std::collections::HashSet<_>>();

        let service = shared_plugin_runtime();
        service.set_disabled_plugin_ids_async(disabled_ids).await;
        let report = service
            .reload_dir_detailed_from_state_async(&plugins_dir)
            .await?;
        let errors = report
            .load_report
            .errors
            .into_iter()
            .map(|err| format!("{err:#}"))
            .collect();
        Ok(ApplyStateReport {
            phase: "completed",
            loaded: report.load_report.loaded.len(),
            deactivated: report.load_report.deactivated.len(),
            reclaimed_leases: report.load_report.reclaimed_leases,
            errors,
            plan_discovered: report.plan.discovered,
            plan_disabled: report.plan.disabled,
            plan_actions_total: report.plan.actions_total,
            plan_load_new: report.plan.load_new,
            plan_reload_changed: report.plan.reload_changed,
            plan_deactivate: report.plan.deactivate,
            plan_ms: report.plan_ms,
            execute_ms: report.execute_ms,
            total_ms: report.total_ms,
            action_outcomes: report
                .actions
                .into_iter()
                .map(|item| format!("{}:{}:{}", item.action, item.plugin_id, item.outcome))
                .collect(),
            coalesced_requests: 0,
            execution_loops: 0,
        })
    })
    .await?;
    let mut report = result.report;
    report.coalesced_requests = result.coalesced_requests;
    report.execution_loops = result.execution_loops;
    Ok(report)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub async fn plugin_runtime_apply_state(
    _library: &stellatune_library::LibraryHandle,
) -> anyhow::Result<ApplyStateReport> {
    let result =
        apply_state::run_coalesced(|| async { Ok(ApplyStateReport::empty_completed()) }).await?;
    let mut report = result.report;
    report.coalesced_requests = result.coalesced_requests;
    report.execution_loops = result.execution_loops;
    Ok(report)
}
