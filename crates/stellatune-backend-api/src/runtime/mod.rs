use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
};

use anyhow::{Result, anyhow};
use std::time::Instant;

use crate::player_service::service::PlayerService;
use stellatune_audio::config::engine::ResampleQuality;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::LocalTime;

mod engine;
mod local_probe;
mod pipeline;
mod plugin_manager;
mod transcode_decoder;
mod transcode_encoder;
mod typescript_source;

pub use local_probe::{ProbedTrackDecodeInfo, decoder_supported_extensions, probe_local_track};
pub use pipeline::set_runtime_builtin_transform_options;
pub use plugin_manager::{PluginManagerHandle, PluginManagerOperationError};
pub use transcode_decoder::{
    MediaMetadata, PcmF32Chunk, TranscodeDecoderInfo, TranscodeDecoderSession,
    open_local_transcode_decoder,
};
pub use transcode_encoder::{
    TranscodeEncoderDescriptor, TranscodeEncoderSession, list_local_transcode_encoders,
    open_local_transcode_encoder,
};
pub use typescript_source::{TypeScriptSourceResolver, TypeScriptSourceResolverFactory};

/// Shared control-plane runtime. It owns no playback state and starts Node only
/// when a registered capability is first invoked.
pub fn shared_typescript_runtime() -> Arc<stellatune_plugins::typescript::TypeScriptRuntime> {
    static RUNTIME: OnceLock<Arc<stellatune_plugins::typescript::TypeScriptRuntime>> =
        OnceLock::new();
    Arc::clone(RUNTIME.get_or_init(|| {
        Arc::new(stellatune_plugins::typescript::TypeScriptRuntime::new(
            typescript_runner_path(),
        ))
    }))
}

pub fn shared_plugin_manager(plugins_dir: &Path) -> PluginManagerHandle {
    static MANAGERS: OnceLock<Mutex<BTreeMap<PathBuf, PluginManagerHandle>>> = OnceLock::new();
    let key = plugins_dir.to_path_buf();
    let mut managers = MANAGERS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    managers
        .entry(key.clone())
        .or_insert_with(|| {
            PluginManagerHandle::spawn(
                shared_playback_controller(),
                shared_typescript_runtime(),
                key,
            )
        })
        .clone()
}

fn typescript_runner_path() -> PathBuf {
    if let Some(path) = std::env::var_os("STELLATUNE_TYPESCRIPT_RUNNER") {
        return PathBuf::from(path);
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        for relative in [
            "typescript-plugin-runtime/runner.mjs",
            "tools/typescript-plugin-runtime/runner.mjs",
            "runner.mjs",
        ] {
            let candidate = directory.join(relative);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/typescript-plugin-runtime/runner.mjs")
}

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

pub fn shared_playback_controller() -> stellatune_audio::playback::control::PlaybackController {
    engine::shared_playback_controller()
}

pub fn install_player_service(service: Arc<PlayerService>) -> Result<(), Arc<PlayerService>> {
    player_service_slot().set(service)
}

pub fn shared_player_service() -> Option<Arc<PlayerService>> {
    // Keep storage in a helper-local singleton shared with `install_player_service`.
    player_service_slot().get().cloned()
}

fn player_service_slot() -> &'static OnceLock<Arc<PlayerService>> {
    static SERVICE: OnceLock<Arc<PlayerService>> = OnceLock::new();
    &SERVICE
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
        let filter = add_quiet_http_directives(filter);
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

fn add_quiet_http_directives(filter: EnvFilter) -> EnvFilter {
    let mut filter = filter;
    for directive in [
        "hyper_util=info",
        "lattice_actor::runtime::dispatch=info",
        "reqwest=info",
        "sqlx=warn",
        "symphonia=warn",
        "symphonia_core::formats::probe=error",
        "wasapi=info",
    ] {
        if let Ok(parsed) = directive.parse::<tracing_subscriber::filter::Directive>() {
            filter = filter.add_directive(parsed);
        }
    }
    filter
}

pub async fn runtime_shutdown() {
    engine::runtime_shutdown().await;
    if let Err(error) = shared_typescript_runtime().shutdown().await {
        tracing::warn!(%error, "TypeScript plugin runtime shutdown failed");
    }
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

pub async fn plugin_runtime_apply_state_status_json() -> String {
    serde_json::json!({
        "phase": "idle",
        "request_id": 0,
        "latest_requested_request_id": 0,
        "last_completed_request_id": 0,
        "last_error_count": 0,
        "last_errors": [],
    })
    .to_string()
}

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

    report.phase = "schedule";
    tracing::debug!(plugin_id, phase = report.phase, "plugin_disable_phase");
    let manager = shared_plugin_manager(library.plugins_dir_path());
    if let Err(error) = manager.set_enabled(plugin_id.clone(), false).await {
        report.errors.push(error.to_string());
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

pub async fn plugin_runtime_enable(
    library: &stellatune_library::LibraryHandle,
    plugin_id: String,
) -> Result<EnableReport> {
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }
    library.plugin_set_enabled(plugin_id.clone(), true).await?;

    shared_plugin_manager(library.plugins_dir_path())
        .set_enabled(plugin_id.clone(), true)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let active = shared_typescript_runtime()
        .registered_plugins()
        .await
        .into_iter()
        .any(|plugin| plugin.manifest.id == plugin_id);
    if !active {
        return Err(anyhow!(
            "plugin '{plugin_id}' is still inactive after enable"
        ));
    }

    Ok(EnableReport {
        plugin_id,
        phase: "completed",
    })
}

pub async fn plugin_runtime_apply_state(
    library: &stellatune_library::LibraryHandle,
) -> Result<ApplyStateReport> {
    let started = Instant::now();
    let plugins_dir = library.plugins_dir_path().to_path_buf();
    let disabled_ids = library
        .list_disabled_plugin_ids()
        .await?
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let disabled_count = disabled_ids.len();
    let loaded = shared_plugin_manager(&plugins_dir)
        .reconcile(disabled_ids)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(ApplyStateReport {
        phase: "completed",
        loaded: loaded.len(),
        deactivated: disabled_count,
        errors: Vec::new(),
        plan_discovered: loaded.len() + disabled_count,
        plan_disabled: disabled_count,
        plan_actions_total: loaded.len() + disabled_count,
        plan_load_new: loaded.len(),
        plan_reload_changed: 0,
        plan_deactivate: disabled_count,
        plan_ms: 0,
        execute_ms: started.elapsed().as_millis() as u64,
        total_ms: started.elapsed().as_millis() as u64,
        action_outcomes: loaded
            .into_iter()
            .map(|plugin| format!("register:{}:completed", plugin.id))
            .collect(),
        coalesced_requests: 0,
        execution_loops: 1,
    })
}
