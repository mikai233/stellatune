use std::sync::{Arc, Mutex, OnceLock};
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use anyhow::{Result, anyhow};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Instant;

use stellatune_audio::config::engine::ResampleQuality;
use stellatune_audio::engine::EngineHandle;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::LocalTime;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_wasm_plugins::host_runtime::runtime_service::SharedPluginRuntime;

mod apply_state;
mod engine;
mod hybrid_decoder_stage;
mod pipeline;

pub use hybrid_decoder_stage::{
    HybridDecoderStage, HybridProbedTrackDecodeInfo, SharedUserDecoderProvider,
    UserDecoderImplementation, UserDecoderProvider, decoder_supported_extensions_hybrid,
    decoder_supported_extensions_hybrid_with_user_decoders, default_user_decoder_providers,
    probe_track_decode_info_hybrid, probe_track_decode_info_hybrid_with_user_decoders,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputBackend {
    Shared,
    WasapiExclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDeviceDescriptor {
    pub backend: OutputBackend,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceSinkMetricsSnapshot {
    pub written_samples: u64,
    pub dropped_samples: u64,
    pub callback_requested_samples: u64,
    pub callback_provided_samples: u64,
    pub underrun_callbacks: u64,
    pub callback_errors: u64,
    pub reconfigure_attempts: u64,
    pub reconfigure_successes: u64,
    pub reconfigure_failures: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeOutputDeviceApplyReport {
    pub requested_backend: OutputBackend,
    pub applied_backend: OutputBackend,
    pub requested_device_id: Option<String>,
    pub applied_device_id: Option<String>,
    pub output_sample_rate: u32,
    pub output_channels: u16,
    pub fallback_to_default: bool,
}

pub fn shared_runtime_engine() -> Arc<EngineHandle> {
    engine::shared_runtime_engine()
}

pub fn runtime_list_output_devices() -> Result<Vec<OutputDeviceDescriptor>, String> {
    engine::runtime_list_output_devices()
}

pub fn runtime_output_sink_metrics() -> DeviceSinkMetricsSnapshot {
    engine::runtime_output_sink_metrics()
}

pub async fn runtime_set_output_device(
    backend: OutputBackend,
    device_id: Option<String>,
) -> Result<RuntimeOutputDeviceApplyReport, String> {
    engine::runtime_set_output_device(backend, device_id).await
}

pub async fn runtime_set_output_options(
    match_track_sample_rate: bool,
    resample_quality: ResampleQuality,
) -> Result<(), String> {
    engine::runtime_set_output_options(match_track_sample_rate, resample_quality).await
}

pub async fn runtime_set_output_sink_route(
    plugin_id: String,
    type_id: String,
    config_json: String,
    target_json: String,
) -> Result<(), String> {
    engine::runtime_set_output_sink_route(plugin_id, type_id, config_json, target_json).await
}

pub async fn runtime_clear_output_sink_route() -> Result<(), String> {
    engine::runtime_clear_output_sink_route().await
}

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

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub type SharedPluginRuntime = ();

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn shared_plugin_runtime() -> SharedPluginRuntime {
    stellatune_wasm_plugins::host_runtime::shared_runtime_service()
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
                // Keep application-level debug logging but silence noisy Wasmtime/Cranelift internals.
                EnvFilter::new(
                    "debug,cranelift_codegen=warn,cranelift_frontend=warn,wasmtime_cranelift=warn,wasmtime_internal_cranelift=warn,wasmtime_internal_jit_debug=warn,wasmtime_internal_cache=warn",
                )
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

async fn cleanup_plugin_runtime_for_shutdown() {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.active_plugin_ids();
    plugin_ids.sort();
    let report = service.shutdown_and_cleanup().await;
    if report.errors.is_empty() {
        tracing::info!(
            active_plugins_before_cleanup = plugin_ids.len(),
            deactivated = report.deactivated.len(),
            errors = report.errors.len(),
            "plugin runtime cleanup completed during shutdown"
        );
    } else {
        tracing::warn!(
            active_plugins_before_cleanup = plugin_ids.len(),
            deactivated = report.deactivated.len(),
            errors = report.errors.len(),
            "plugin runtime cleanup completed with leftovers during shutdown"
        );
    }
}

pub async fn runtime_shutdown() {
    engine::runtime_shutdown().await;
    cleanup_plugin_runtime_for_shutdown().await;
}

#[derive(Debug, Clone)]
pub struct DisableReport {
    pub plugin_id: String,
    pub phase: &'static str,
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

pub async fn plugin_runtime_apply_state_status_json() -> String {
    apply_state::status_json().await
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub async fn plugin_runtime_disable(
    library: &stellatune_library::LibraryHandle,
    plugin_id: String,
    _timeout_ms: u64,
) -> Result<DisableReport> {
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }

    let started_at = Instant::now();
    let mut report = DisableReport {
        plugin_id: plugin_id.clone(),
        phase: "freeze",
        errors: Vec::new(),
    };

    tracing::info!(plugin_id, "plugin_disable_begin");

    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    library.plugin_set_enabled(plugin_id.clone(), false).await?;

    // Clear active output route before unloading the plugin lease. This avoids
    // a teardown-order race where runtime destroy control for an in-use plugin
    // sink can overlap with sink-session reconfigure control and trip the sink
    // loop control timeout.
    if let Err(error) = engine::runtime_clear_output_sink_route_for_plugin(&plugin_id).await {
        report.errors.push(format!(
            "failed to clear output sink route for disabled plugin '{plugin_id}': {error}"
        ));
    }

    report.phase = "schedule";
    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    let service = shared_plugin_runtime();
    let unload_report = service.unload_plugin_report(&plugin_id).await;
    report
        .errors
        .extend(unload_report.errors.into_iter().map(|err| err.to_string()));

    report.phase = "apply_state";
    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    match plugin_runtime_apply_state(library).await {
        Ok(apply_report) => {
            report.errors.extend(apply_report.errors);
        },
        Err(error) => report.errors.push(format!(
            "failed to apply plugin runtime state after disable for '{plugin_id}': {error:#}"
        )),
    }

    let active_plugin_ids = shared_plugin_runtime().active_plugin_ids();
    if active_plugin_ids.iter().any(|id| id == &plugin_id) {
        report.errors.push(format!(
            "plugin '{plugin_id}' is still active after disable/apply-state"
        ));
    }

    report.phase = "completed";
    tracing::info!(
        plugin_id,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
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

    let apply_report = plugin_runtime_apply_state(library).await?;
    if !apply_report.errors.is_empty() {
        let details = apply_report.errors.join("; ");
        return Err(anyhow!(
            "plugin runtime apply-state completed with errors after enable for '{plugin_id}': {details}"
        ));
    }
    let active_plugin_ids = shared_plugin_runtime().active_plugin_ids();
    if !active_plugin_ids.iter().any(|id| id == &plugin_id) {
        return Err(anyhow!(
            "plugin '{plugin_id}' is still inactive after enable/apply-state"
        ));
    }

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
        service.set_disabled_plugin_ids(disabled_ids).await;
        let report = service.reload_dir_detailed_from_state(&plugins_dir).await?;
        let mut errors: Vec<String> = report
            .load_report
            .errors
            .into_iter()
            .map(|err| format!("{err:#}"))
            .collect();
        let active_plugin_ids = service.active_plugin_ids();
        if let Err(error) =
            engine::runtime_clear_output_sink_route_if_plugin_unavailable(&active_plugin_ids).await
        {
            errors.push(format!(
                "failed to reconcile output sink route after plugin apply state: {error}"
            ));
        }
        Ok(ApplyStateReport {
            phase: "completed",
            loaded: report.load_report.loaded.len(),
            deactivated: report.load_report.deactivated.len(),
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
