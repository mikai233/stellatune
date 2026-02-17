use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use stellatune_audio_builtin_adapters::device_sink::{
    OutputBackend as AdapterOutputBackend, OutputDeviceSpec, default_output_spec_for_backend,
    list_output_devices, output_spec_for_route,
};
use stellatune_audio_plugin_adapters::output_sink_runtime::negotiate_output_sink_spec;
use stellatune_audio_plugin_adapters::output_sink_stage::PluginOutputSinkRouteSpec;
use stellatune_audio::assembly::{MixerPlan, PipelineMutation, ResamplerPlan};
use stellatune_audio::control::{EngineHandle, start_engine};
use stellatune_audio::types::{LfeMode, ResampleQuality};

use super::pipeline::{
    V2BackendAssembler, shared_device_sink_control, shared_runtime_sink_route_control,
};
use super::{
    DeviceSinkMetricsSnapshot, OutputBackend, OutputDeviceDescriptor,
    RuntimeOutputDeviceApplyReport, init_tracing,
};

struct RuntimeEngineMetrics {
    runtime_engine_inits_total: AtomicU64,
}

const OUTPUT_SINK_MONITOR_POLL_INTERVAL: Duration = Duration::from_millis(250);
const OUTPUT_SINK_UNDERRUN_LOG_INTERVAL: Duration = Duration::from_secs(1);
const OUTPUT_SINK_ACTIVITY_TIMEOUT: Duration = Duration::from_millis(1_500);
const OUTPUT_SINK_RING_CAPACITY_MS: i64 = 40;
const OUTPUT_SINK_MIN_LOW_WATERMARK_MS: i64 = 8;
const OUTPUT_SINK_RESUME_STABLE_TICKS: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputSinkWatermarkState {
    Unknown,
    Low,
    Healthy,
}

#[derive(Debug)]
struct OutputSinkMonitorState {
    watermark_state: OutputSinkWatermarkState,
    recovery_ready_streak: u8,
    last_underrun_total: u64,
    last_underrun_log_at: Instant,
    last_written_samples: u64,
    last_audio_activity_at: Instant,
}

impl OutputSinkMonitorState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            watermark_state: OutputSinkWatermarkState::Unknown,
            recovery_ready_streak: 0,
            last_underrun_total: 0,
            last_underrun_log_at: now,
            last_written_samples: 0,
            last_audio_activity_at: now,
        }
    }

    fn reset_watermark(&mut self) {
        self.watermark_state = OutputSinkWatermarkState::Unknown;
        self.recovery_ready_streak = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeOutputOptions {
    match_track_sample_rate: bool,
    resample_quality: ResampleQuality,
}

impl Default for RuntimeOutputOptions {
    fn default() -> Self {
        Self {
            match_track_sample_rate: false,
            resample_quality: ResampleQuality::High,
        }
    }
}

impl RuntimeEngineMetrics {
    fn new() -> Self {
        Self {
            runtime_engine_inits_total: AtomicU64::new(0),
        }
    }
}

fn runtime_engine_metrics() -> &'static RuntimeEngineMetrics {
    static METRICS: OnceLock<RuntimeEngineMetrics> = OnceLock::new();
    METRICS.get_or_init(RuntimeEngineMetrics::new)
}

fn runtime_output_options() -> &'static Mutex<RuntimeOutputOptions> {
    static OPTIONS: OnceLock<Mutex<RuntimeOutputOptions>> = OnceLock::new();
    OPTIONS.get_or_init(|| Mutex::new(RuntimeOutputOptions::default()))
}

fn ensure_output_sink_monitor_started() {
    static MONITOR_STARTED: OnceLock<()> = OnceLock::new();
    MONITOR_STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("stellatune-runtime-sink-monitor".to_string())
            .spawn(move || {
                let mut state = OutputSinkMonitorState::new();
                loop {
                    std::thread::sleep(OUTPUT_SINK_MONITOR_POLL_INTERVAL);
                    monitor_output_sink_metrics(&mut state);
                }
            });
    });
}

fn snapshot_runtime_output_options() -> RuntimeOutputOptions {
    runtime_output_options()
        .lock()
        .map(|guard| *guard)
        .unwrap_or_else(|poisoned| *poisoned.into_inner())
}

fn output_sink_watermarks_ms() -> (i64, i64) {
    let high = ((OUTPUT_SINK_RING_CAPACITY_MS.saturating_mul(3)) / 4)
        .max(OUTPUT_SINK_MIN_LOW_WATERMARK_MS + 1);
    let low = (high / 2)
        .max(OUTPUT_SINK_MIN_LOW_WATERMARK_MS)
        .min(high.saturating_sub(1));
    (low, high)
}

fn estimate_buffered_ms(metrics: DeviceSinkMetricsSnapshot, spec: OutputDeviceSpec) -> i64 {
    let channels = spec.channels.max(1) as u64;
    let sample_rate = spec.sample_rate.max(1) as u64;
    let buffered_samples = metrics
        .written_samples
        .saturating_sub(metrics.callback_provided_samples);
    let buffered_frames = buffered_samples / channels;
    ((buffered_frames.saturating_mul(1000)) / sample_rate) as i64
}

fn monitor_output_sink_metrics(state: &mut OutputSinkMonitorState) {
    if shared_runtime_sink_route_control()
        .current_plugin_route()
        .is_some()
    {
        state.reset_watermark();
        return;
    }

    let spec = match resolve_device_output_spec() {
        Ok(spec) => spec,
        Err(_) => return,
    };
    let metrics = runtime_output_sink_metrics();

    let now = Instant::now();
    if metrics.written_samples > state.last_written_samples {
        state.last_written_samples = metrics.written_samples;
        state.last_audio_activity_at = now;
    }

    if now.duration_since(state.last_audio_activity_at) > OUTPUT_SINK_ACTIVITY_TIMEOUT {
        state.reset_watermark();
        state.last_underrun_total = metrics.underrun_callbacks;
        return;
    }

    if metrics.underrun_callbacks < state.last_underrun_total {
        state.last_underrun_total = metrics.underrun_callbacks;
    } else if metrics.underrun_callbacks > state.last_underrun_total
        && now.duration_since(state.last_underrun_log_at) >= OUTPUT_SINK_UNDERRUN_LOG_INTERVAL
    {
        let delta = metrics
            .underrun_callbacks
            .saturating_sub(state.last_underrun_total);
        state.last_underrun_total = metrics.underrun_callbacks;
        state.last_underrun_log_at = now;
        tracing::warn!(
            total = metrics.underrun_callbacks,
            delta,
            callback_requested_samples = metrics.callback_requested_samples,
            callback_provided_samples = metrics.callback_provided_samples,
            "audio underrun callbacks observed"
        );
    }

    let buffered_ms = estimate_buffered_ms(metrics, spec);
    let (low_watermark_ms, high_watermark_ms) = output_sink_watermarks_ms();

    match state.watermark_state {
        OutputSinkWatermarkState::Unknown => {
            if buffered_ms <= low_watermark_ms {
                state.watermark_state = OutputSinkWatermarkState::Low;
                tracing::warn!(
                    buffered_ms,
                    low_watermark_ms,
                    high_watermark_ms,
                    "output sink buffer entered low-watermark region"
                );
            } else {
                state.watermark_state = OutputSinkWatermarkState::Healthy;
            }
        },
        OutputSinkWatermarkState::Healthy => {
            if buffered_ms <= low_watermark_ms {
                state.watermark_state = OutputSinkWatermarkState::Low;
                state.recovery_ready_streak = 0;
                tracing::warn!(
                    buffered_ms,
                    low_watermark_ms,
                    high_watermark_ms,
                    "output sink buffer low-watermark reached"
                );
            }
        },
        OutputSinkWatermarkState::Low => {
            if buffered_ms >= high_watermark_ms {
                state.recovery_ready_streak = state.recovery_ready_streak.saturating_add(1);
                if state.recovery_ready_streak >= OUTPUT_SINK_RESUME_STABLE_TICKS {
                    state.watermark_state = OutputSinkWatermarkState::Healthy;
                    state.recovery_ready_streak = 0;
                    tracing::info!(
                        buffered_ms,
                        low_watermark_ms,
                        high_watermark_ms,
                        "output sink buffer recovered above high-watermark"
                    );
                }
            } else {
                state.recovery_ready_streak = 0;
            }
        },
    }
}

fn new_runtime_engine() -> Arc<EngineHandle> {
    init_tracing();
    let inits_total = runtime_engine_metrics()
        .runtime_engine_inits_total
        .fetch_add(1, Ordering::Relaxed)
        + 1;

    let assembler = Arc::new(V2BackendAssembler::default());
    let engine =
        start_engine(assembler).unwrap_or_else(|e| panic!("failed to start v2 runtime: {e}"));

    tracing::info!(
        runtime_engine_inits_total = inits_total,
        "runtime engine initialized"
    );
    ensure_output_sink_monitor_started();
    Arc::new(engine)
}

pub fn shared_runtime_engine() -> Arc<EngineHandle> {
    static ENGINE: OnceLock<Arc<EngineHandle>> = OnceLock::new();
    if let Some(engine) = ENGINE.get() {
        tracing::debug!("reusing shared runtime engine");
        return Arc::clone(engine);
    }
    Arc::clone(ENGINE.get_or_init(new_runtime_engine))
}

pub fn runtime_list_output_devices() -> Result<Vec<OutputDeviceDescriptor>, String> {
    list_output_devices().map(|items| {
        items
            .into_iter()
            .map(|item| OutputDeviceDescriptor {
                backend: from_adapter_backend(item.backend),
                id: item.id,
                name: item.name,
            })
            .collect()
    })
}

pub fn runtime_output_sink_metrics() -> DeviceSinkMetricsSnapshot {
    let snapshot = shared_device_sink_control().metrics_snapshot();
    DeviceSinkMetricsSnapshot {
        written_samples: snapshot.written_samples,
        dropped_samples: snapshot.dropped_samples,
        callback_requested_samples: snapshot.callback_requested_samples,
        callback_provided_samples: snapshot.callback_provided_samples,
        underrun_callbacks: snapshot.underrun_callbacks,
        callback_errors: snapshot.callback_errors,
        reconfigure_attempts: snapshot.reconfigure_attempts,
        reconfigure_successes: snapshot.reconfigure_successes,
        reconfigure_failures: snapshot.reconfigure_failures,
    }
}

pub async fn runtime_set_output_device(
    backend: OutputBackend,
    device_id: Option<String>,
) -> Result<RuntimeOutputDeviceApplyReport, String> {
    let requested_device_id = normalize_device_id(device_id);
    let control = shared_device_sink_control();
    let sink_route_control = shared_runtime_sink_route_control();
    let engine = shared_runtime_engine();

    let (previous_backend, previous_device_id) = control.desired_route();
    let previous_plugin_route = sink_route_control.current_plugin_route();
    let previous_spec = resolve_current_output_spec().ok();

    let (applied_backend, applied_device_id, output_spec, fallback_to_default) =
        resolve_target_output_spec(backend, requested_device_id.as_deref())?;

    sink_route_control.clear_plugin_route();
    control.set_route(applied_backend, applied_device_id.clone());
    if let Err(error) = apply_output_spec_mutations(engine.as_ref(), output_spec).await {
        if let Some(route) = previous_plugin_route {
            sink_route_control.set_plugin_route(route);
        }
        control.set_route(previous_backend, previous_device_id.clone());
        if let Some(spec) = previous_spec {
            let _ = apply_output_spec_mutations(engine.as_ref(), spec).await;
        }
        return Err(format!(
            "failed to apply output route switch to {:?}:{:?}: {error}",
            applied_backend, applied_device_id
        ));
    }

    Ok(RuntimeOutputDeviceApplyReport {
        requested_backend: backend,
        applied_backend: from_adapter_backend(applied_backend),
        requested_device_id,
        applied_device_id,
        output_sample_rate: output_spec.sample_rate,
        output_channels: output_spec.channels,
        fallback_to_default,
    })
}

pub async fn runtime_set_output_options(
    match_track_sample_rate: bool,
    resample_quality: ResampleQuality,
) -> Result<(), String> {
    {
        let mut guard = runtime_output_options()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = RuntimeOutputOptions {
            match_track_sample_rate,
            resample_quality,
        };
    }

    let output_spec = resolve_current_output_spec()?;
    apply_output_spec_mutations(shared_runtime_engine().as_ref(), output_spec).await
}

pub async fn runtime_set_output_sink_route(
    plugin_id: String,
    type_id: String,
    config_json: String,
    target_json: String,
) -> Result<(), String> {
    let route = PluginOutputSinkRouteSpec::new(plugin_id, type_id, config_json, target_json)?;
    let route_control = shared_runtime_sink_route_control();
    let previous_route = route_control.current_plugin_route();
    let fallback_device_spec = resolve_device_output_spec().ok();
    route_control.set_plugin_route(route);

    let output_spec = match resolve_current_output_spec() {
        Ok(spec) => spec,
        Err(error) => {
            if let Some(spec) =
                resolve_rollback_output_spec(&route_control, previous_route, fallback_device_spec)
            {
                let _ = apply_output_spec_mutations(shared_runtime_engine().as_ref(), spec).await;
            }
            return Err(error);
        },
    };

    if let Err(error) =
        apply_output_spec_mutations(shared_runtime_engine().as_ref(), output_spec).await
    {
        if let Some(spec) =
            resolve_rollback_output_spec(&route_control, previous_route, fallback_device_spec)
        {
            let _ = apply_output_spec_mutations(shared_runtime_engine().as_ref(), spec).await;
        }
        return Err(format!(
            "failed to apply plugin output sink route switch: {error}"
        ));
    }

    Ok(())
}

pub async fn runtime_clear_output_sink_route() -> Result<(), String> {
    let route_control = shared_runtime_sink_route_control();
    if route_control.current_plugin_route().is_none() {
        return Ok(());
    }
    let previous_route = route_control.current_plugin_route();
    let fallback_device_spec = resolve_device_output_spec().ok();
    route_control.clear_plugin_route();
    let output_spec = match resolve_current_output_spec() {
        Ok(spec) => spec,
        Err(error) => {
            if let Some(spec) =
                resolve_rollback_output_spec(&route_control, previous_route, fallback_device_spec)
            {
                let _ = apply_output_spec_mutations(shared_runtime_engine().as_ref(), spec).await;
            }
            return Err(error);
        },
    };
    if let Err(error) =
        apply_output_spec_mutations(shared_runtime_engine().as_ref(), output_spec).await
    {
        if let Some(spec) =
            resolve_rollback_output_spec(&route_control, previous_route, fallback_device_spec)
        {
            let _ = apply_output_spec_mutations(shared_runtime_engine().as_ref(), spec).await;
        }
        return Err(format!("failed to clear plugin output sink route: {error}"));
    }
    Ok(())
}

pub async fn runtime_clear_output_sink_route_for_plugin(plugin_id: &str) -> Result<bool, String> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return Ok(false);
    }
    let route_control = shared_runtime_sink_route_control();
    let current_route = route_control.current_plugin_route();
    if !should_clear_route_for_plugin(current_route.as_ref(), plugin_id) {
        return Ok(false);
    }
    runtime_clear_output_sink_route().await?;
    Ok(true)
}

pub async fn runtime_clear_output_sink_route_if_plugin_unavailable(
    active_plugin_ids: &[String],
) -> Result<bool, String> {
    let route_control = shared_runtime_sink_route_control();
    let current_route = route_control.current_plugin_route();
    if !should_clear_route_if_plugin_unavailable(current_route.as_ref(), active_plugin_ids) {
        return Ok(false);
    }
    runtime_clear_output_sink_route().await?;
    Ok(true)
}

fn should_clear_route_for_plugin(
    current_route: Option<&PluginOutputSinkRouteSpec>,
    plugin_id: &str,
) -> bool {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return false;
    }
    current_route
        .map(|route| route.plugin_id == plugin_id)
        .unwrap_or(false)
}

fn should_clear_route_if_plugin_unavailable(
    current_route: Option<&PluginOutputSinkRouteSpec>,
    active_plugin_ids: &[String],
) -> bool {
    let Some(route) = current_route else {
        return false;
    };
    !active_plugin_ids.iter().any(|id| id == &route.plugin_id)
}

fn resolve_rollback_output_spec(
    route_control: &super::pipeline::RuntimeSinkRouteControl,
    previous_route: Option<PluginOutputSinkRouteSpec>,
    fallback_device_spec: Option<OutputDeviceSpec>,
) -> Option<OutputDeviceSpec> {
    match previous_route {
        Some(route) => {
            route_control.set_plugin_route(route);
            if let Ok(spec) = resolve_current_output_spec() {
                return Some(spec);
            }
            route_control.clear_plugin_route();
        },
        None => {
            route_control.clear_plugin_route();
            if let Ok(spec) = resolve_current_output_spec() {
                return Some(spec);
            }
        },
    }

    fallback_device_spec.or_else(|| resolve_device_output_spec().ok())
}

pub async fn runtime_prepare_hot_restart() {
    let engine = shared_runtime_engine();
    if let Err(err) = engine.stop().await {
        tracing::warn!("runtime_prepare_hot_restart stop failed: {err}");
    }
}

pub async fn runtime_shutdown() {
    let engine = shared_runtime_engine();
    if let Err(err) = engine.stop().await {
        tracing::warn!("runtime_shutdown stop failed: {err}");
    }
    if let Err(err) = engine.shutdown().await {
        tracing::warn!("runtime_shutdown command failed: {err}");
    }
}

fn resolve_target_output_spec(
    requested_backend: OutputBackend,
    requested_device_id: Option<&str>,
) -> Result<(AdapterOutputBackend, Option<String>, OutputDeviceSpec, bool), String> {
    let requested_backend = to_adapter_backend(requested_backend);
    let requested = requested_device_id.map(str::to_string);
    match output_spec_for_route(requested_backend, requested_device_id) {
        Ok(spec) => Ok((requested_backend, requested, spec, false)),
        Err(error) => {
            if requested_device_id.is_none() {
                return Err(error);
            }
            let fallback = default_output_spec_for_backend(requested_backend).map_err(|fallback_error| {
                format!(
                    "requested output device unavailable: {error}; fallback to default failed: {fallback_error}"
                )
            })?;
            Ok((requested_backend, None, fallback, true))
        },
    }
}

fn resolve_current_output_spec() -> Result<OutputDeviceSpec, String> {
    let device_spec = resolve_device_output_spec()?;
    let route_control = shared_runtime_sink_route_control();
    let Some(route) = route_control.current_plugin_route() else {
        return Ok(device_spec);
    };
    let negotiated = negotiate_output_sink_spec(
        &route.plugin_id,
        &route.type_id,
        &route.config_json,
        &route.target_json,
        device_spec.sample_rate,
        device_spec.channels,
    )?;
    Ok(OutputDeviceSpec {
        sample_rate: negotiated.sample_rate,
        channels: negotiated.channels,
    })
}

fn resolve_device_output_spec() -> Result<OutputDeviceSpec, String> {
    let control = shared_device_sink_control();
    let (backend, device_id) = control.desired_route();
    output_spec_for_route(backend, device_id.as_deref())
        .or_else(|_| default_output_spec_for_backend(backend))
        .map_err(|error| format!("failed to resolve output spec for current route: {error}"))
}

async fn apply_output_spec_mutations(
    engine: &EngineHandle,
    spec: OutputDeviceSpec,
) -> Result<(), String> {
    let output_options = snapshot_runtime_output_options();
    engine
        .apply_pipeline_mutation(PipelineMutation::SetMixerPlan {
            mixer: Some(MixerPlan::new(spec.channels, LfeMode::Mute)),
        })
        .await?;
    let resampler = if output_options.match_track_sample_rate {
        None
    } else {
        Some(ResamplerPlan::new(
            spec.sample_rate,
            output_options.resample_quality,
        ))
    };
    engine
        .apply_pipeline_mutation(PipelineMutation::SetResamplerPlan { resampler })
        .await
}

fn normalize_device_id(device_id: Option<String>) -> Option<String> {
    device_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn to_adapter_backend(backend: OutputBackend) -> AdapterOutputBackend {
    match backend {
        OutputBackend::Shared => AdapterOutputBackend::Shared,
        OutputBackend::WasapiExclusive => AdapterOutputBackend::WasapiExclusive,
    }
}

fn from_adapter_backend(backend: AdapterOutputBackend) -> OutputBackend {
    match backend {
        AdapterOutputBackend::Shared => OutputBackend::Shared,
        AdapterOutputBackend::WasapiExclusive => OutputBackend::WasapiExclusive,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DeviceSinkMetricsSnapshot, OutputDeviceSpec, OutputSinkMonitorState,
        OutputSinkWatermarkState, estimate_buffered_ms, output_sink_watermarks_ms,
        should_clear_route_for_plugin, should_clear_route_if_plugin_unavailable,
    };
    use stellatune_audio_plugin_adapters::output_sink_stage::PluginOutputSinkRouteSpec;

    fn route(plugin_id: &str) -> PluginOutputSinkRouteSpec {
        PluginOutputSinkRouteSpec::new(
            plugin_id.to_string(),
            "sink.type".to_string(),
            "{}".to_string(),
            "{}".to_string(),
        )
        .expect("route should be valid")
    }

    #[test]
    fn should_clear_route_for_plugin_only_when_current_route_matches() {
        let current = route("plugin.a");

        assert!(!should_clear_route_for_plugin(None, "plugin.a"));
        assert!(!should_clear_route_for_plugin(Some(&current), ""));
        assert!(!should_clear_route_for_plugin(Some(&current), "plugin.b"));
        assert!(should_clear_route_for_plugin(Some(&current), "plugin.a"));
    }

    #[test]
    fn should_clear_route_if_plugin_unavailable_only_when_missing_from_active_list() {
        let current = route("plugin.a");
        let empty: Vec<String> = Vec::new();
        let active_without_route = vec!["plugin.b".to_string(), "plugin.c".to_string()];
        let active_with_route = vec!["plugin.a".to_string(), "plugin.b".to_string()];

        assert!(!should_clear_route_if_plugin_unavailable(None, &empty));
        assert!(should_clear_route_if_plugin_unavailable(
            Some(&current),
            &active_without_route
        ));
        assert!(!should_clear_route_if_plugin_unavailable(
            Some(&current),
            &active_with_route
        ));
    }

    #[test]
    fn estimate_buffered_ms_uses_written_minus_provided() {
        let metrics = DeviceSinkMetricsSnapshot {
            written_samples: 48_000 * 2,
            callback_provided_samples: 24_000 * 2,
            ..Default::default()
        };
        let spec = OutputDeviceSpec {
            sample_rate: 48_000,
            channels: 2,
        };

        assert_eq!(estimate_buffered_ms(metrics, spec), 500);
    }

    #[test]
    fn output_sink_watermarks_have_valid_ordering() {
        let (low, high) = output_sink_watermarks_ms();

        assert!(low > 0);
        assert!(high > low);
    }

    #[test]
    fn monitor_state_reset_clears_watermark_progress() {
        let mut state = OutputSinkMonitorState::new();
        state.watermark_state = OutputSinkWatermarkState::Low;
        state.recovery_ready_streak = 1;
        state.reset_watermark();

        assert_eq!(state.watermark_state, OutputSinkWatermarkState::Unknown);
        assert_eq!(state.recovery_ready_streak, 0);
    }
}
