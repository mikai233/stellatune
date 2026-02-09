use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use arc_swap::ArcSwapOption;
use crossbeam_channel::{Receiver, Sender};
use tracing::{debug, error, info, warn};

use stellatune_core::{
    Command, Event, HostEventTopic, HostPlayerTickPayload, PlayerState, PluginRuntimeEvent,
    TrackPlayability, TrackRef,
};
use stellatune_output::{OutputSpec, output_spec_for_device};
use stellatune_plugin_api::{StAudioSpec, v2::StOutputSinkNegotiatedSpecV2};
use stellatune_plugins::runtime::CapabilityKind;
use stellatune_plugins::{PluginManager, default_host_vtable};

use crate::engine::config::{
    BUFFER_HIGH_WATERMARK_MS, BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE, BUFFER_LOW_WATERMARK_MS,
    BUFFER_LOW_WATERMARK_MS_EXCLUSIVE, CONTROL_TICK_MS, SEEK_TRACK_FADE_WAIT_POLL_MS,
    SEEK_TRACK_FADE_WAIT_TIMEOUT_MS, UNDERRUN_LOG_INTERVAL,
};
use crate::engine::decode::decoder::{assess_track_playability, open_engine_decoder};
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{DecodeCtrl, EngineCtrl, InternalMsg, PredecodedChunk};
use crate::engine::plugin_event_hub::PluginEventHub;
use crate::engine::session::{
    DecodeWorker, OUTPUT_SINK_QUEUE_CAP_MESSAGES, OutputPipeline, OutputSinkWorker,
    PlaybackSession, PromotedPreload, StartSessionArgs, start_decode_worker, start_session,
};

#[cfg(debug_assertions)]
const DEBUG_PRELOAD_LOG_EVERY: u64 = 24;
const TRACK_REF_TOKEN_PREFIX: &str = "stref-json:";
const PLUGIN_SINK_FALLBACK_SAMPLE_RATE: u32 = 48_000;
const PLUGIN_SINK_FALLBACK_CHANNELS: u16 = 2;
const PLUGIN_SINK_DEFAULT_CHUNK_FRAMES: u32 = 256;
const PLUGIN_SINK_MIN_LOW_WATERMARK_MS: i64 = 2;
const PLUGIN_SINK_MIN_HIGH_WATERMARK_MS: i64 = 4;
type SharedTrackInfo = Arc<ArcSwapOption<stellatune_core::TrackDecodeInfo>>;

fn snapshot_plugins(plugins: &Arc<Mutex<PluginManager>>) -> Result<PluginManager, String> {
    plugins
        .lock()
        .map(|pm| pm.clone())
        .map_err(|_| "plugins mutex poisoned".to_string())
}

fn with_runtime_service<T>(
    f: impl FnOnce(&stellatune_plugins::v2::PluginRuntimeService) -> Result<T, String>,
) -> Result<T, String> {
    let shared = stellatune_plugins::v2::shared_runtime_service_v2();
    let service = shared
        .lock()
        .map_err(|_| "plugin runtime v2 mutex poisoned".to_string())?;
    f(&service)
}

fn runtime_default_config_json(
    plugin_id: &str,
    kind: CapabilityKind,
    type_id: &str,
) -> Result<String, String> {
    with_runtime_service(|service| {
        service
            .resolve_active_capability(plugin_id, kind, type_id)
            .map(|c| c.default_config_json)
            .ok_or_else(|| format!("capability not found: {plugin_id}::{type_id}"))
    })
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

struct OpenOutputSinkWorkerArgs<'a> {
    route: &'a OutputSinkRouteSpec,
    sample_rate: u32,
    channels: u16,
    volume: Arc<AtomicU32>,
    transition_gain: Arc<AtomicU32>,
    transition_target_gain: Arc<AtomicU32>,
    internal_tx: &'a Sender<InternalMsg>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RuntimeInstanceKey {
    plugin_id: String,
    type_id: String,
    config_json: String,
}

impl RuntimeInstanceKey {
    fn new(plugin_id: &str, type_id: &str, config_json: String) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            type_id: type_id.to_string(),
            config_json,
        }
    }
}

mod debug_metrics {
    #[cfg(debug_assertions)]
    use super::DEBUG_PRELOAD_LOG_EVERY;
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
}

/// Handle used by higher layers (e.g. FFI) to drive the player.
#[derive(Clone)]
pub struct EngineHandle {
    cmd_tx: Sender<Command>,
    engine_ctrl_tx: Sender<EngineCtrl>,
    events: Arc<EventHub>,
    plugin_events: Arc<PluginEventHub>,
    plugins: Arc<Mutex<PluginManager>>,
    track_info: SharedTrackInfo,
}

impl EngineHandle {
    fn send_engine_query_request(
        &self,
        build: impl FnOnce(Sender<Result<String, String>>) -> EngineCtrl,
    ) -> Result<String, String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.engine_ctrl_tx
            .send(build(resp_tx))
            .map_err(|_| "control thread exited".to_string())?;
        resp_rx
            .recv()
            .map_err(|_| "control thread dropped query response".to_string())?
    }

    pub fn send_command(&self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn set_dsp_chain(&self, chain: Vec<stellatune_core::DspChainItem>) {
        let _ = self.engine_ctrl_tx.send(EngineCtrl::SetDspChain { chain });
    }

    pub fn reload_plugins(&self, dir: String) {
        let _ = self.engine_ctrl_tx.send(EngineCtrl::ReloadPlugins { dir });
    }

    pub fn reload_plugins_with_disabled(&self, dir: String, disabled_ids: Vec<String>) {
        let _ = self
            .engine_ctrl_tx
            .send(EngineCtrl::ReloadPluginsWithDisabled { dir, disabled_ids });
    }

    pub fn set_lfe_mode(&self, mode: stellatune_core::LfeMode) {
        let _ = self.engine_ctrl_tx.send(EngineCtrl::SetLfeMode { mode });
    }

    pub fn subscribe_events(&self) -> Receiver<Event> {
        self.events.subscribe()
    }

    pub fn subscribe_plugin_runtime_events(&self) -> Receiver<PluginRuntimeEvent> {
        self.plugin_events.subscribe()
    }

    pub fn emit_plugin_runtime_event(&self, event: PluginRuntimeEvent) {
        self.plugin_events.emit(event);
    }

    pub fn plugin_publish_event_json(
        &self,
        plugin_id: Option<String>,
        event_json: String,
    ) -> Result<(), String> {
        let pm = self
            .plugins
            .lock()
            .map_err(|_| "plugins mutex poisoned".to_string())?;
        match plugin_id {
            Some(plugin_id) => pm
                .push_host_event_json(&plugin_id, &event_json)
                .map_err(|e| e.to_string()),
            None => {
                pm.broadcast_host_event_json(&event_json);
                Ok(())
            }
        }
    }

    pub fn list_plugins(&self) -> Vec<stellatune_core::PluginDescriptor> {
        let Ok(pm) = snapshot_plugins(&self.plugins) else {
            return Vec::new();
        };
        pm.plugins()
            .iter()
            .map(|p| stellatune_core::PluginDescriptor {
                id: p.library().id(),
                name: p.library().name(),
            })
            .collect()
    }

    pub fn list_dsp_types(&self) -> Vec<stellatune_core::DspTypeDescriptor> {
        let Ok(pm) = snapshot_plugins(&self.plugins) else {
            return Vec::new();
        };
        pm.list_dsp_types()
            .into_iter()
            .map(|t| stellatune_core::DspTypeDescriptor {
                plugin_id: t.plugin_id,
                plugin_name: t.plugin_name,
                type_id: t.type_id,
                display_name: t.display_name,
                config_schema_json: t.config_schema_json,
                default_config_json: t.default_config_json,
            })
            .collect()
    }

    pub fn list_source_catalog_types(&self) -> Vec<stellatune_core::SourceCatalogTypeDescriptor> {
        let Ok(pm) = snapshot_plugins(&self.plugins) else {
            return Vec::new();
        };
        pm.list_source_catalog_types()
            .into_iter()
            .map(|t| stellatune_core::SourceCatalogTypeDescriptor {
                plugin_id: t.plugin_id,
                plugin_name: t.plugin_name,
                type_id: t.type_id,
                display_name: t.display_name,
                config_schema_json: t.config_schema_json,
                default_config_json: t.default_config_json,
            })
            .collect()
    }

    pub fn list_lyrics_provider_types(&self) -> Vec<stellatune_core::LyricsProviderTypeDescriptor> {
        let Ok(pm) = snapshot_plugins(&self.plugins) else {
            return Vec::new();
        };
        pm.list_lyrics_provider_types()
            .into_iter()
            .map(|t| stellatune_core::LyricsProviderTypeDescriptor {
                plugin_id: t.plugin_id,
                plugin_name: t.plugin_name,
                type_id: t.type_id,
                display_name: t.display_name,
            })
            .collect()
    }

    pub fn list_output_sink_types(&self) -> Vec<stellatune_core::OutputSinkTypeDescriptor> {
        let Ok(pm) = snapshot_plugins(&self.plugins) else {
            return Vec::new();
        };
        pm.list_output_sink_types()
            .into_iter()
            .map(|t| stellatune_core::OutputSinkTypeDescriptor {
                plugin_id: t.plugin_id,
                plugin_name: t.plugin_name,
                type_id: t.type_id,
                display_name: t.display_name,
                config_schema_json: t.config_schema_json,
                default_config_json: t.default_config_json,
            })
            .collect()
    }

    pub fn can_play_track_refs(&self, tracks: Vec<TrackRef>) -> Vec<TrackPlayability> {
        let Ok(pm) = snapshot_plugins(&self.plugins) else {
            warn!(
                track_count = tracks.len(),
                "can_play_track_refs failed: plugins mutex poisoned"
            );
            return tracks
                .into_iter()
                .map(|track| TrackPlayability {
                    track,
                    playable: false,
                    reason: Some("plugins_unavailable".to_string()),
                })
                .collect();
        };
        let verdicts = tracks
            .iter()
            .map(|track| assess_track_playability(track, &pm))
            .collect::<Vec<_>>();
        let blocked = verdicts.iter().filter(|v| !v.playable).count();
        if blocked > 0 {
            let reasons = verdicts
                .iter()
                .filter(|v| !v.playable)
                .map(|v| v.reason.as_deref().unwrap_or("unknown"))
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

    pub fn source_list_items<C, R, Items>(
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
        let payload =
            self.send_engine_query_request(|resp_tx| EngineCtrl::SourceListItemsJson {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                config_json,
                request_json,
                resp_tx,
            })?;
        serde_json::from_str::<Items>(&payload)
            .map_err(|e| format!("failed to deserialize source response: {e}"))
    }

    pub fn lyrics_provider_search<Q, Resp>(
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
        let payload = self.send_engine_query_request(|resp_tx| EngineCtrl::LyricsSearchJson {
            plugin_id: plugin_id.to_string(),
            type_id: type_id.to_string(),
            query_json,
            resp_tx,
        })?;
        serde_json::from_str::<Resp>(&payload)
            .map_err(|e| format!("failed to deserialize lyrics search response: {e}"))
    }

    pub fn lyrics_provider_fetch<T, Resp>(
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
        let payload = self.send_engine_query_request(|resp_tx| EngineCtrl::LyricsFetchJson {
            plugin_id: plugin_id.to_string(),
            type_id: type_id.to_string(),
            track_json,
            resp_tx,
        })?;
        serde_json::from_str::<Resp>(&payload)
            .map_err(|e| format!("failed to deserialize lyrics fetch response: {e}"))
    }

    pub fn output_sink_list_targets<C, Targets>(
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
        let payload =
            self.send_engine_query_request(|resp_tx| EngineCtrl::OutputSinkListTargetsJson {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                config_json,
                resp_tx,
            })?;
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
    start_engine_with_plugins(Arc::new(Mutex::new(PluginManager::new(
        default_host_vtable(),
    ))))
}

pub fn start_engine_with_plugins(plugins: Arc<Mutex<PluginManager>>) -> EngineHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let (engine_ctrl_tx, engine_ctrl_rx) = crossbeam_channel::unbounded();
    let (internal_tx, internal_rx) = crossbeam_channel::unbounded();

    let events = Arc::new(EventHub::new());
    let thread_events = Arc::clone(&events);
    let plugin_events = Arc::new(PluginEventHub::new());
    let thread_plugin_events = Arc::clone(&plugin_events);

    let track_info: SharedTrackInfo = Arc::new(ArcSwapOption::new(None));

    let plugins_for_thread = Arc::clone(&plugins);
    let track_info_for_thread = Arc::clone(&track_info);
    let _join: JoinHandle<()> = thread::Builder::new()
        .name("stellatune-control".to_string())
        .spawn(move || {
            run_control_loop(
                ControlLoopChannels {
                    cmd_rx,
                    engine_ctrl_rx,
                    internal_rx,
                    internal_tx,
                },
                ControlLoopDeps {
                    events: thread_events,
                    plugin_events: thread_plugin_events,
                    plugins: plugins_for_thread,
                    track_info: track_info_for_thread,
                },
            )
        })
        .expect("failed to spawn stellatune-control thread");

    EngineHandle {
        cmd_tx,
        engine_ctrl_tx,
        events,
        plugin_events,
        plugins,
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
    desired_dsp_chain: Vec<DspChainEntry>,
    lfe_mode: stellatune_core::LfeMode,
    selected_backend: stellatune_core::AudioBackend,
    selected_device_id: Option<String>,
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
    desired_output_sink_route: Option<OutputSinkRouteSpec>,
    output_sink_chunk_frames: u32,
    output_sink_worker: Option<OutputSinkWorker>,
    output_sink_worker_spec: Option<OutputSinkWorkerSpec>,
    output_pipeline: Option<OutputPipeline>,
    decode_worker: Option<DecodeWorker>,
    preload_worker: Option<PreloadWorker>,
    preload_token: u64,
    requested_preload_path: Option<String>,
    requested_preload_position_ms: u64,
    source_instances: HashMap<RuntimeInstanceKey, stellatune_plugins::v2::SourceCatalogInstanceV2>,
    lyrics_instances: HashMap<RuntimeInstanceKey, stellatune_plugins::v2::LyricsProviderInstanceV2>,
    output_sink_instances:
        HashMap<RuntimeInstanceKey, stellatune_plugins::v2::OutputSinkInstanceV2>,
}

struct PreloadWorker {
    tx: Sender<PreloadJob>,
    join: JoinHandle<()>,
}

struct ControlLoopChannels {
    cmd_rx: Receiver<Command>,
    engine_ctrl_rx: Receiver<EngineCtrl>,
    internal_rx: Receiver<InternalMsg>,
    internal_tx: Sender<InternalMsg>,
}

struct ControlLoopDeps {
    events: Arc<EventHub>,
    plugin_events: Arc<PluginEventHub>,
    plugins: Arc<Mutex<PluginManager>>,
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
            desired_dsp_chain: Vec::new(),
            lfe_mode: stellatune_core::LfeMode::default(),
            selected_backend: stellatune_core::AudioBackend::Shared,
            selected_device_id: None,
            match_track_sample_rate: false,
            gapless_playback: true,
            seek_track_fade: true,
            desired_output_sink_route: None,
            output_sink_chunk_frames: 0,
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
    let ControlLoopDeps {
        events,
        plugin_events,
        plugins,
        track_info,
    } = deps;

    info!("control thread started");
    let mut state = EngineState::new();
    state.decode_worker = Some(start_decode_worker(
        Arc::clone(&events),
        internal_tx.clone(),
        Arc::clone(&plugins),
    ));
    state.preload_worker = Some(start_preload_worker(
        Arc::clone(&plugins),
        internal_tx.clone(),
    ));
    let tick = crossbeam_channel::tick(Duration::from_millis(CONTROL_TICK_MS));

    // Prewarm output spec in the background so the first Play doesn't pay the WASAPI/COM setup cost.
    ensure_output_spec_prewarm(&mut state, &internal_tx);

    loop {
        crossbeam_channel::select! {
            recv(cmd_rx) -> msg => {
                let Ok(cmd) = msg else { break };
                if handle_command(
                    cmd,
                    &mut state,
                    &events,
                    &plugin_events,
                    &internal_tx,
                    &plugins,
                    &track_info,
                ) {
                    break;
                }
            }
            recv(engine_ctrl_rx) -> msg => {
                let Ok(msg) = msg else { break };
                handle_engine_ctrl(msg, &mut state, &events, &plugins);
            }
            recv(internal_rx) -> msg => {
                let Ok(msg) = msg else { break };
                handle_internal(msg, &mut state, &events, &internal_tx, &track_info);
            }
            recv(tick) -> _ => {
                publish_player_tick_event(&state, &plugins);
                handle_tick(
                    &mut state,
                    &events,
                    &plugin_events,
                    &internal_tx,
                    &plugins,
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

fn clear_runtime_query_instance_cache(state: &mut EngineState) {
    state.source_instances.clear();
    state.lyrics_instances.clear();
    state.output_sink_instances.clear();
}

fn source_list_items_json_via_runtime(
    state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    config_json: String,
    request_json: String,
) -> Result<String, String> {
    let key = RuntimeInstanceKey::new(plugin_id, type_id, config_json.clone());
    if !state.source_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            service
                .create_source_catalog_instance(plugin_id, type_id, &config_json)
                .map_err(|e| e.to_string())
        })?;
        state.source_instances.insert(key.clone(), created);
    }
    let instance = state
        .source_instances
        .get_mut(&key)
        .ok_or_else(|| "source instance cache insertion failed".to_string())?;
    instance
        .list_items_json(&request_json)
        .map_err(|e| e.to_string())
}

fn lyrics_search_json_via_runtime(
    state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    query_json: String,
) -> Result<String, String> {
    let config_json =
        runtime_default_config_json(plugin_id, CapabilityKind::LyricsProvider, type_id)?;
    let key = RuntimeInstanceKey::new(plugin_id, type_id, config_json.clone());
    if !state.lyrics_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            service
                .create_lyrics_provider_instance(plugin_id, type_id, &config_json)
                .map_err(|e| e.to_string())
        })?;
        state.lyrics_instances.insert(key.clone(), created);
    }
    let instance = state
        .lyrics_instances
        .get_mut(&key)
        .ok_or_else(|| "lyrics instance cache insertion failed".to_string())?;
    instance.search_json(&query_json).map_err(|e| e.to_string())
}

fn lyrics_fetch_json_via_runtime(
    state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    track_json: String,
) -> Result<String, String> {
    let config_json =
        runtime_default_config_json(plugin_id, CapabilityKind::LyricsProvider, type_id)?;
    let key = RuntimeInstanceKey::new(plugin_id, type_id, config_json.clone());
    if !state.lyrics_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            service
                .create_lyrics_provider_instance(plugin_id, type_id, &config_json)
                .map_err(|e| e.to_string())
        })?;
        state.lyrics_instances.insert(key.clone(), created);
    }
    let instance = state
        .lyrics_instances
        .get_mut(&key)
        .ok_or_else(|| "lyrics instance cache insertion failed".to_string())?;
    instance.fetch_json(&track_json).map_err(|e| e.to_string())
}

fn output_sink_list_targets_json_via_runtime(
    state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    config_json: String,
) -> Result<String, String> {
    let key = RuntimeInstanceKey::new(plugin_id, type_id, config_json.clone());
    if !state.output_sink_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            service
                .create_output_sink_instance(plugin_id, type_id, &config_json)
                .map_err(|e| e.to_string())
        })?;
        state.output_sink_instances.insert(key.clone(), created);
    }
    let instance = state
        .output_sink_instances
        .get_mut(&key)
        .ok_or_else(|| "output sink instance cache insertion failed".to_string())?;
    instance.list_targets_json().map_err(|e| e.to_string())
}

fn handle_engine_ctrl(
    msg: EngineCtrl,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    plugins: &Arc<Mutex<PluginManager>>,
) {
    match msg {
        EngineCtrl::SetDspChain { chain } => {
            let parsed = match parse_dsp_chain(chain) {
                Ok(parsed) => parsed,
                Err(message) => {
                    events.emit(Event::Error { message });
                    return;
                }
            };
            state.desired_dsp_chain = parsed;
            if state.session.is_some()
                && let Err(message) = apply_dsp_chain(state)
            {
                events.emit(Event::Error { message });
            }
        }
        EngineCtrl::SourceListItemsJson {
            plugin_id,
            type_id,
            config_json,
            request_json,
            resp_tx,
        } => {
            let _ = resp_tx.send(source_list_items_json_via_runtime(
                state,
                &plugin_id,
                &type_id,
                config_json,
                request_json,
            ));
        }
        EngineCtrl::LyricsSearchJson {
            plugin_id,
            type_id,
            query_json,
            resp_tx,
        } => {
            let _ = resp_tx.send(lyrics_search_json_via_runtime(
                state, &plugin_id, &type_id, query_json,
            ));
        }
        EngineCtrl::LyricsFetchJson {
            plugin_id,
            type_id,
            track_json,
            resp_tx,
        } => {
            let _ = resp_tx.send(lyrics_fetch_json_via_runtime(
                state, &plugin_id, &type_id, track_json,
            ));
        }
        EngineCtrl::OutputSinkListTargetsJson {
            plugin_id,
            type_id,
            config_json,
            resp_tx,
        } => {
            let _ = resp_tx.send(output_sink_list_targets_json_via_runtime(
                state,
                &plugin_id,
                &type_id,
                config_json,
            ));
        }
        EngineCtrl::ReloadPlugins { dir } => {
            handle_reload_plugins(state, events, plugins, dir, Vec::new());
        }
        EngineCtrl::ReloadPluginsWithDisabled { dir, disabled_ids } => {
            handle_reload_plugins(state, events, plugins, dir, disabled_ids);
        }
        EngineCtrl::SetLfeMode { mode } => {
            state.lfe_mode = mode;
            if let Some(session) = state.session.as_ref() {
                let _ = session.ctrl_tx.send(DecodeCtrl::SetLfeMode { mode });
            }
        }
    }
}

fn handle_reload_plugins(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    plugins: &Arc<Mutex<PluginManager>>,
    dir: String,
    disabled_ids: Vec<String>,
) {
    // Transactional reload:
    // 1) Build a fresh manager and load plugins first.
    // 2) Swap to the new manager only if load produced usable plugins (or if there were none before).
    // This avoids ending up with an empty plugin set after a reload failure.
    let disabled = disabled_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    events.emit(Event::Log {
        message: format!(
            "plugin reload requested: dir={} disabled_count={}",
            dir,
            disabled.len()
        ),
    });
    clear_runtime_query_instance_cache(state);

    let mut next_pm = PluginManager::new(default_host_vtable());
    next_pm.set_disabled_ids(disabled.clone());
    let report = match unsafe { next_pm.load_dir_filtered(&dir, &disabled) } {
        Ok(report) => report,
        Err(e) => {
            events.emit(Event::Error {
                message: format!("failed to reload plugins (load stage): {e:#}"),
            });
            return;
        }
    };

    let loaded_count = report.loaded.len();
    let error_count = report.errors.len();
    let loaded_ids = report
        .loaded
        .iter()
        .map(|p| p.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let mut pm = match plugins.lock() {
        Ok(pm) => pm,
        Err(_) => {
            events.emit(Event::Error {
                message: "failed to reload plugins: plugins mutex poisoned".to_string(),
            });
            return;
        }
    };
    let prev_count = pm.plugins().len();

    if loaded_count == 0 && prev_count > 0 {
        events.emit(Event::Error {
            message: format!(
                "plugin reload aborted: loaded=0 errors={} (kept previous plugins={})",
                error_count, prev_count
            ),
        });
        for err in report.errors {
            events.emit(Event::Log {
                message: format!("plugin load error: {err:#}"),
            });
        }
        return;
    }

    *pm = next_pm;
    let runtime_report = match stellatune_plugins::v2::shared_runtime_service_v2().lock() {
        Ok(service) => service
            .reload_dir_filtered(&dir, &disabled)
            .map_err(|e| e.to_string()),
        Err(_) => Err("plugin runtime v2 mutex poisoned".to_string()),
    };
    events.emit(Event::Log {
        message: format!(
            "plugins reloaded from {}: previous={} loaded={} errors={} [{}]",
            dir, prev_count, loaded_count, error_count, loaded_ids
        ),
    });
    match runtime_report {
        Ok(v2) => events.emit(Event::Log {
            message: format!(
                "plugin runtime v2 reload: loaded={} deactivated={} errors={} unloaded_generations={}",
                v2.loaded.len(),
                v2.deactivated.len(),
                v2.errors.len(),
                v2.unloaded_generations
            ),
        }),
        Err(err) => events.emit(Event::Error {
            message: format!("plugin runtime v2 reload failed: {err}"),
        }),
    }
    for err in report.errors {
        events.emit(Event::Log {
            message: format!("plugin load error: {err:#}"),
        });
    }
}

fn handle_internal(
    msg: InternalMsg,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
    track_info: &SharedTrackInfo,
) {
    match msg {
        InternalMsg::Eof => {
            events.emit(Event::Log {
                message: "end of stream".to_string(),
            });
            if state.wants_playback
                && let Some(path) = state.current_track.clone()
            {
                events.emit(Event::PlaybackEnded {
                    path: event_path_from_engine_token(&path),
                });
            }
            stop_decode_session(state, track_info);
            state.wants_playback = false;
            state.play_request_started_at = None;
            set_state(state, events, PlayerState::Stopped);
        }
        InternalMsg::Error(message) => {
            events.emit(Event::Error { message });
            stop_decode_session(state, track_info);
            state.wants_playback = false;
            state.play_request_started_at = None;
            set_state(state, events, PlayerState::Stopped);
        }
        InternalMsg::OutputError(message) => {
            if state.session.is_none() {
                error!("output stream error (no active session): {message}");
                events.emit(Event::Log {
                    message: format!("output stream error (no active session): {message}"),
                });
                return;
            }

            error!("output stream error: {message}");
            events.emit(Event::Error {
                message: format!("output stream error: {message}"),
            });

            let Some(_path) = state.current_track.clone() else {
                stop_all_audio(state, track_info);
                state.wants_playback = false;
                set_state(state, events, PlayerState::Stopped);
                return;
            };

            let prev_state = state.player_state;
            stop_decode_session(state, track_info);
            drop_output_pipeline(state);

            // Force refresh output spec (device may have changed sample rate).
            state.cached_output_spec = None;
            ensure_output_spec_prewarm(state, internal_tx);
            state.pending_session_start =
                prev_state == PlayerState::Playing || prev_state == PlayerState::Buffering;

            match prev_state {
                PlayerState::Playing | PlayerState::Buffering => {
                    state.wants_playback = true;
                    state.play_request_started_at = Some(Instant::now());
                    set_state(state, events, PlayerState::Buffering);
                }
                PlayerState::Paused => {
                    state.wants_playback = false;
                    state.play_request_started_at = None;
                    set_state(state, events, PlayerState::Paused);
                }
                PlayerState::Stopped => {
                    state.wants_playback = false;
                    state.play_request_started_at = None;
                    set_state(state, events, PlayerState::Stopped);
                }
            }

            events.emit(Event::Log {
                message: "output error: scheduled session restart".to_string(),
            });
        }
        InternalMsg::Position(ms) => {
            state.position_ms = ms;
        }
        InternalMsg::OutputSpecReady {
            spec,
            took_ms,
            token,
        } => {
            if token != state.output_spec_token {
                return;
            }
            state.cached_output_spec = Some(spec);
            state.output_spec_prewarm_inflight = false;
            debug!(
                "output_spec prewarm ready in {}ms: {}Hz {}ch",
                took_ms, spec.sample_rate, spec.channels
            );
        }
        InternalMsg::OutputSpecFailed {
            message,
            took_ms,
            token,
        } => {
            if token != state.output_spec_token {
                return;
            }
            state.cached_output_spec = None;
            state.output_spec_prewarm_inflight = false;
            warn!("output_spec prewarm failed in {}ms: {}", took_ms, message);
            if state.wants_playback && state.session.is_none() {
                state.pending_session_start = false;
                state.wants_playback = false;
                state.play_request_started_at = None;
                events.emit(Event::Error {
                    message: format!("failed to query output device: {message}"),
                });
                set_state(state, events, PlayerState::Stopped);
            }
        }
        InternalMsg::PreloadReady {
            path,
            position_ms,
            decoder,
            track_info,
            chunk,
            took_ms,
            token,
        } => {
            if token != state.preload_token {
                return;
            }
            if state.requested_preload_path.as_deref() != Some(path.as_str()) {
                return;
            }
            if state.requested_preload_position_ms != position_ms {
                return;
            }
            debug_metrics::note_preload_result(true, took_ms);
            if let Some(worker) = state.decode_worker.as_ref() {
                worker.promote_preload(PromotedPreload {
                    path: path.clone(),
                    position_ms,
                    decoder,
                    track_info,
                    chunk,
                });
            }
            debug_metrics::maybe_log_preload_stats();
            debug!(%path, position_ms, took_ms, "preload cached");
        }
        InternalMsg::PreloadFailed {
            path,
            position_ms,
            message,
            took_ms,
            token,
        } => {
            if token != state.preload_token {
                return;
            }
            if state.requested_preload_path.as_deref() != Some(path.as_str()) {
                return;
            }
            if state.requested_preload_position_ms != position_ms {
                return;
            }
            debug_metrics::note_preload_result(false, took_ms);
            debug_metrics::maybe_log_preload_stats();
            debug!(%path, position_ms, took_ms, "preload failed: {message}");
        }
    }
}

fn handle_command(
    cmd: Command,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    plugin_events: &Arc<PluginEventHub>,
    internal_tx: &Sender<InternalMsg>,
    plugins: &Arc<Mutex<PluginManager>>,
    track_info: &SharedTrackInfo,
) -> bool {
    match cmd {
        Command::LoadTrack { path } => {
            maybe_fade_out_before_disrupt(state);
            stop_decode_session(state, track_info);
            state.current_track = Some(path.clone());
            state.position_ms = 0;
            state.wants_playback = false;
            state.pending_session_start = false;
            state.play_request_started_at = None;
            track_info.store(None);
            events.emit(Event::TrackChanged { path });
            events.emit(Event::Position {
                ms: state.position_ms,
            });
            set_state(state, events, PlayerState::Stopped);
        }
        Command::LoadTrackRef { track } => {
            let Some(path) = track_ref_to_engine_token(&track) else {
                events.emit(Event::Error {
                    message: "track locator is empty".to_string(),
                });
                return false;
            };
            let Some(event_path) = track_ref_to_event_path(&track) else {
                events.emit(Event::Error {
                    message: "track locator is empty".to_string(),
                });
                return false;
            };
            maybe_fade_out_before_disrupt(state);
            stop_decode_session(state, track_info);
            state.current_track = Some(path.clone());
            state.position_ms = 0;
            state.wants_playback = false;
            state.pending_session_start = false;
            state.play_request_started_at = None;
            track_info.store(None);
            events.emit(Event::TrackChanged { path: event_path });
            events.emit(Event::Position {
                ms: state.position_ms,
            });
            set_state(state, events, PlayerState::Stopped);
        }
        Command::Play => {
            let Some(path) = state.current_track.clone() else {
                events.emit(Event::Error {
                    message: "no track loaded".to_string(),
                });
                return false;
            };

            state.wants_playback = true;
            state.play_request_started_at = Some(Instant::now());

            if state.session.is_none() {
                set_state(state, events, PlayerState::Buffering);
                if let Some(cached_out_spec) = state.cached_output_spec {
                    let out_spec = match resolve_output_spec_and_sink_chunk(state, cached_out_spec)
                    {
                        Ok(spec) => spec,
                        Err(message) => {
                            events.emit(Event::Error { message });
                            set_state(state, events, PlayerState::Stopped);
                            state.wants_playback = false;
                            state.pending_session_start = false;
                            state.play_request_started_at = None;
                            return false;
                        }
                    };
                    let start_at_ms = state.position_ms.max(0) as u64;
                    let Some(decode_worker) = state.decode_worker.as_ref() else {
                        events.emit(Event::Error {
                            message: "decode worker unavailable".to_string(),
                        });
                        set_state(state, events, PlayerState::Stopped);
                        state.wants_playback = false;
                        state.pending_session_start = false;
                        state.play_request_started_at = None;
                        return false;
                    };
                    let backend = output_backend_for_selected(state.selected_backend);
                    match start_session(StartSessionArgs {
                        path,
                        decode_worker,
                        internal_tx: internal_tx.clone(),
                        backend,
                        device_id: state.selected_device_id.clone(),
                        match_track_sample_rate: state.match_track_sample_rate,
                        gapless_playback: state.gapless_playback,
                        out_spec,
                        start_at_ms: start_at_ms as i64,
                        volume: Arc::clone(&state.volume_atomic),
                        lfe_mode: state.lfe_mode,
                        output_sink_chunk_frames: state.output_sink_chunk_frames,
                        output_sink_only: state.desired_output_sink_route.is_some(),
                        output_pipeline: &mut state.output_pipeline,
                    }) {
                        Ok(session) => {
                            track_info.store(Some(Arc::new(session.track_info.clone())));
                            state.session = Some(session);
                            if let Err(message) =
                                sync_output_sink_with_active_session(state, internal_tx)
                            {
                                events.emit(Event::Error { message });
                            }
                            if let Err(message) = apply_dsp_chain(state) {
                                events.emit(Event::Error { message });
                            }
                        }
                        Err(message) => {
                            events.emit(Event::Error { message });
                            set_state(state, events, PlayerState::Stopped);
                            state.wants_playback = false;
                            state.pending_session_start = false;
                            state.play_request_started_at = None;
                            return false;
                        }
                    }
                } else {
                    state.pending_session_start = true;
                    ensure_output_spec_prewarm(state, internal_tx);
                    // Wait for OutputSpecReady then start the session in `handle_tick`.
                    return false;
                }
            }

            if let Some(session) = state.session.as_ref() {
                if state.seek_track_fade {
                    session
                        .transition_gain
                        .store(0.0f32.to_bits(), Ordering::Relaxed);
                    session
                        .transition_target_gain
                        .store(0.0f32.to_bits(), Ordering::Relaxed);
                } else {
                    force_transition_gain_unity(Some(session));
                }
                session.output_enabled.store(false, Ordering::Release);
                let _ = session.ctrl_tx.send(DecodeCtrl::Play);
            }

            // Enter Buffering until we have enough samples queued to start cleanly.
            set_state(state, events, PlayerState::Buffering);
            handle_tick(
                state,
                events,
                plugin_events,
                internal_tx,
                plugins,
                track_info,
            );
        }
        Command::Pause => {
            if let Some(session) = state.session.as_ref() {
                maybe_fade_out_before_disrupt(state);
                session.output_enabled.store(false, Ordering::Release);
                let _ = session.ctrl_tx.send(DecodeCtrl::Pause);
            }
            state.wants_playback = false;
            state.play_request_started_at = None;
            state.pending_session_start = false;
            set_state(state, events, PlayerState::Paused);
        }
        Command::SeekMs { position_ms } => {
            let Some(_path) = state.current_track.clone() else {
                events.emit(Event::Error {
                    message: "no track loaded".to_string(),
                });
                return false;
            };

            maybe_fade_out_before_disrupt(state);
            state.position_ms = (position_ms as i64).max(0);
            events.emit(Event::Position {
                ms: state.position_ms,
            });

            // If a session exists, ask the decode thread to seek in-place and flush buffered audio.
            if let Some(session) = state.session.as_ref() {
                session.output_enabled.store(false, Ordering::Release);
                let _ = session.ctrl_tx.send(DecodeCtrl::SeekMs {
                    position_ms: state.position_ms,
                });
            }

            // If we are actively playing, re-enter buffering so the output resumes cleanly once
            // enough samples are queued after seek.
            if state.wants_playback
                && matches!(
                    state.player_state,
                    PlayerState::Playing | PlayerState::Buffering
                )
            {
                set_state(state, events, PlayerState::Buffering);
                state.play_request_started_at = Some(Instant::now());
                handle_tick(
                    state,
                    events,
                    plugin_events,
                    internal_tx,
                    plugins,
                    track_info,
                );
            }
        }
        Command::SetVolume { volume } => {
            // UI volume is linear [0, 1], but perceived loudness is roughly logarithmic. Map to a
            // gain curve so the slider feels more even across its range.
            let ui = volume.clamp(0.0, 1.0);
            let gain = ui_volume_to_gain(ui);
            state.volume = ui;
            state.volume_atomic.store(gain.to_bits(), Ordering::Relaxed);
            // Emit UI volume so Flutter keeps the slider position stable.
            events.emit(Event::VolumeChanged { volume: ui });
        }
        Command::SetLfeMode { mode } => {
            state.lfe_mode = mode;
            if let Some(session) = state.session.as_ref() {
                let _ = session.ctrl_tx.send(DecodeCtrl::SetLfeMode { mode });
            }
        }
        Command::Stop => {
            stop_decode_session(state, track_info);
            state.position_ms = 0;
            state.wants_playback = false;
            state.play_request_started_at = None;
            state.pending_session_start = false;
            events.emit(Event::Position {
                ms: state.position_ms,
            });
            set_state(state, events, PlayerState::Stopped);
        }
        Command::SetOutputDevice { backend, device_id } => {
            state.selected_backend = backend;
            state.selected_device_id = device_id;
            // Output spec depends on device/backend (sample rate/channels). Refresh it.
            state.cached_output_spec = None;
            state.output_spec_prewarm_inflight = false;
            state.output_spec_token = state.output_spec_token.wrapping_add(1);
            ensure_output_spec_prewarm(state, internal_tx);
            if state.session.is_some() {
                stop_decode_session(state, track_info);
            }
            drop_output_pipeline(state);
            if state.wants_playback {
                state.pending_session_start = true;
            }
        }
        Command::SetOutputOptions {
            match_track_sample_rate,
            gapless_playback,
            seek_track_fade,
        } => {
            if !seek_track_fade {
                force_transition_gain_unity(state.session.as_ref());
            }
            state.seek_track_fade = seek_track_fade;

            let changed = state.match_track_sample_rate != match_track_sample_rate
                || state.gapless_playback != gapless_playback;
            if changed {
                state.match_track_sample_rate = match_track_sample_rate;
                state.gapless_playback = gapless_playback;
                if state.session.is_some() {
                    stop_decode_session(state, track_info);
                    if state.wants_playback {
                        state.pending_session_start = true;
                    }
                }
                if !state.gapless_playback {
                    drop_output_pipeline(state);
                }
            }
        }
        Command::SetOutputSinkRoute { route } => {
            let parsed_route = match parse_output_sink_route(route) {
                Ok(route) => route,
                Err(message) => {
                    events.emit(Event::Error { message });
                    return false;
                }
            };
            let mode_changed = state.desired_output_sink_route.is_none();
            let route_changed = state.desired_output_sink_route.as_ref() != Some(&parsed_route);
            state.desired_output_sink_route = Some(parsed_route);
            if mode_changed || route_changed {
                state.output_sink_chunk_frames = 0;
                state.cached_output_spec = None;
                state.output_spec_prewarm_inflight = false;
                state.output_spec_token = state.output_spec_token.wrapping_add(1);
                ensure_output_spec_prewarm(state, internal_tx);
                let resume_playback = state.wants_playback;
                if state.session.is_some() {
                    stop_decode_session(state, track_info);
                    drop_output_pipeline(state);
                }
                if resume_playback {
                    state.pending_session_start = true;
                    set_state(state, events, PlayerState::Buffering);
                }
            }
            if let Err(message) = sync_output_sink_with_active_session(state, internal_tx) {
                events.emit(Event::Error { message });
            }
        }
        Command::ClearOutputSinkRoute => {
            let mode_changed = state.desired_output_sink_route.is_some();
            state.desired_output_sink_route = None;
            state.output_sink_chunk_frames = 0;
            if mode_changed {
                state.cached_output_spec = None;
                state.output_spec_prewarm_inflight = false;
                state.output_spec_token = state.output_spec_token.wrapping_add(1);
                ensure_output_spec_prewarm(state, internal_tx);
                let resume_playback = state.wants_playback;
                if state.session.is_some() {
                    stop_decode_session(state, track_info);
                    drop_output_pipeline(state);
                }
                if resume_playback {
                    state.pending_session_start = true;
                    set_state(state, events, PlayerState::Buffering);
                }
            }
            if let Err(message) = sync_output_sink_with_active_session(state, internal_tx) {
                events.emit(Event::Error { message });
            }
        }
        Command::PreloadTrack { path, position_ms } => {
            let path = path.trim().to_string();
            if path.is_empty() {
                return false;
            }
            if state.requested_preload_path.as_deref() == Some(path.as_str())
                && state.requested_preload_position_ms == position_ms
            {
                return false;
            }
            state.requested_preload_path = Some(path.clone());
            state.requested_preload_position_ms = position_ms;
            state.preload_token = state.preload_token.wrapping_add(1);
            debug_metrics::note_preload_request();
            enqueue_preload_task(state, path, position_ms, state.preload_token);
        }
        Command::PreloadTrackRef { track, position_ms } => {
            let Some(path) = track_ref_to_engine_token(&track) else {
                return false;
            };
            if state.requested_preload_path.as_deref() == Some(path.as_str())
                && state.requested_preload_position_ms == position_ms
            {
                return false;
            }
            state.requested_preload_path = Some(path.clone());
            state.requested_preload_position_ms = position_ms;
            state.preload_token = state.preload_token.wrapping_add(1);
            debug_metrics::note_preload_request();
            enqueue_preload_task(state, path, position_ms, state.preload_token);
        }
        Command::RefreshDevices => {
            let selected_backend = output_backend_for_selected(state.selected_backend);
            let devices = stellatune_output::list_host_devices(Some(selected_backend))
                .into_iter()
                .map(|d| stellatune_core::AudioDevice {
                    backend: match d.backend {
                        stellatune_output::AudioBackend::Shared => {
                            stellatune_core::AudioBackend::Shared
                        }
                        stellatune_output::AudioBackend::WasapiExclusive => {
                            stellatune_core::AudioBackend::WasapiExclusive
                        }
                    },
                    id: d.id,
                    name: d.name,
                })
                .collect();
            events.emit(Event::OutputDevicesChanged { devices });
        }
        Command::Shutdown => {
            stop_all_audio(state, track_info);
            state.wants_playback = false;
            state.play_request_started_at = None;
            state.pending_session_start = false;
            return true;
        }
    }

    false
}

fn start_preload_worker(
    plugins: Arc<Mutex<PluginManager>>,
    internal_tx: Sender<InternalMsg>,
) -> PreloadWorker {
    let (tx, rx) = crossbeam_channel::unbounded::<PreloadJob>();
    let join = thread::Builder::new()
        .name("stellatune-preload-next".to_string())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                match job {
                    PreloadJob::Task {
                        path,
                        position_ms,
                        token,
                    } => handle_preload_task(path, position_ms, token, &plugins, &internal_tx),
                    PreloadJob::Shutdown => break,
                }
            }
        })
        .expect("failed to spawn stellatune-preload-next thread");
    PreloadWorker { tx, join }
}

fn enqueue_preload_task(state: &mut EngineState, path: String, position_ms: u64, token: u64) {
    let Some(worker) = state.preload_worker.as_ref() else {
        return;
    };
    let _ = worker.tx.send(PreloadJob::Task {
        path,
        position_ms,
        token,
    });
}

fn handle_preload_task(
    path: String,
    position_ms: u64,
    token: u64,
    plugins: &Arc<Mutex<PluginManager>>,
    internal_tx: &Sender<InternalMsg>,
) {
    let t0 = Instant::now();
    match open_engine_decoder(&path, plugins) {
        Ok((mut decoder, track_info)) => {
            if position_ms > 0
                && let Err(err) = decoder.seek_ms(position_ms)
            {
                let _ = internal_tx.send(InternalMsg::PreloadFailed {
                    path: path.clone(),
                    position_ms,
                    message: err,
                    took_ms: t0.elapsed().as_millis() as u64,
                    token,
                });
                return;
            }
            match decoder.next_block(2048) {
                Ok(Some(samples)) if !samples.is_empty() => {
                    let _ = internal_tx.send(InternalMsg::PreloadReady {
                        path: path.clone(),
                        position_ms,
                        decoder,
                        track_info: track_info.clone(),
                        chunk: PredecodedChunk {
                            samples,
                            sample_rate: track_info.sample_rate,
                            channels: track_info.channels,
                            start_at_ms: position_ms,
                        },
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
                Ok(_) => {
                    let _ = internal_tx.send(InternalMsg::PreloadFailed {
                        path: path.clone(),
                        position_ms,
                        message: "decoder returned no preload audio".to_string(),
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
                Err(err) => {
                    let _ = internal_tx.send(InternalMsg::PreloadFailed {
                        path: path.clone(),
                        position_ms,
                        message: err,
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
            }
        }
        Err(err) => {
            let _ = internal_tx.send(InternalMsg::PreloadFailed {
                path: path.clone(),
                position_ms,
                message: err,
                took_ms: t0.elapsed().as_millis() as u64,
                token,
            });
        }
    }
}

fn engine_token_to_track_ref(token: &str) -> Option<stellatune_core::TrackRef> {
    let json = token.strip_prefix(TRACK_REF_TOKEN_PREFIX)?;
    serde_json::from_str::<stellatune_core::TrackRef>(json).ok()
}

fn event_path_from_engine_token(token: &str) -> String {
    match engine_token_to_track_ref(token) {
        Some(track) => track.locator,
        None => token.to_string(),
    }
}

fn track_ref_to_event_path(track: &stellatune_core::TrackRef) -> Option<String> {
    let locator = track.locator.trim();
    if locator.is_empty() {
        None
    } else {
        Some(locator.to_string())
    }
}

fn track_ref_to_engine_token(track: &stellatune_core::TrackRef) -> Option<String> {
    if track.source_id.trim().eq_ignore_ascii_case("local") {
        return track_ref_to_event_path(track);
    }
    let json = serde_json::to_string(track).ok()?;
    Some(format!("{TRACK_REF_TOKEN_PREFIX}{json}"))
}

fn ui_volume_to_gain(ui: f32) -> f32 {
    // 0 maps to true mute.
    if ui <= 0.0 {
        return 0.0;
    }
    // Use a dB curve for perceived loudness. Range chosen to keep low volumes usable without
    // making the first half of the slider effectively silent.
    const MIN_DB: f32 = -30.0;
    let db = MIN_DB * (1.0 - ui);
    10.0_f32.powf(db / 20.0)
}

fn maybe_fade_out_before_disrupt(state: &EngineState) {
    if !state.seek_track_fade || state.player_state != PlayerState::Playing {
        return;
    }
    let Some(session) = state.session.as_ref() else {
        return;
    };
    session
        .transition_target_gain
        .store(0.0f32.to_bits(), Ordering::Relaxed);
    let started = Instant::now();
    while started.elapsed().as_millis() < SEEK_TRACK_FADE_WAIT_TIMEOUT_MS as u128 {
        let current = f32::from_bits(session.transition_gain.load(Ordering::Relaxed));
        if current <= 0.05 {
            break;
        }
        thread::sleep(Duration::from_millis(SEEK_TRACK_FADE_WAIT_POLL_MS));
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
    state.player_state = new_state;
    events.emit(Event::StateChanged { state: new_state });
}

fn ensure_output_spec_prewarm(state: &mut EngineState, internal_tx: &Sender<InternalMsg>) {
    if state.cached_output_spec.is_some() || state.output_spec_prewarm_inflight {
        return;
    }

    if state.desired_output_sink_route.is_some() {
        let spec = output_spec_for_plugin_sink(state);
        state.cached_output_spec = Some(spec);
        state.output_spec_prewarm_inflight = false;
        debug!(
            "output_spec prewarm bypassed for plugin sink: {}Hz {}ch",
            spec.sample_rate, spec.channels
        );
        return;
    }

    state.output_spec_prewarm_inflight = true;
    let token = state.output_spec_token;
    let backend = output_backend_for_selected(state.selected_backend);
    let device_id = state.selected_device_id.clone();
    let tx = internal_tx.clone();
    thread::Builder::new()
        .name("stellatune-output-spec".to_string())
        .spawn(move || {
            let t0 = Instant::now();
            match output_spec_for_device(backend, device_id) {
                Ok(spec) => {
                    let _ = tx.send(InternalMsg::OutputSpecReady {
                        spec,
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
                Err(e) => {
                    let _ = tx.send(InternalMsg::OutputSpecFailed {
                        message: e.to_string(),
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
            }
        })
        .expect("failed to spawn stellatune-output-spec thread");
}

fn output_spec_for_plugin_sink(state: &EngineState) -> OutputSpec {
    let start_at_ms = state.position_ms.max(0) as u64;
    if let (Some(path), Some(worker)) =
        (state.current_track.as_deref(), state.decode_worker.as_ref())
        && let Some(track_info) = worker.peek_promoted_track_info(path, start_at_ms)
    {
        return OutputSpec {
            sample_rate: track_info.sample_rate.max(1),
            channels: track_info.channels.max(1),
        };
    }

    OutputSpec {
        sample_rate: PLUGIN_SINK_FALLBACK_SAMPLE_RATE,
        channels: PLUGIN_SINK_FALLBACK_CHANNELS,
    }
}

fn output_sink_queue_watermarks_ms(sample_rate: u32, chunk_frames: u32) -> (i64, i64) {
    let frames_per_chunk = if chunk_frames == 0 {
        PLUGIN_SINK_DEFAULT_CHUNK_FRAMES as u64
    } else {
        chunk_frames as u64
    };
    let capacity_frames = frames_per_chunk
        .saturating_mul(OUTPUT_SINK_QUEUE_CAP_MESSAGES as u64)
        .max(1);
    let sample_rate = sample_rate.max(1) as u64;
    let capacity_ms = ((capacity_frames.saturating_mul(1000)) / sample_rate)
        .max(PLUGIN_SINK_MIN_LOW_WATERMARK_MS as u64);
    let high = ((capacity_ms.saturating_mul(3)) / 4)
        .max(PLUGIN_SINK_MIN_HIGH_WATERMARK_MS as u64)
        .min(capacity_ms) as i64;
    let low = (high / 2).max(PLUGIN_SINK_MIN_LOW_WATERMARK_MS);
    (low.min(high.saturating_sub(1)), high)
}

fn negotiate_output_sink_spec(
    state: &EngineState,
    desired_spec: OutputSpec,
) -> Result<StOutputSinkNegotiatedSpecV2, String> {
    let route = state
        .desired_output_sink_route
        .as_ref()
        .ok_or_else(|| "output sink route not configured".to_string())?;
    let config_json = serde_json::to_string(&route.config)
        .map_err(|e| format!("invalid output sink config json: {e}"))?;
    let target_json = serde_json::to_string(&route.target)
        .map_err(|e| format!("invalid output sink target json: {e}"))?;
    with_runtime_service(|service| {
        let mut sink = service
            .create_output_sink_instance(&route.plugin_id, &route.type_id, &config_json)
            .map_err(|e| format!("output sink create failed: {e}"))?;
        sink.negotiate_spec(
            &target_json,
            StAudioSpec {
                sample_rate: desired_spec.sample_rate.max(1),
                channels: desired_spec.channels.max(1),
                reserved: 0,
            },
        )
        .map_err(|e| format!("output sink negotiate failed: {e}"))
    })
}

fn resolve_output_spec_and_sink_chunk(
    state: &mut EngineState,
    non_sink_out_spec: OutputSpec,
) -> Result<OutputSpec, String> {
    if state.desired_output_sink_route.is_none() {
        state.output_sink_chunk_frames = 0;
        return Ok(non_sink_out_spec);
    }

    state.output_sink_chunk_frames = 0;
    let desired_spec = output_spec_for_plugin_sink(state);
    let negotiated = negotiate_output_sink_spec(state, desired_spec)?;
    state.output_sink_chunk_frames = negotiated.preferred_chunk_frames;
    Ok(OutputSpec {
        sample_rate: negotiated.spec.sample_rate.max(1),
        channels: negotiated.spec.channels.max(1),
    })
}

fn output_backend_for_selected(
    backend: stellatune_core::AudioBackend,
) -> stellatune_output::AudioBackend {
    match backend {
        stellatune_core::AudioBackend::Shared => stellatune_output::AudioBackend::Shared,
        stellatune_core::AudioBackend::WasapiExclusive => {
            stellatune_output::AudioBackend::WasapiExclusive
        }
    }
}

fn publish_player_tick_event(state: &EngineState, plugins: &Arc<Mutex<PluginManager>>) {
    let event_json = match serde_json::to_string(&HostPlayerTickPayload {
        topic: HostEventTopic::PlayerTick,
        state: state.player_state,
        position_ms: state.position_ms,
        track: state.current_track.clone(),
        wants_playback: state.wants_playback,
    }) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Ok(pm) = plugins.lock() {
        pm.broadcast_host_event_json(&event_json);
    }
}

fn handle_tick(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    _plugin_events: &Arc<PluginEventHub>,
    internal_tx: &Sender<InternalMsg>,
    _plugins: &Arc<Mutex<PluginManager>>,
    track_info: &SharedTrackInfo,
) {
    // If we are waiting for an output spec (prewarm) and have no active session, start the session
    // as soon as the spec becomes available.
    if state.session.is_none()
        && state.wants_playback
        && state.pending_session_start
        && state.cached_output_spec.is_some()
    {
        let Some(path) = state.current_track.clone() else {
            state.pending_session_start = false;
            state.wants_playback = false;
            state.play_request_started_at = None;
            set_state(state, events, PlayerState::Stopped);
            return;
        };
        let Some(cached_out_spec) = state.cached_output_spec else {
            state.pending_session_start = false;
            state.wants_playback = false;
            state.play_request_started_at = None;
            events.emit(Event::Error {
                message: "output spec missing while pending session start".to_string(),
            });
            set_state(state, events, PlayerState::Stopped);
            return;
        };
        let out_spec = match resolve_output_spec_and_sink_chunk(state, cached_out_spec) {
            Ok(spec) => spec,
            Err(message) => {
                state.pending_session_start = false;
                state.wants_playback = false;
                state.play_request_started_at = None;
                events.emit(Event::Error { message });
                set_state(state, events, PlayerState::Stopped);
                return;
            }
        };
        let start_at_ms = state.position_ms.max(0) as u64;
        let Some(decode_worker) = state.decode_worker.as_ref() else {
            state.pending_session_start = false;
            state.wants_playback = false;
            state.play_request_started_at = None;
            events.emit(Event::Error {
                message: "decode worker unavailable".to_string(),
            });
            set_state(state, events, PlayerState::Stopped);
            return;
        };
        let backend = output_backend_for_selected(state.selected_backend);
        match start_session(StartSessionArgs {
            path,
            decode_worker,
            internal_tx: internal_tx.clone(),
            backend,
            device_id: state.selected_device_id.clone(),
            match_track_sample_rate: state.match_track_sample_rate,
            gapless_playback: state.gapless_playback,
            out_spec,
            start_at_ms: start_at_ms as i64,
            volume: Arc::clone(&state.volume_atomic),
            lfe_mode: state.lfe_mode,
            output_sink_chunk_frames: state.output_sink_chunk_frames,
            output_sink_only: state.desired_output_sink_route.is_some(),
            output_pipeline: &mut state.output_pipeline,
        }) {
            Ok(session) => {
                track_info.store(Some(Arc::new(session.track_info.clone())));
                state.session = Some(session);
                state.pending_session_start = false;
                if let Err(message) = sync_output_sink_with_active_session(state, internal_tx) {
                    events.emit(Event::Error { message });
                }
                if let Err(message) = apply_dsp_chain(state) {
                    events.emit(Event::Error { message });
                }
                if let Some(session) = state.session.as_ref() {
                    let _ = session.ctrl_tx.send(DecodeCtrl::Play);
                }
                set_state(state, events, PlayerState::Buffering);
            }
            Err(message) => {
                state.pending_session_start = false;
                state.wants_playback = false;
                state.play_request_started_at = None;
                events.emit(Event::Error { message });
                set_state(state, events, PlayerState::Stopped);
            }
        }
    }

    let Some(session) = state.session.as_ref() else {
        return;
    };

    if state.desired_output_sink_route.is_some() {
        session.output_enabled.store(false, Ordering::Release);
        if !state.wants_playback {
            set_state(state, events, PlayerState::Paused);
            return;
        }

        let channels = session.out_channels as usize;
        if channels == 0 {
            return;
        }
        let pending_samples = state
            .output_sink_worker
            .as_ref()
            .map(|worker| worker.pending_samples())
            .unwrap_or(0);
        let pending_frames = pending_samples / channels;
        let buffered_ms =
            ((pending_frames as u64 * 1000) / session.out_sample_rate.max(1) as u64) as i64;
        let (low_watermark_ms, high_watermark_ms) = output_sink_queue_watermarks_ms(
            session.out_sample_rate,
            state.output_sink_chunk_frames,
        );

        match state.player_state {
            PlayerState::Playing => {
                if buffered_ms <= low_watermark_ms {
                    set_state(state, events, PlayerState::Buffering);
                }
            }
            PlayerState::Buffering => {
                if buffered_ms >= high_watermark_ms {
                    if state.seek_track_fade {
                        session
                            .transition_target_gain
                            .store(1.0f32.to_bits(), Ordering::Relaxed);
                    } else {
                        force_transition_gain_unity(Some(session));
                    }
                    state.play_request_started_at = None;
                    set_state(state, events, PlayerState::Playing);
                }
            }
            PlayerState::Paused | PlayerState::Stopped => {}
        }
        return;
    }

    let channels = session.out_channels as usize;
    if channels == 0 {
        return;
    }

    let buffered_samples = session.buffered_samples.load(Ordering::Relaxed);
    let buffered_frames = buffered_samples / channels;
    let buffered_ms =
        ((buffered_frames as u64 * 1000) / session.out_sample_rate.max(1) as u64) as i64;

    let underruns = session.underrun_callbacks.load(Ordering::Relaxed);
    if underruns > state.last_underrun_total
        && state.last_underrun_log_at.elapsed() >= UNDERRUN_LOG_INTERVAL
    {
        let delta = underruns - state.last_underrun_total;
        state.last_underrun_total = underruns;
        state.last_underrun_log_at = Instant::now();
        events.emit(Event::Log {
            message: format!("audio underrun callbacks: total={underruns}, +{delta}"),
        });
    }

    if !state.wants_playback {
        session.output_enabled.store(false, Ordering::Release);
        return;
    }

    let (low_watermark_ms, high_watermark_ms) = match state.selected_backend {
        stellatune_core::AudioBackend::WasapiExclusive => (
            BUFFER_LOW_WATERMARK_MS_EXCLUSIVE,
            BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE,
        ),
        _ => (BUFFER_LOW_WATERMARK_MS, BUFFER_HIGH_WATERMARK_MS),
    };

    match state.player_state {
        PlayerState::Playing => {
            if buffered_ms <= low_watermark_ms {
                session.output_enabled.store(false, Ordering::Release);
                set_state(state, events, PlayerState::Buffering);
                debug!("buffer low watermark reached: buffered_ms={buffered_ms}");
            } else {
                session.output_enabled.store(true, Ordering::Release);
            }
        }
        PlayerState::Buffering => {
            if buffered_ms >= high_watermark_ms {
                session.output_enabled.store(true, Ordering::Release);
                if state.seek_track_fade {
                    session
                        .transition_target_gain
                        .store(1.0f32.to_bits(), Ordering::Relaxed);
                } else {
                    force_transition_gain_unity(Some(session));
                }
                set_state(state, events, PlayerState::Playing);
                let elapsed_ms = state
                    .play_request_started_at
                    .take()
                    .map(|t0| t0.elapsed().as_millis() as u64);
                debug!(buffered_ms, elapsed_ms = ?elapsed_ms, "buffering completed");
            } else {
                session.output_enabled.store(false, Ordering::Release);
            }
        }
        PlayerState::Paused | PlayerState::Stopped => {
            session.output_enabled.store(false, Ordering::Release);
        }
    }
}

fn apply_dsp_chain(state: &mut EngineState) -> Result<(), String> {
    let Some(session) = state.session.as_ref() else {
        return Ok(());
    };

    let chain_spec = state.desired_dsp_chain.clone();
    if chain_spec.is_empty() {
        let _ = session
            .ctrl_tx
            .send(DecodeCtrl::SetDspChain { chain: Vec::new() });
        return Ok(());
    }

    let chain = with_runtime_service(|service| {
        let mut chain = Vec::with_capacity(chain_spec.len());
        for item in &chain_spec {
            let config_json = serde_json::to_string(&item.config).map_err(|e| {
                format!(
                    "invalid DSP config json for {}::{}: {e}",
                    item.plugin_id, item.type_id
                )
            })?;
            let inst = service
                .create_dsp_instance(
                    &item.plugin_id,
                    &item.type_id,
                    session.out_sample_rate,
                    session.out_channels,
                    &config_json,
                )
                .map_err(|e| {
                    format!(
                        "failed to create DSP {}::{}: {e}",
                        item.plugin_id, item.type_id
                    )
                })?;
            chain.push(inst);
        }
        Ok(chain)
    })?;

    let _ = session.ctrl_tx.send(DecodeCtrl::SetDspChain { chain });
    Ok(())
}

fn open_output_sink_worker(args: OpenOutputSinkWorkerArgs<'_>) -> Result<OutputSinkWorker, String> {
    let config_json = serde_json::to_string(&args.route.config)
        .map_err(|e| format!("invalid output sink config json: {e}"))?;
    let target_json = serde_json::to_string(&args.route.target)
        .map_err(|e| format!("invalid output sink target json: {e}"))?;
    let mut sink = with_runtime_service(|service| {
        service
            .create_output_sink_instance(&args.route.plugin_id, &args.route.type_id, &config_json)
            .map_err(|e| format!("output sink create failed: {e}"))
    })?;
    sink.open(
        &target_json,
        StAudioSpec {
            sample_rate: args.sample_rate,
            channels: args.channels,
            reserved: 0,
        },
    )
    .map_err(|e| format!("output sink open failed: {e}"))?;
    Ok(OutputSinkWorker::start(
        sink,
        args.channels,
        args.sample_rate,
        args.volume,
        args.transition_gain,
        args.transition_target_gain,
        args.internal_tx.clone(),
    ))
}

fn sync_output_sink_with_active_session(
    state: &mut EngineState,
    internal_tx: &Sender<InternalMsg>,
) -> Result<(), String> {
    let Some(session) = state.session.as_ref() else {
        shutdown_output_sink_worker(state);
        return Ok(());
    };
    let ctrl_tx = session.ctrl_tx.clone();
    let sample_rate = session.out_sample_rate;
    let channels = session.out_channels;
    let transition_gain = Arc::clone(&session.transition_gain);
    let transition_target_gain = Arc::clone(&session.transition_target_gain);
    let desired_route = state.desired_output_sink_route.clone();
    let Some(route) = desired_route else {
        let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
            tx: None,
            output_sink_chunk_frames: 0,
        });
        shutdown_output_sink_worker(state);
        return Ok(());
    };
    let desired_spec = OutputSinkWorkerSpec {
        route: route.clone(),
        sample_rate,
        channels,
        chunk_frames: state.output_sink_chunk_frames,
    };
    if let (Some(worker), Some(active_spec)) = (
        state.output_sink_worker.as_ref(),
        state.output_sink_worker_spec.as_ref(),
    ) && active_spec == &desired_spec
    {
        let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
            tx: Some(worker.sender()),
            output_sink_chunk_frames: state.output_sink_chunk_frames,
        });
        return Ok(());
    }

    let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
        tx: None,
        output_sink_chunk_frames: 0,
    });
    shutdown_output_sink_worker(state);

    let worker = open_output_sink_worker(OpenOutputSinkWorkerArgs {
        route: &route,
        sample_rate,
        channels,
        volume: Arc::clone(&state.volume_atomic),
        transition_gain,
        transition_target_gain,
        internal_tx,
    })?;
    let tx = worker.sender();
    state.output_sink_worker = Some(worker);
    state.output_sink_worker_spec = Some(desired_spec);
    let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
        tx: Some(tx),
        output_sink_chunk_frames: state.output_sink_chunk_frames,
    });
    Ok(())
}

fn shutdown_output_sink_worker(state: &mut EngineState) {
    state.output_sink_worker_spec = None;
    let Some(worker) = state.output_sink_worker.take() else {
        return;
    };
    worker.shutdown();
}

fn stop_decode_session(state: &mut EngineState, track_info: &SharedTrackInfo) {
    track_info.store(None);

    let Some(session) = state.session.take() else {
        shutdown_output_sink_worker(state);
        return;
    };

    let _ = session.ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
        tx: None,
        output_sink_chunk_frames: 0,
    });
    shutdown_output_sink_worker(state);

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

fn shutdown_preload_worker(state: &mut EngineState) {
    let Some(worker) = state.preload_worker.take() else {
        return;
    };
    let _ = worker.tx.send(PreloadJob::Shutdown);
    let _ = worker.join.join();
    debug!("preload worker stopped");
}

fn drop_output_pipeline(state: &mut EngineState) {
    if state.output_pipeline.take().is_some() {
        debug!("output pipeline dropped");
    }
}

fn stop_all_audio(state: &mut EngineState, track_info: &SharedTrackInfo) {
    stop_decode_session(state, track_info);
    drop_output_pipeline(state);
}
