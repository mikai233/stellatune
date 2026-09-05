use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::pipeline::shared_device_sink_control;
use super::{
    DeviceSinkMetricsSnapshot, OutputBackend, OutputDeviceDescriptor,
    RuntimeOutputDeviceApplyReport, init_tracing,
};
use stellatune_audio::config::engine::ResampleQuality;
use stellatune_audio::planner::StageRegistrySnapshot;
use stellatune_audio::playback::control::PlaybackController;
use stellatune_audio::playback::runtime::{PlaybackRuntime, PlaybackRuntimeConfig};
use stellatune_audio_builtin_adapters::device_sink::{
    OutputBackend as AdapterOutputBackend, OutputDeviceSpec, default_output_spec_for_backend,
    list_output_devices, output_spec_for_route,
};
use stellatune_audio_builtin_adapters::factories::{
    RuntimeDeviceSinkFactory, SymphoniaDecoderFactory,
};

struct RuntimeEngineMetrics {
    runtime_engine_inits_total: AtomicU64,
}

const OUTPUT_SINK_MONITOR_POLL_INTERVAL: Duration = Duration::from_millis(250);
const OUTPUT_SINK_DROPPED_LOG_INTERVAL: Duration = Duration::from_millis(500);
const OUTPUT_SINK_UNDERRUN_LOG_INTERVAL: Duration = Duration::from_secs(1);
const OUTPUT_SINK_ACTIVITY_TIMEOUT: Duration = Duration::from_millis(1_500);
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
    last_dropped_total: u64,
    last_dropped_log_at: Instant,
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
            last_dropped_total: 0,
            last_dropped_log_at: now,
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
pub(super) struct RuntimeOutputOptions {
    pub(super) match_track_sample_rate: bool,
    pub(super) resample_quality: ResampleQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedOutputSpec {
    spec: OutputDeviceSpec,
    plugin_prefers_track_rate: Option<bool>,
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
            .name("stellatune-sink-monitor".to_string())
            .spawn(move || {
                let mut state = OutputSinkMonitorState::new();
                loop {
                    std::thread::sleep(OUTPUT_SINK_MONITOR_POLL_INTERVAL);
                    monitor_output_sink_metrics(&mut state);
                }
            });
    });
}

fn output_sink_watermarks_ms(capacity_ms: u64) -> (i64, i64) {
    let high = ((capacity_ms.min(i64::MAX as u64) as i64).saturating_mul(3) / 4)
        .max(OUTPUT_SINK_MIN_LOW_WATERMARK_MS + 1);
    let low = (high / 2)
        .max(OUTPUT_SINK_MIN_LOW_WATERMARK_MS)
        .min(high.saturating_sub(1));
    (low, high)
}

fn estimate_buffered_ms(metrics: DeviceSinkMetricsSnapshot, spec: OutputDeviceSpec) -> i64 {
    let channels = u64::from(spec.channel_count());
    let sample_rate = spec.sample_rate.max(1) as u64;
    let buffered_samples = metrics
        .written_samples
        .saturating_sub(metrics.callback_provided_samples);
    let buffered_frames = buffered_samples / channels;
    ((buffered_frames.saturating_mul(1000)) / sample_rate) as i64
}

fn monitor_output_sink_metrics(state: &mut OutputSinkMonitorState) {
    let spec = match resolve_device_output_spec() {
        Ok(spec) => spec,
        Err(_) => return,
    };
    let metrics = runtime_output_sink_metrics();
    if metrics.ring_capacity_ms == 0 {
        state.reset_watermark();
        return;
    }

    let now = Instant::now();
    if metrics.written_samples > state.last_written_samples {
        state.last_written_samples = metrics.written_samples;
        state.last_audio_activity_at = now;
    }

    if now.duration_since(state.last_audio_activity_at) > OUTPUT_SINK_ACTIVITY_TIMEOUT {
        state.reset_watermark();
        state.last_dropped_total = metrics.dropped_samples;
        state.last_underrun_total = metrics.underrun_callbacks;
        return;
    }

    if metrics.dropped_samples < state.last_dropped_total {
        state.last_dropped_total = metrics.dropped_samples;
    } else if metrics.dropped_samples > state.last_dropped_total
        && now.duration_since(state.last_dropped_log_at) >= OUTPUT_SINK_DROPPED_LOG_INTERVAL
    {
        let delta = metrics
            .dropped_samples
            .saturating_sub(state.last_dropped_total);
        state.last_dropped_total = metrics.dropped_samples;
        state.last_dropped_log_at = now;
        tracing::warn!(
            total = metrics.dropped_samples,
            delta,
            written_samples = metrics.written_samples,
            callback_requested_samples = metrics.callback_requested_samples,
            callback_provided_samples = metrics.callback_provided_samples,
            underrun_callbacks = metrics.underrun_callbacks,
            callback_errors = metrics.callback_errors,
            "output sink dropped samples observed"
        );
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
    let (low_watermark_ms, high_watermark_ms) = output_sink_watermarks_ms(metrics.ring_capacity_ms);

    match state.watermark_state {
        OutputSinkWatermarkState::Unknown => {
            if buffered_ms <= low_watermark_ms {
                state.watermark_state = OutputSinkWatermarkState::Low;
                tracing::warn!(
                    buffered_ms,
                    low_watermark_ms,
                    high_watermark_ms,
                    written_samples = metrics.written_samples,
                    dropped_samples = metrics.dropped_samples,
                    callback_requested_samples = metrics.callback_requested_samples,
                    callback_provided_samples = metrics.callback_provided_samples,
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
                    written_samples = metrics.written_samples,
                    dropped_samples = metrics.dropped_samples,
                    callback_requested_samples = metrics.callback_requested_samples,
                    callback_provided_samples = metrics.callback_provided_samples,
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
                        written_samples = metrics.written_samples,
                        dropped_samples = metrics.dropped_samples,
                        callback_requested_samples = metrics.callback_requested_samples,
                        callback_provided_samples = metrics.callback_provided_samples,
                        "output sink buffer recovered above high-watermark"
                    );
                }
            } else {
                state.recovery_ready_streak = 0;
            }
        },
    }
}

struct TypedPlaybackRuntime {
    runtime: Mutex<Option<PlaybackRuntime>>,
    controller: PlaybackController,
}

fn typed_playback_runtime() -> &'static TypedPlaybackRuntime {
    static RUNTIME: OnceLock<TypedPlaybackRuntime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        init_tracing();
        runtime_engine_metrics()
            .runtime_engine_inits_total
            .fetch_add(1, Ordering::Relaxed);
        let registry = StageRegistrySnapshot {
            decoders: vec![Arc::new(SymphoniaDecoderFactory::new())],
            transforms: Vec::new(),
            sink: Arc::new(RuntimeDeviceSinkFactory::new(
                shared_device_sink_control(),
                1,
            )),
        };
        let runtime = PlaybackRuntime::start(PlaybackRuntimeConfig::new(registry))
            .unwrap_or_else(|error| panic!("failed to start playback runtime: {error}"));
        let controller = runtime.controller();
        ensure_output_sink_monitor_started();
        TypedPlaybackRuntime {
            runtime: Mutex::new(Some(runtime)),
            controller,
        }
    })
}

pub fn shared_playback_controller() -> PlaybackController {
    typed_playback_runtime().controller.clone()
}

pub async fn shutdown_playback_runtime() {
    let runtime = typed_playback_runtime()
        .runtime
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(runtime) = runtime
        && let Err(error) = runtime.shutdown().await
    {
        tracing::warn!(%error, "typed playback runtime shutdown failed");
    }
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
        ring_capacity_ms: snapshot.ring_capacity_ms,
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
    let player = shared_playback_controller();

    let (previous_backend, previous_device_id) = control.desired_route();
    let previous_spec = resolve_current_output_spec().ok();

    let (applied_backend, applied_device_id, output_spec, fallback_to_default) =
        resolve_target_output_spec(backend, requested_device_id.as_deref())?;
    let resolved_output_spec = ResolvedOutputSpec {
        spec: output_spec,
        plugin_prefers_track_rate: None,
    };

    control.set_route(applied_backend, applied_device_id.clone());
    if let Err(error) = apply_output_spec_mutations(&player, resolved_output_spec).await {
        control.set_route(previous_backend, previous_device_id.clone());
        if let Some(spec) = previous_spec {
            let _ = apply_output_spec_mutations(&player, spec).await;
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
        output_sample_rate: resolved_output_spec.spec.sample_rate,
        output_channels: resolved_output_spec.spec.channel_count(),
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

    let output_spec = resolve_current_output_spec_for_output_options()?;
    apply_output_spec_mutations(&shared_playback_controller(), output_spec).await
}

pub async fn runtime_set_output_sink_route(
    _plugin_id: String,
    _type_id: String,
    _config_json: String,
    _target_json: String,
) -> Result<(), String> {
    Err("TypeScript plugins cannot implement PCM output stages; install a native external sink instead".to_string())
}

pub async fn runtime_clear_output_sink_route() -> Result<(), String> {
    Ok(())
}

pub async fn runtime_shutdown() {
    shutdown_playback_runtime().await;
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

fn resolve_current_output_spec() -> Result<ResolvedOutputSpec, String> {
    Ok(ResolvedOutputSpec {
        spec: resolve_device_output_spec()?,
        plugin_prefers_track_rate: None,
    })
}

fn resolve_device_output_spec() -> Result<OutputDeviceSpec, String> {
    let control = shared_device_sink_control();
    let (backend, device_id) = control.desired_route();
    output_spec_for_route(backend, device_id.as_deref())
        .or_else(|_| default_output_spec_for_backend(backend))
        .map_err(|error| format!("failed to resolve output spec for current route: {error}"))
}

fn resolve_current_output_spec_for_output_options() -> Result<ResolvedOutputSpec, String> {
    resolve_current_output_spec()
}

async fn apply_output_spec_mutations(
    player: &PlaybackController,
    _resolved: ResolvedOutputSpec,
) -> Result<(), String> {
    player
        .rebuild_output()
        .await
        .map_err(|error| error.to_string())
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
    use stellatune_audio_core::format::ChannelLayout;

    use super::{
        DeviceSinkMetricsSnapshot, OutputDeviceSpec, OutputSinkMonitorState,
        OutputSinkWatermarkState, estimate_buffered_ms, output_sink_watermarks_ms,
    };

    #[test]
    fn estimate_buffered_ms_uses_written_minus_provided() {
        let metrics = DeviceSinkMetricsSnapshot {
            written_samples: 48_000 * 2,
            callback_provided_samples: 24_000 * 2,
            ..Default::default()
        };
        let spec = OutputDeviceSpec {
            sample_rate: 48_000,
            channel_layout: ChannelLayout::STEREO,
        };

        assert_eq!(estimate_buffered_ms(metrics, spec), 500);
    }

    #[test]
    fn output_sink_watermarks_have_valid_ordering() {
        for capacity in [20, 40, 80] {
            let (low, high) = output_sink_watermarks_ms(capacity);
            assert!(low > 0);
            assert!(high > low);
            assert!(high < capacity as i64);
        }
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
