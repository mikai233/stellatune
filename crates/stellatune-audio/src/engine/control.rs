use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use arc_swap::ArcSwapOption;
use crossbeam_channel::{Receiver, Sender};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use stellatune_core::{Command, Event, PlayerState, TrackPlayability, TrackRef};
use stellatune_output::OutputSpec;
use stellatune_plugin_api::StOutputSinkNegotiatedSpec;
use stellatune_plugins::runtime::introspection::CapabilityKind;
use stellatune_plugins::runtime::{
    actor::WorkerControlMessage,
    worker_endpoint::{
        LyricsProviderWorkerController, OutputSinkWorkerController, SourceCatalogWorkerController,
    },
};

use crate::engine::config::{
    BUFFER_HIGH_WATERMARK_MS, BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE, BUFFER_LOW_WATERMARK_MS,
    BUFFER_LOW_WATERMARK_MS_EXCLUSIVE, BUFFER_RESUME_STABLE_TICKS, CONTROL_TICK_MS,
    TRANSITION_FADE_RAMP_MS_SEEK, TRANSITION_FADE_RAMP_MS_TRACK_SWITCH,
    TRANSITION_FADE_WAIT_EXTRA_MS, TRANSITION_FADE_WAIT_POLL_MS, UNDERRUN_LOG_INTERVAL,
};
use crate::engine::decode::decoder::assess_track_playability;
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{DecodeCtrl, EngineCtrl, InternalMsg, RuntimeDspChainEntry};
use crate::engine::session::{
    DecodeWorker, OUTPUT_SINK_QUEUE_CAP_MESSAGES, OutputPipeline, OutputSinkWorker,
    PlaybackSession, StartSessionArgs, start_decode_worker, start_session,
};
use crate::engine::update_events::emit_config_update_runtime_event;

mod commands;
mod engine_ctrl;
mod internal;
mod output_sink;
mod preload;
mod runtime_query;
mod tick;

use commands::handle_command;
use engine_ctrl::handle_engine_ctrl;
use internal::handle_internal;
use output_sink::{
    output_sink_queue_watermarks_ms, output_spec_for_plugin_sink,
    resolve_output_spec_and_sink_chunk, shutdown_output_sink_worker,
    sync_output_sink_with_active_session,
};
use preload::{
    enqueue_preload_task, event_path_from_engine_token, start_preload_worker,
    track_ref_to_engine_token, track_ref_to_event_path,
};
use runtime_query::{
    clear_runtime_query_instance_cache, clear_runtime_query_instance_cache_for_plugin,
    lyrics_fetch_json_via_runtime, lyrics_search_json_via_runtime,
    output_sink_list_targets_json_via_runtime, source_list_items_json_via_runtime,
};
use tick::{
    ensure_output_spec_prewarm, handle_tick, output_backend_for_selected, publish_player_tick_event,
};

#[cfg(debug_assertions)]
const DEBUG_PRELOAD_LOG_EVERY: u64 = 24;
#[cfg(debug_assertions)]
const DEBUG_SWITCH_LOG_EVERY: u64 = 20;
const TRACK_REF_TOKEN_PREFIX: &str = "stref-json:";
const PLUGIN_SINK_FALLBACK_SAMPLE_RATE: u32 = 48_000;
const PLUGIN_SINK_FALLBACK_CHANNELS: u16 = 2;
const PLUGIN_SINK_DEFAULT_CHUNK_FRAMES: u32 = 256;
const PLUGIN_SINK_MIN_LOW_WATERMARK_MS: i64 = 8;
const PLUGIN_SINK_MIN_HIGH_WATERMARK_MS: i64 = 16;
type SharedTrackInfo = Arc<ArcSwapOption<stellatune_core::TrackDecodeInfo>>;

fn with_runtime_service<T>(
    f: impl FnOnce(
        &stellatune_plugins::runtime::handle::SharedPluginRuntimeService,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let service = stellatune_plugins::runtime::handle::shared_runtime_service();
    f(&service)
}

fn runtime_default_config_json(
    plugin_id: &str,
    kind: CapabilityKind,
    type_id: &str,
) -> Result<String, String> {
    with_runtime_service(|service| {
        service
            .find_capability(plugin_id, kind, type_id)
            .map(|c| c.default_config_json)
            .ok_or_else(|| format!("capability not found: {plugin_id}::{type_id}"))
    })
}

fn plugin_name_from_metadata_json(plugin_id: &str, metadata_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|v| {
            v.get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| plugin_id.to_string())
}

#[derive(Debug, Clone)]
struct DspChainEntry {
    plugin_id: String,
    type_id: String,
    config: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
struct OutputSinkRouteSpec {
    plugin_id: String,
    type_id: String,
    config: serde_json::Value,
    target: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
struct OutputSinkWorkerSpec {
    route: OutputSinkRouteSpec,
    sample_rate: u32,
    channels: u16,
    chunk_frames: u32,
}

#[derive(Debug, Clone, PartialEq)]
struct OutputSinkNegotiationCache {
    route: OutputSinkRouteSpec,
    desired_spec: OutputSpec,
    negotiated: StOutputSinkNegotiatedSpec,
}

struct OpenOutputSinkWorkerArgs<'a> {
    route: &'a OutputSinkRouteSpec,
    sample_rate: u32,
    channels: u16,
    volume: Arc<AtomicU32>,
    transition_gain: Arc<AtomicU32>,
    transition_target_gain: Arc<AtomicU32>,
    transition_ramp_ms: Arc<AtomicU32>,
    internal_tx: &'a Sender<InternalMsg>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RuntimeInstanceSlotKey {
    plugin_id: String,
    type_id: String,
}

impl RuntimeInstanceSlotKey {
    fn new(plugin_id: &str, type_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            type_id: type_id.to_string(),
        }
    }
}

struct CachedSourceInstance {
    config_json: String,
    controller: SourceCatalogWorkerController,
    control_rx: Receiver<WorkerControlMessage>,
}

struct CachedLyricsInstance {
    config_json: String,
    controller: LyricsProviderWorkerController,
    control_rx: Receiver<WorkerControlMessage>,
}

struct CachedOutputSinkInstance {
    config_json: String,
    controller: OutputSinkWorkerController,
    control_rx: Receiver<WorkerControlMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionStopMode {
    TearDownSink,
    KeepSink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisruptFadeKind {
    TrackSwitch,
    Seek,
}

mod debug_metrics {
    #[cfg(debug_assertions)]
    use super::DEBUG_PRELOAD_LOG_EVERY;
    #[cfg(debug_assertions)]
    use super::DEBUG_SWITCH_LOG_EVERY;
    #[cfg(debug_assertions)]
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(debug_assertions)]
    use tracing::debug;

    #[cfg(debug_assertions)]
    static PRELOAD_REQUESTS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PRELOAD_READY: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PRELOAD_FAILED: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PRELOAD_TASK_TOTAL_MS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PRELOAD_TASK_MAX_MS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static SWITCH_COMPLETIONS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static SWITCH_LATENCY_TOTAL_MS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static SWITCH_LATENCY_MAX_MS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static OUTPUT_SINK_RECREATES: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static OUTPUT_SAMPLE_RATE_CHANGES: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static LAST_OUTPUT_SAMPLE_RATE: AtomicU64 = AtomicU64::new(0);

    #[cfg(debug_assertions)]
    fn update_max(max: &AtomicU64, value: u64) {
        let mut cur = max.load(Ordering::Relaxed);
        while value > cur {
            match max.compare_exchange(cur, value, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(v) => cur = v,
            }
        }
    }

    #[cfg(debug_assertions)]
    pub(crate) fn note_preload_request() {
        PRELOAD_REQUESTS.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_preload_request() {}

    #[cfg(debug_assertions)]
    pub(crate) fn note_preload_result(success: bool, took_ms: u64) {
        PRELOAD_TASK_TOTAL_MS.fetch_add(took_ms, Ordering::Relaxed);
        update_max(&PRELOAD_TASK_MAX_MS, took_ms);
        if success {
            PRELOAD_READY.fetch_add(1, Ordering::Relaxed);
        } else {
            PRELOAD_FAILED.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_preload_result(_success: bool, _took_ms: u64) {}

    #[cfg(debug_assertions)]
    pub(crate) fn maybe_log_preload_stats() {
        let ready = PRELOAD_READY.load(Ordering::Relaxed);
        let failed = PRELOAD_FAILED.load(Ordering::Relaxed);
        let completed = ready + failed;
        if completed == 0 || !completed.is_multiple_of(DEBUG_PRELOAD_LOG_EVERY) {
            return;
        }
        let requests = PRELOAD_REQUESTS.load(Ordering::Relaxed);
        let avg_task_ms = PRELOAD_TASK_TOTAL_MS.load(Ordering::Relaxed) as f64 / completed as f64;
        debug!(
            requests,
            ready,
            failed,
            avg_task_ms,
            max_task_ms = PRELOAD_TASK_MAX_MS.load(Ordering::Relaxed),
            "preload stats"
        );
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn maybe_log_preload_stats() {}

    #[cfg(debug_assertions)]
    fn maybe_log_track_switch_stats(completed: u64) {
        if completed == 0 || !completed.is_multiple_of(DEBUG_SWITCH_LOG_EVERY) {
            return;
        }
        let avg_switch_latency_ms =
            SWITCH_LATENCY_TOTAL_MS.load(Ordering::Relaxed) as f64 / completed as f64;
        let sink_recreates = OUTPUT_SINK_RECREATES.load(Ordering::Relaxed);
        let output_sample_rate_changes = OUTPUT_SAMPLE_RATE_CHANGES.load(Ordering::Relaxed);
        let sink_recreates_per_100 = sink_recreates as f64 * 100.0 / completed as f64;
        let output_rate_changes_per_100 =
            output_sample_rate_changes as f64 * 100.0 / completed as f64;
        debug!(
            completed,
            avg_switch_latency_ms,
            max_switch_latency_ms = SWITCH_LATENCY_MAX_MS.load(Ordering::Relaxed),
            sink_recreates,
            sink_recreates_per_100,
            output_sample_rate_changes,
            output_rate_changes_per_100,
            "track switch stats"
        );
    }

    #[cfg(debug_assertions)]
    pub(crate) fn note_track_switch_latency(elapsed_ms: u64) {
        let completed = SWITCH_COMPLETIONS.fetch_add(1, Ordering::Relaxed) + 1;
        SWITCH_LATENCY_TOTAL_MS.fetch_add(elapsed_ms, Ordering::Relaxed);
        update_max(&SWITCH_LATENCY_MAX_MS, elapsed_ms);
        maybe_log_track_switch_stats(completed);
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_track_switch_latency(_elapsed_ms: u64) {}

    #[cfg(debug_assertions)]
    pub(crate) fn note_output_sink_recreate() {
        OUTPUT_SINK_RECREATES.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_output_sink_recreate() {}

    #[cfg(debug_assertions)]
    pub(crate) fn note_output_sink_sample_rate(sample_rate: u32) {
        if sample_rate == 0 {
            return;
        }
        let prev = LAST_OUTPUT_SAMPLE_RATE.swap(sample_rate as u64, Ordering::Relaxed);
        if prev != 0 && prev != sample_rate as u64 {
            OUTPUT_SAMPLE_RATE_CHANGES.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_output_sink_sample_rate(_sample_rate: u32) {}
}

/// Handle used by higher layers (e.g. FFI) to drive the player.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CommandResponse {
    Ack,
    OutputDevices {
        devices: Vec<stellatune_core::AudioDevice>,
    },
}

struct CommandMessage {
    command: Command,
    resp_tx: Option<tokio::sync::oneshot::Sender<Result<CommandResponse, String>>>,
}

#[derive(Clone)]
pub struct EngineHandle {
    cmd_tx: Sender<CommandMessage>,
    engine_ctrl_tx: Sender<EngineCtrl>,
    events: Arc<EventHub>,
    track_info: SharedTrackInfo,
}

impl EngineHandle {
    async fn send_engine_query_request(
        &self,
        build: impl FnOnce(tokio::sync::oneshot::Sender<Result<String, String>>) -> EngineCtrl,
    ) -> Result<String, String> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        self.engine_ctrl_tx
            .send(build(resp_tx))
            .map_err(|_| "control thread exited".to_string())?;
        resp_rx
            .await
            .map_err(|_| "control thread dropped query response".to_string())?
    }

    async fn send_command_async(&self, cmd: Command) -> Result<CommandResponse, String> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(CommandMessage {
                command: cmd,
                resp_tx: Some(resp_tx),
            })
            .map_err(|_| "control thread exited".to_string())?;
        resp_rx
            .await
            .map_err(|_| "control thread dropped command response".to_string())?
    }

    pub async fn dispatch_command(&self, cmd: Command) -> Result<(), String> {
        Self::expect_ack(self.send_command_async(cmd).await?)
    }

    fn expect_ack(resp: CommandResponse) -> Result<(), String> {
        match resp {
            CommandResponse::Ack => Ok(()),
            CommandResponse::OutputDevices { .. } => {
                Err("unexpected command response payload".to_string())
            }
        }
    }

    pub async fn switch_track_ref(&self, track: TrackRef, lazy: bool) -> Result<(), String> {
        Self::expect_ack(
            self.send_command_async(Command::SwitchTrackRef { track, lazy })
                .await?,
        )
    }

    pub async fn play(&self) -> Result<(), String> {
        Self::expect_ack(self.send_command_async(Command::Play).await?)
    }

    pub async fn pause(&self) -> Result<(), String> {
        Self::expect_ack(self.send_command_async(Command::Pause).await?)
    }

    pub async fn seek_ms(&self, position_ms: u64) -> Result<(), String> {
        Self::expect_ack(
            self.send_command_async(Command::SeekMs { position_ms })
                .await?,
        )
    }

    pub async fn set_volume(&self, volume: f32) -> Result<(), String> {
        Self::expect_ack(
            self.send_command_async(Command::SetVolume { volume })
                .await?,
        )
    }

    pub async fn stop(&self) -> Result<(), String> {
        Self::expect_ack(self.send_command_async(Command::Stop).await?)
    }

    pub async fn set_output_device(
        &self,
        backend: stellatune_core::AudioBackend,
        device_id: Option<String>,
    ) -> Result<(), String> {
        Self::expect_ack(
            self.send_command_async(Command::SetOutputDevice { backend, device_id })
                .await?,
        )
    }

    pub async fn set_output_options(
        &self,
        match_track_sample_rate: bool,
        gapless_playback: bool,
        seek_track_fade: bool,
    ) -> Result<(), String> {
        Self::expect_ack(
            self.send_command_async(Command::SetOutputOptions {
                match_track_sample_rate,
                gapless_playback,
                seek_track_fade,
            })
            .await?,
        )
    }

    pub async fn set_output_sink_route(
        &self,
        route: stellatune_core::OutputSinkRoute,
    ) -> Result<(), String> {
        Self::expect_ack(
            self.send_command_async(Command::SetOutputSinkRoute { route })
                .await?,
        )
    }

    pub async fn clear_output_sink_route(&self) -> Result<(), String> {
        Self::expect_ack(
            self.send_command_async(Command::ClearOutputSinkRoute)
                .await?,
        )
    }

    pub async fn preload_track_ref(&self, track: TrackRef, position_ms: u64) -> Result<(), String> {
        Self::expect_ack(
            self.send_command_async(Command::PreloadTrackRef { track, position_ms })
                .await?,
        )
    }

    pub async fn refresh_devices(&self) -> Result<Vec<stellatune_core::AudioDevice>, String> {
        match self.send_command_async(Command::RefreshDevices).await? {
            CommandResponse::OutputDevices { devices } => Ok(devices),
            CommandResponse::Ack => Err("unexpected command response payload".to_string()),
        }
    }

    pub fn set_dsp_chain(&self, chain: Vec<stellatune_core::DspChainItem>) {
        let _ = self.engine_ctrl_tx.send(EngineCtrl::SetDspChain { chain });
    }

    pub fn reload_plugins(&self, dir: String) {
        let _ = self.engine_ctrl_tx.send(EngineCtrl::ReloadPlugins { dir });
    }

    pub async fn quiesce_plugin_usage(&self, plugin_id: String) -> Result<(), String> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        self.engine_ctrl_tx
            .send(EngineCtrl::QuiescePluginUsage { plugin_id, resp_tx })
            .map_err(|_| "control thread exited".to_string())?;
        resp_rx
            .await
            .map_err(|_| "control thread dropped quiesce response".to_string())?
    }

    pub fn set_lfe_mode(&self, mode: stellatune_core::LfeMode) {
        let _ = self.engine_ctrl_tx.send(EngineCtrl::SetLfeMode { mode });
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub fn plugin_publish_event_json(
        &self,
        plugin_id: Option<String>,
        event_json: String,
    ) -> Result<(), String> {
        match plugin_id {
            Some(plugin_id) => with_runtime_service(|service| {
                if service.current_plugin_lease_info(&plugin_id).is_none() {
                    return Err(format!("plugin not found: {plugin_id}"));
                }
                stellatune_plugins::runtime::handle::shared_runtime_service()
                    .push_host_event_json(&plugin_id, &event_json);
                Ok(())
            }),
            None => {
                stellatune_plugins::runtime::handle::shared_runtime_service()
                    .broadcast_host_event_json(&event_json);
                Ok(())
            }
        }
    }

    pub async fn list_plugins(&self) -> Vec<stellatune_core::PluginDescriptor> {
        let started = Instant::now();
        let service = stellatune_plugins::runtime::handle::shared_runtime_service();
        let mut plugin_ids = service.active_plugin_ids_async().await;
        plugin_ids.sort();
        let mut out = Vec::with_capacity(plugin_ids.len());
        for plugin_id in plugin_ids {
            let Some(generation) = service.current_plugin_lease_info_async(&plugin_id).await else {
                continue;
            };
            out.push(stellatune_core::PluginDescriptor {
                id: plugin_id.clone(),
                name: plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json),
            });
        }
        let elapsed = started.elapsed();
        if elapsed > Duration::from_millis(150) {
            warn!(
                elapsed_ms = elapsed.as_millis() as u64,
                plugin_count = out.len(),
                "list_plugins completed slowly"
            );
        }
        out
    }

    pub async fn list_dsp_types(&self) -> Vec<stellatune_core::DspTypeDescriptor> {
        let service = stellatune_plugins::runtime::handle::shared_runtime_service();
        let mut plugin_ids = service.active_plugin_ids_async().await;
        plugin_ids.sort();
        let mut out = Vec::new();
        for plugin_id in plugin_ids {
            let Some(generation) = service.current_plugin_lease_info_async(&plugin_id).await else {
                continue;
            };
            let plugin_name = plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json);
            let mut capabilities = service.list_capabilities_async(&plugin_id).await;
            capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
            for capability in capabilities {
                if capability.kind != CapabilityKind::Dsp {
                    continue;
                }
                out.push(stellatune_core::DspTypeDescriptor {
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id: capability.type_id,
                    display_name: capability.display_name,
                    config_schema_json: capability.config_schema_json,
                    default_config_json: capability.default_config_json,
                });
            }
        }
        out
    }

    pub async fn list_source_catalog_types(
        &self,
    ) -> Vec<stellatune_core::SourceCatalogTypeDescriptor> {
        let service = stellatune_plugins::runtime::handle::shared_runtime_service();
        let mut plugin_ids = service.active_plugin_ids_async().await;
        plugin_ids.sort();
        let mut out = Vec::new();
        for plugin_id in plugin_ids {
            let Some(generation) = service.current_plugin_lease_info_async(&plugin_id).await else {
                continue;
            };
            let plugin_name = plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json);
            let mut capabilities = service.list_capabilities_async(&plugin_id).await;
            capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
            for capability in capabilities {
                if capability.kind != CapabilityKind::SourceCatalog {
                    continue;
                }
                out.push(stellatune_core::SourceCatalogTypeDescriptor {
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id: capability.type_id,
                    display_name: capability.display_name,
                    config_schema_json: capability.config_schema_json,
                    default_config_json: capability.default_config_json,
                });
            }
        }
        out
    }

    pub async fn list_lyrics_provider_types(
        &self,
    ) -> Vec<stellatune_core::LyricsProviderTypeDescriptor> {
        let service = stellatune_plugins::runtime::handle::shared_runtime_service();
        let mut plugin_ids = service.active_plugin_ids_async().await;
        plugin_ids.sort();
        let mut out = Vec::new();
        for plugin_id in plugin_ids {
            let Some(generation) = service.current_plugin_lease_info_async(&plugin_id).await else {
                continue;
            };
            let plugin_name = plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json);
            let mut capabilities = service.list_capabilities_async(&plugin_id).await;
            capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
            for capability in capabilities {
                if capability.kind != CapabilityKind::LyricsProvider {
                    continue;
                }
                out.push(stellatune_core::LyricsProviderTypeDescriptor {
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id: capability.type_id,
                    display_name: capability.display_name,
                });
            }
        }
        out
    }

    pub async fn list_output_sink_types(&self) -> Vec<stellatune_core::OutputSinkTypeDescriptor> {
        let service = stellatune_plugins::runtime::handle::shared_runtime_service();
        let mut plugin_ids = service.active_plugin_ids_async().await;
        plugin_ids.sort();
        let mut out = Vec::new();
        for plugin_id in plugin_ids {
            let Some(generation) = service.current_plugin_lease_info_async(&plugin_id).await else {
                continue;
            };
            let plugin_name = plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json);
            let mut capabilities = service.list_capabilities_async(&plugin_id).await;
            capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
            for capability in capabilities {
                if capability.kind != CapabilityKind::OutputSink {
                    continue;
                }
                out.push(stellatune_core::OutputSinkTypeDescriptor {
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id: capability.type_id,
                    display_name: capability.display_name,
                    config_schema_json: capability.config_schema_json,
                    default_config_json: capability.default_config_json,
                });
            }
        }
        out
    }

    pub fn can_play_track_refs(&self, tracks: Vec<TrackRef>) -> Vec<TrackPlayability> {
        let verdicts = tracks
            .iter()
            .map(assess_track_playability)
            .collect::<Vec<_>>();
        let blocked = verdicts.iter().filter(|v| !v.playable).count();
        if blocked > 0 {
            let mut reason_counts: BTreeMap<&str, usize> = BTreeMap::new();
            for reason in verdicts
                .iter()
                .filter(|v| !v.playable)
                .map(|v| v.reason.as_deref().unwrap_or("unknown"))
            {
                *reason_counts.entry(reason).or_insert(0) += 1;
            }
            let reasons = reason_counts
                .into_iter()
                .map(|(reason, count)| format!("{reason}x{count}"))
                .collect::<Vec<_>>()
                .join(",");
            debug!(
                track_count = verdicts.len(),
                blocked,
                reasons = %reasons,
                "can_play_track_refs blocked tracks"
            );
        }
        verdicts
    }

    pub async fn source_list_items<C, R, Items>(
        &self,
        plugin_id: &str,
        type_id: &str,
        config: &C,
        request: &R,
    ) -> Result<Items, String>
    where
        C: serde::Serialize,
        R: serde::Serialize,
        Items: serde::de::DeserializeOwned,
    {
        let config_json = serde_json::to_string(config)
            .map_err(|e| format!("failed to serialize source config: {e}"))?;
        let request_json = serde_json::to_string(request)
            .map_err(|e| format!("failed to serialize source request: {e}"))?;
        let payload = self
            .send_engine_query_request(|resp_tx| EngineCtrl::SourceListItemsJson {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                config_json,
                request_json,
                resp_tx,
            })
            .await?;
        serde_json::from_str::<Items>(&payload)
            .map_err(|e| format!("failed to deserialize source response: {e}"))
    }

    pub async fn lyrics_provider_search<Q, Resp>(
        &self,
        plugin_id: &str,
        type_id: &str,
        query: &Q,
    ) -> Result<Resp, String>
    where
        Q: serde::Serialize,
        Resp: serde::de::DeserializeOwned,
    {
        let query_json = serde_json::to_string(query)
            .map_err(|e| format!("failed to serialize lyrics query: {e}"))?;
        let payload = self
            .send_engine_query_request(|resp_tx| EngineCtrl::LyricsSearchJson {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                query_json,
                resp_tx,
            })
            .await?;
        serde_json::from_str::<Resp>(&payload)
            .map_err(|e| format!("failed to deserialize lyrics search response: {e}"))
    }

    pub async fn lyrics_provider_fetch<T, Resp>(
        &self,
        plugin_id: &str,
        type_id: &str,
        track: &T,
    ) -> Result<Resp, String>
    where
        T: serde::Serialize,
        Resp: serde::de::DeserializeOwned,
    {
        let track_json = serde_json::to_string(track)
            .map_err(|e| format!("failed to serialize lyrics track: {e}"))?;
        let payload = self
            .send_engine_query_request(|resp_tx| EngineCtrl::LyricsFetchJson {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                track_json,
                resp_tx,
            })
            .await?;
        serde_json::from_str::<Resp>(&payload)
            .map_err(|e| format!("failed to deserialize lyrics fetch response: {e}"))
    }

    pub async fn output_sink_list_targets<C, Targets>(
        &self,
        plugin_id: &str,
        type_id: &str,
        config: &C,
    ) -> Result<Targets, String>
    where
        C: serde::Serialize,
        Targets: serde::de::DeserializeOwned,
    {
        let config_json = serde_json::to_string(config)
            .map_err(|e| format!("failed to serialize output sink config: {e}"))?;
        let payload = self
            .send_engine_query_request(|resp_tx| EngineCtrl::OutputSinkListTargetsJson {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                config_json,
                resp_tx,
            })
            .await?;
        serde_json::from_str::<Targets>(&payload)
            .map_err(|e| format!("failed to deserialize output sink targets: {e}"))
    }

    pub fn current_track_info(&self) -> Option<stellatune_core::TrackDecodeInfo> {
        self.track_info
            .load_full()
            .map(|track_info| track_info.as_ref().clone())
    }
}

pub fn start_engine() -> EngineHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let (engine_ctrl_tx, engine_ctrl_rx) = crossbeam_channel::unbounded();
    let (internal_tx, internal_rx) = crossbeam_channel::unbounded();

    let events = Arc::new(EventHub::new());
    let thread_events = Arc::clone(&events);

    let track_info: SharedTrackInfo = Arc::new(ArcSwapOption::new(None));
    let track_info_for_thread = Arc::clone(&track_info);
    let _join: JoinHandle<()> = thread::Builder::new()
        .name("stellatune-control".to_string())
        .spawn(move || {
            let _rt_guard = stellatune_output::enable_realtime_audio_thread();
            run_control_loop(
                ControlLoopChannels {
                    cmd_rx,
                    engine_ctrl_rx,
                    internal_rx,
                    internal_tx,
                },
                ControlLoopDeps {
                    events: thread_events,
                    track_info: track_info_for_thread,
                },
            )
        })
        .expect("failed to spawn stellatune-control thread");

    EngineHandle {
        cmd_tx,
        engine_ctrl_tx,
        events,
        track_info,
    }
}

struct EngineState {
    player_state: PlayerState,
    position_ms: i64,
    current_track: Option<String>,
    session: Option<PlaybackSession>,
    wants_playback: bool,
    volume: f32,
    volume_atomic: Arc<AtomicU32>,
    last_underrun_total: u64,
    last_underrun_log_at: Instant,
    play_request_started_at: Option<Instant>,
    cached_output_spec: Option<OutputSpec>,
    output_spec_prewarm_inflight: bool,
    output_spec_token: u64,
    pending_session_start: bool,
    buffering_ready_streak: u8,
    desired_dsp_chain: Vec<DspChainEntry>,
    lfe_mode: stellatune_core::LfeMode,
    selected_backend: stellatune_core::AudioBackend,
    selected_device_id: Option<String>,
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
    desired_output_sink_route: Option<OutputSinkRouteSpec>,
    output_sink_chunk_frames: u32,
    output_sink_negotiation_cache: Option<OutputSinkNegotiationCache>,
    output_sink_worker: Option<OutputSinkWorker>,
    output_sink_worker_spec: Option<OutputSinkWorkerSpec>,
    output_pipeline: Option<OutputPipeline>,
    decode_worker: Option<DecodeWorker>,
    preload_worker: Option<PreloadWorker>,
    preload_token: u64,
    requested_preload_path: Option<String>,
    requested_preload_position_ms: u64,
    source_instances: HashMap<RuntimeInstanceSlotKey, CachedSourceInstance>,
    lyrics_instances: HashMap<RuntimeInstanceSlotKey, CachedLyricsInstance>,
    output_sink_instances: HashMap<RuntimeInstanceSlotKey, CachedOutputSinkInstance>,
    switch_timing_seq: u64,
    manual_switch_timing: Option<ManualSwitchTiming>,
    seek_position_guard: Option<SeekPositionGuard>,
    position_session_id: u64,
}

#[derive(Debug, Clone)]
struct ManualSwitchTiming {
    id: u64,
    from_track: Option<String>,
    to_track: String,
    began_at: Instant,
    fade_done_at: Option<Instant>,
    stop_done_at: Option<Instant>,
    committed_at: Option<Instant>,
    play_requested_at: Option<Instant>,
    session_ready_at: Option<Instant>,
}

#[derive(Debug, Clone)]
struct SeekPositionGuard {
    target_ms: i64,
    origin_ms: i64,
    requested_at: Instant,
}

struct PreloadWorker {
    tx: mpsc::UnboundedSender<PreloadJob>,
    join: tokio::task::JoinHandle<()>,
}

struct ControlLoopChannels {
    cmd_rx: Receiver<CommandMessage>,
    engine_ctrl_rx: Receiver<EngineCtrl>,
    internal_rx: Receiver<InternalMsg>,
    internal_tx: Sender<InternalMsg>,
}

struct ControlLoopDeps {
    events: Arc<EventHub>,
    track_info: SharedTrackInfo,
}

enum PreloadJob {
    Task {
        path: String,
        position_ms: u64,
        token: u64,
    },
    Shutdown,
}

impl EngineState {
    fn new() -> Self {
        let volume = 1.0_f32;
        Self {
            player_state: PlayerState::Stopped,
            position_ms: 0,
            current_track: None,
            session: None,
            wants_playback: false,
            volume,
            volume_atomic: Arc::new(AtomicU32::new(volume.to_bits())),
            last_underrun_total: 0,
            last_underrun_log_at: Instant::now(),
            play_request_started_at: None,
            cached_output_spec: None,
            output_spec_prewarm_inflight: false,
            output_spec_token: 0,
            pending_session_start: false,
            buffering_ready_streak: 0,
            desired_dsp_chain: Vec::new(),
            lfe_mode: stellatune_core::LfeMode::default(),
            selected_backend: stellatune_core::AudioBackend::Shared,
            selected_device_id: None,
            match_track_sample_rate: false,
            gapless_playback: true,
            seek_track_fade: true,
            desired_output_sink_route: None,
            output_sink_chunk_frames: 0,
            output_sink_negotiation_cache: None,
            output_sink_worker: None,
            output_sink_worker_spec: None,
            output_pipeline: None,
            decode_worker: None,
            preload_worker: None,
            preload_token: 0,
            requested_preload_path: None,
            requested_preload_position_ms: 0,
            source_instances: HashMap::new(),
            lyrics_instances: HashMap::new(),
            output_sink_instances: HashMap::new(),
            switch_timing_seq: 0,
            manual_switch_timing: None,
            seek_position_guard: None,
            position_session_id: 0,
        }
    }
}

fn run_control_loop(channels: ControlLoopChannels, deps: ControlLoopDeps) {
    let ControlLoopChannels {
        cmd_rx,
        engine_ctrl_rx,
        internal_rx,
        internal_tx,
    } = channels;
    let ControlLoopDeps { events, track_info } = deps;

    info!("control thread started");
    let mut state = EngineState::new();
    state.decode_worker = Some(start_decode_worker(internal_tx.clone()));
    state.preload_worker = Some(start_preload_worker(internal_tx.clone()));
    let tick = crossbeam_channel::tick(Duration::from_millis(CONTROL_TICK_MS));

    // Prewarm output spec in the background so the first Play doesn't pay the WASAPI/COM setup cost.
    ensure_output_spec_prewarm(&mut state, &internal_tx);

    loop {
        crossbeam_channel::select! {
            recv(cmd_rx) -> msg => {
                let Ok(cmd) = msg else { break };
                let result = handle_command(
                    cmd.command,
                    &mut state,
                    &events,
                    &internal_tx,
                    &track_info,
                );
                if let Some(resp_tx) = cmd.resp_tx {
                    let _ = resp_tx.send(result.response);
                }
                if result.should_shutdown {
                    break;
                }
            }
            recv(engine_ctrl_rx) -> msg => {
                let Ok(msg) = msg else { break };
                handle_engine_ctrl(msg, &mut state, &events, &internal_tx, &track_info);
            }
            recv(internal_rx) -> msg => {
                let Ok(msg) = msg else { break };
                handle_internal(msg, &mut state, &events, &internal_tx, &track_info);
            }
            recv(tick) -> _ => {
                publish_player_tick_event(&state);
                handle_tick(
                    &mut state,
                    &events,
                    &internal_tx,
                    &track_info,
                );
            }
        }
    }

    stop_all_audio(&mut state, &track_info);
    shutdown_decode_worker(&mut state);
    shutdown_preload_worker(&mut state);
    events.emit(Event::Log {
        message: "control thread exited".to_string(),
    });
    info!("control thread exited");
}

fn parse_dsp_chain(
    chain: Vec<stellatune_core::DspChainItem>,
) -> Result<Vec<DspChainEntry>, String> {
    chain
        .into_iter()
        .map(|item| {
            let config = item.config::<serde_json::Value>().map_err(|e| {
                format!(
                    "invalid dsp config_json for {}::{}: {e}",
                    item.plugin_id, item.type_id
                )
            })?;
            Ok(DspChainEntry {
                plugin_id: item.plugin_id,
                type_id: item.type_id,
                config,
            })
        })
        .collect()
}

fn parse_output_sink_route(
    route: stellatune_core::OutputSinkRoute,
) -> Result<OutputSinkRouteSpec, String> {
    let config = route
        .config::<serde_json::Value>()
        .map_err(|e| format!("invalid output sink route config_json: {e}"))?;
    let target = route
        .target::<serde_json::Value>()
        .map_err(|e| format!("invalid output sink route target_json: {e}"))?;
    Ok(OutputSinkRouteSpec {
        plugin_id: route.plugin_id,
        type_id: route.type_id,
        config,
        target,
    })
}

fn maybe_fade_out_before_disrupt(state: &EngineState, kind: DisruptFadeKind) {
    if !state.seek_track_fade {
        maybe_reset_output_sink_after_disrupt(state);
        return;
    }
    let Some(session) = state.session.as_ref() else {
        maybe_reset_output_sink_after_disrupt(state);
        return;
    };
    let pending_audio = if state.desired_output_sink_route.is_some() {
        state
            .output_sink_worker
            .as_ref()
            .map(|worker| worker.pending_samples())
            .unwrap_or(0)
            > 0
    } else {
        session.buffered_samples.load(Ordering::Relaxed) > 0
    };
    let playback_active = state.wants_playback
        && matches!(
            state.player_state,
            PlayerState::Playing | PlayerState::Buffering
        );
    let should_apply = match kind {
        DisruptFadeKind::TrackSwitch => playback_active && pending_audio,
        DisruptFadeKind::Seek => playback_active,
    };
    if !should_apply {
        maybe_reset_output_sink_after_disrupt(state);
        return;
    }
    let ramp_ms = match kind {
        DisruptFadeKind::TrackSwitch => TRANSITION_FADE_RAMP_MS_TRACK_SWITCH,
        DisruptFadeKind::Seek => TRANSITION_FADE_RAMP_MS_SEEK,
    };
    session
        .transition_ramp_ms
        .store(ramp_ms as u32, Ordering::Relaxed);
    session
        .transition_target_gain
        .store(0.0f32.to_bits(), Ordering::Relaxed);
    let should_wait = match kind {
        DisruptFadeKind::TrackSwitch => pending_audio,
        DisruptFadeKind::Seek => {
            if state.player_state == PlayerState::Playing {
                // In active playback, we should wait for fade-down even if queue estimation is low.
                true
            } else {
                pending_audio
            }
        }
    };
    if !should_wait {
        return;
    }
    let wait_timeout_ms = ramp_ms.saturating_add(TRANSITION_FADE_WAIT_EXTRA_MS);
    let started = Instant::now();
    while started.elapsed().as_millis() < wait_timeout_ms as u128 {
        let current = f32::from_bits(session.transition_gain.load(Ordering::Relaxed));
        if current <= 0.05 {
            break;
        }
        thread::sleep(Duration::from_millis(TRANSITION_FADE_WAIT_POLL_MS));
    }
    maybe_reset_output_sink_after_disrupt(state);
}

fn maybe_reset_output_sink_after_disrupt(state: &EngineState) {
    let Some(worker) = state.output_sink_worker.as_ref() else {
        return;
    };
    // Use sink-level reset ABI. This clears plugin-internal buffers.
    if let Err(e) = worker.reset_for_disrupt() {
        debug!("output sink disrupt reset skipped: {e}");
    }
}

fn force_transition_gain_unity(session: Option<&PlaybackSession>) {
    let Some(session) = session else {
        return;
    };
    session
        .transition_target_gain
        .store(1.0f32.to_bits(), Ordering::Relaxed);
    session
        .transition_gain
        .store(1.0f32.to_bits(), Ordering::Relaxed);
}

fn set_state(state: &mut EngineState, events: &Arc<EventHub>, new_state: PlayerState) {
    if state.player_state == new_state {
        return;
    }
    state.buffering_ready_streak = 0;
    state.player_state = new_state;
    events.emit(Event::StateChanged { state: new_state });
}

fn next_position_session_id(state: &mut EngineState) -> u64 {
    state.position_session_id = state.position_session_id.wrapping_add(1);
    if state.position_session_id == 0 {
        state.position_session_id = 1;
    }
    state.position_session_id
}

fn emit_position_event(state: &EngineState, events: &Arc<EventHub>) {
    let path = state
        .current_track
        .as_ref()
        .map(|token| event_path_from_engine_token(token))
        .unwrap_or_default();
    events.emit(Event::Position {
        ms: state.position_ms,
        path,
        session_id: state.position_session_id,
    });
}

fn apply_dsp_chain(state: &mut EngineState) -> Result<(), String> {
    let Some(session) = state.session.as_ref() else {
        return Ok(());
    };

    let chain_spec = state.desired_dsp_chain.clone();
    let mut chain = Vec::with_capacity(chain_spec.len());
    for item in &chain_spec {
        let config_json = serde_json::to_string(&item.config).map_err(|e| {
            format!(
                "invalid DSP config json for {}::{}: {e}",
                item.plugin_id, item.type_id
            )
        })?;
        chain.push(RuntimeDspChainEntry {
            plugin_id: item.plugin_id.clone(),
            type_id: item.type_id.clone(),
            config_json,
        });
    }

    let _ = session.ctrl_tx.send(DecodeCtrl::SetDspChain { chain });
    Ok(())
}

fn stop_decode_session(
    state: &mut EngineState,
    track_info: &SharedTrackInfo,
    mode: SessionStopMode,
) {
    track_info.store(None);

    let Some(session) = state.session.take() else {
        if mode == SessionStopMode::TearDownSink {
            shutdown_output_sink_worker(state);
        }
        return;
    };

    let _ = session.ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
        tx: None,
        output_sink_chunk_frames: 0,
    });
    if mode == SessionStopMode::TearDownSink {
        shutdown_output_sink_worker(state);
    }

    let buffered_samples = session.buffered_samples.load(Ordering::Relaxed);
    session.output_enabled.store(false, Ordering::Release);
    let _ = session.ctrl_tx.send(DecodeCtrl::Stop);

    debug!(
        track = state
            .current_track
            .as_ref()
            .map(|t| event_path_from_engine_token(t))
            .unwrap_or_else(|| "<none>".to_string()),
        player_state = ?state.player_state,
        wants_playback = state.wants_playback,
        position_ms = state.position_ms,
        out_sample_rate = session.out_sample_rate,
        out_channels = session.out_channels,
        buffered_samples,
        "session stopped"
    );
}

fn shutdown_decode_worker(state: &mut EngineState) {
    if let Some(worker) = state.decode_worker.take() {
        worker.shutdown();
        debug!("decode worker stopped");
    }
}

fn decode_worker_unavailable_error_message(reason: &str) -> String {
    format!("decode worker unavailable: {reason}")
}

fn is_decode_worker_unavailable_error(message: &str) -> bool {
    message.starts_with("decode worker unavailable")
        || message == "decoder thread exited unexpectedly"
}

fn ensure_decode_worker(state: &mut EngineState, internal_tx: &Sender<InternalMsg>) {
    if state.decode_worker.is_none() {
        state.decode_worker = Some(start_decode_worker(internal_tx.clone()));
        warn!("decode worker missing; recreated");
    }
}

fn restart_decode_worker(state: &mut EngineState, internal_tx: &Sender<InternalMsg>, reason: &str) {
    shutdown_decode_worker(state);
    state.decode_worker = Some(start_decode_worker(internal_tx.clone()));
    warn!(reason, "decode worker restarted after unavailable error");
}

fn shutdown_preload_worker(state: &mut EngineState) {
    let Some(worker) = state.preload_worker.take() else {
        return;
    };
    let _ = worker.tx.send(PreloadJob::Shutdown);
    if !worker.join.is_finished() {
        worker.join.abort();
    }
    debug!("preload worker stopped");
}

fn drop_output_pipeline(state: &mut EngineState) {
    if state.output_pipeline.take().is_some() {
        debug!("output pipeline dropped");
    }
}

fn stop_all_audio(state: &mut EngineState, track_info: &SharedTrackInfo) {
    stop_decode_session(state, track_info, SessionStopMode::TearDownSink);
    drop_output_pipeline(state);
}
