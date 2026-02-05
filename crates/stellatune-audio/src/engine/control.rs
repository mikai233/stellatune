use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use tracing::{debug, error, info, warn};

use stellatune_core::{Command, Event, PlayerState};
use stellatune_output::{OutputSpec, default_output_spec};
use stellatune_plugins::{PluginManager, default_host_vtable};

use crate::engine::config::{
    BUFFER_HIGH_WATERMARK_MS, BUFFER_LOW_WATERMARK_MS, CONTROL_TICK_MS, UNDERRUN_LOG_INTERVAL,
};
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{DecodeCtrl, EngineCtrl, InternalMsg};
use crate::engine::session::{PlaybackSession, start_session};

/// Handle used by higher layers (e.g. FFI) to drive the player.
#[derive(Clone)]
pub struct EngineHandle {
    cmd_tx: Sender<Command>,
    engine_ctrl_tx: Sender<EngineCtrl>,
    events: Arc<EventHub>,
    plugins: Arc<Mutex<PluginManager>>,
    track_info: Arc<Mutex<Option<stellatune_core::TrackDecodeInfo>>>,
}

impl EngineHandle {
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

    pub fn list_plugins(&self) -> Vec<stellatune_core::PluginDescriptor> {
        let Ok(pm) = self.plugins.lock() else {
            return Vec::new();
        };
        pm.plugins()
            .iter()
            .map(|p| stellatune_core::PluginDescriptor {
                id: p.library.id(),
                name: p.library.name(),
            })
            .collect()
    }

    pub fn list_dsp_types(&self) -> Vec<stellatune_core::DspTypeDescriptor> {
        let Ok(pm) = self.plugins.lock() else {
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

    pub fn current_track_info(&self) -> Option<stellatune_core::TrackDecodeInfo> {
        let Ok(g) = self.track_info.lock() else {
            return None;
        };
        g.clone()
    }
}

pub fn start_engine() -> EngineHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let (engine_ctrl_tx, engine_ctrl_rx) = crossbeam_channel::unbounded();
    let (internal_tx, internal_rx) = crossbeam_channel::unbounded();

    let events = Arc::new(EventHub::new());
    let thread_events = Arc::clone(&events);

    let plugins = Arc::new(Mutex::new(PluginManager::new(default_host_vtable())));
    let track_info = Arc::new(Mutex::new(None));

    let plugins_for_thread = Arc::clone(&plugins);
    let track_info_for_thread = Arc::clone(&track_info);
    let _join: JoinHandle<()> = thread::Builder::new()
        .name("stellatune-control".to_string())
        .spawn(move || {
            run_control_loop(
                cmd_rx,
                engine_ctrl_rx,
                internal_rx,
                internal_tx,
                thread_events,
                plugins_for_thread,
                track_info_for_thread,
            )
        })
        .expect("failed to spawn stellatune-control thread");

    EngineHandle {
        cmd_tx,
        engine_ctrl_tx,
        events,
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
    pending_session_start: bool,
    desired_dsp_chain: Vec<stellatune_core::DspChainItem>,
    lfe_mode: stellatune_core::LfeMode,
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
            pending_session_start: false,
            desired_dsp_chain: Vec::new(),
            lfe_mode: stellatune_core::LfeMode::default(),
        }
    }
}

fn run_control_loop(
    cmd_rx: Receiver<Command>,
    engine_ctrl_rx: Receiver<EngineCtrl>,
    internal_rx: Receiver<InternalMsg>,
    internal_tx: Sender<InternalMsg>,
    events: Arc<EventHub>,
    plugins: Arc<Mutex<PluginManager>>,
    track_info: Arc<Mutex<Option<stellatune_core::TrackDecodeInfo>>>,
) {
    info!("control thread started");
    let mut state = EngineState::new();
    let tick = crossbeam_channel::tick(Duration::from_millis(CONTROL_TICK_MS));

    // Prewarm output spec in the background so the first Play doesn't pay the WASAPI/COM setup cost.
    ensure_output_spec_prewarm(&mut state, &internal_tx);

    loop {
        crossbeam_channel::select! {
            recv(cmd_rx) -> msg => {
                let Ok(cmd) = msg else { break };
                if handle_command(cmd, &mut state, &events, &internal_tx, &plugins, &track_info) {
                    break;
                }
            }
            recv(engine_ctrl_rx) -> msg => {
                let Ok(msg) = msg else { break };
                handle_engine_ctrl(msg, &mut state, &events, &plugins, &track_info);
            }
            recv(internal_rx) -> msg => {
                let Ok(msg) = msg else { break };
                handle_internal(msg, &mut state, &events, &internal_tx, &track_info);
            }
            recv(tick) -> _ => {
                handle_tick(&mut state, &events, &internal_tx, &plugins, &track_info);
            }
        }
    }

    stop_session(&mut state, &events, &track_info);
    events.emit(Event::Log {
        message: "control thread exited".to_string(),
    });
    info!("control thread exited");
}

fn handle_engine_ctrl(
    msg: EngineCtrl,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    plugins: &Arc<Mutex<PluginManager>>,
    track_info: &Arc<Mutex<Option<stellatune_core::TrackDecodeInfo>>>,
) {
    match msg {
        EngineCtrl::SetDspChain { chain } => {
            state.desired_dsp_chain = chain;
            if state.session.is_some() {
                if let Err(message) = apply_dsp_chain(state, plugins) {
                    events.emit(Event::Error { message });
                }
            }
        }
        EngineCtrl::ReloadPlugins { dir } => {
            handle_reload_plugins(state, events, plugins, track_info, dir, Vec::new());
        }
        EngineCtrl::ReloadPluginsWithDisabled { dir, disabled_ids } => {
            handle_reload_plugins(state, events, plugins, track_info, dir, disabled_ids);
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
    track_info: &Arc<Mutex<Option<stellatune_core::TrackDecodeInfo>>>,
    dir: String,
    disabled_ids: Vec<String>,
) {
    // Safe(v1): stop playback so no decode thread holds plugin instances.
    stop_session(state, events, track_info);
    state.wants_playback = false;
    state.play_request_started_at = None;
    state.pending_session_start = false;
    set_state(state, events, PlayerState::Stopped);

    let mut pm = plugins.lock().expect("plugins mutex poisoned");
    *pm = PluginManager::new(default_host_vtable());
    let disabled = disabled_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    pm.set_disabled_ids(disabled.clone());
    match unsafe { pm.load_dir_filtered(&dir, &disabled) } {
        Ok(report) => {
            events.emit(Event::Log {
                message: format!(
                    "plugins reloaded from {}: loaded={} errors={}",
                    dir,
                    report.loaded.len(),
                    report.errors.len()
                ),
            });
            for err in report.errors {
                events.emit(Event::Log {
                    message: format!("plugin load error: {err:#}"),
                });
            }
        }
        Err(e) => {
            events.emit(Event::Error {
                message: format!("failed to reload plugins: {e:#}"),
            });
        }
    }
}

fn handle_internal(
    msg: InternalMsg,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
    track_info: &Arc<Mutex<Option<stellatune_core::TrackDecodeInfo>>>,
) {
    match msg {
        InternalMsg::Eof => {
            events.emit(Event::Log {
                message: "end of stream".to_string(),
            });
            if state.wants_playback {
                if let Some(path) = state.current_track.clone() {
                    events.emit(Event::PlaybackEnded { path });
                }
            }
            stop_session(state, events, track_info);
            state.wants_playback = false;
            state.play_request_started_at = None;
            set_state(state, events, PlayerState::Stopped);
        }
        InternalMsg::Error(message) => {
            events.emit(Event::Error { message });
            stop_session(state, events, track_info);
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
                stop_session(state, events, track_info);
                state.wants_playback = false;
                set_state(state, events, PlayerState::Stopped);
                return;
            };

            let prev_state = state.player_state;
            stop_session(state, events, track_info);

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
        InternalMsg::OutputSpecReady { spec, took_ms } => {
            state.cached_output_spec = Some(spec);
            state.output_spec_prewarm_inflight = false;
            debug!(
                "default_output_spec prewarm ready in {}ms: {}Hz {}ch",
                took_ms, spec.sample_rate, spec.channels
            );
        }
        InternalMsg::OutputSpecFailed { message, took_ms } => {
            state.cached_output_spec = None;
            state.output_spec_prewarm_inflight = false;
            warn!(
                "default_output_spec prewarm failed in {}ms: {}",
                took_ms, message
            );
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
    }
}

fn handle_command(
    cmd: Command,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
    plugins: &Arc<Mutex<PluginManager>>,
    track_info: &Arc<Mutex<Option<stellatune_core::TrackDecodeInfo>>>,
) -> bool {
    match cmd {
        Command::LoadTrack { path } => {
            stop_session(state, events, track_info);
            state.current_track = Some(path.clone());
            state.position_ms = 0;
            state.wants_playback = false;
            state.pending_session_start = false;
            state.play_request_started_at = None;
            if let Ok(mut g) = track_info.lock() {
                *g = None;
            }
            events.emit(Event::TrackChanged { path });
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
                if let Some(out_spec) = state.cached_output_spec {
                    match start_session(
                        path,
                        events.clone(),
                        internal_tx.clone(),
                        out_spec,
                        state.position_ms,
                        Arc::clone(&state.volume_atomic),
                        Arc::clone(plugins),
                        state.lfe_mode,
                    ) {
                        Ok(session) => {
                            if let Ok(mut g) = track_info.lock() {
                                *g = Some(session.track_info.clone());
                            }
                            state.session = Some(session);
                            if let Err(message) = apply_dsp_chain(state, plugins) {
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
                session.output_enabled.store(false, Ordering::Release);
                let _ = session.ctrl_tx.send(DecodeCtrl::Play);
            }

            // Enter Buffering until we have enough samples queued to start cleanly.
            set_state(state, events, PlayerState::Buffering);
            handle_tick(state, events, internal_tx, plugins, track_info);
        }
        Command::Pause => {
            if let Some(session) = state.session.as_ref() {
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
                handle_tick(state, events, internal_tx, plugins, track_info);
            }
        }
        Command::SetVolume { volume } => {
            // UI volume is linear [0, 1], but perceived loudness is roughly logarithmic. Map to a
            // gain curve so the slider feels more even across its range.
            let ui = volume.max(0.0).min(1.0);
            let gain = ui_volume_to_gain(ui);
            state.volume = ui;
            state.volume_atomic.store(gain.to_bits(), Ordering::Relaxed);
            if let Some(session) = state.session.as_ref() {
                session.volume.store(gain.to_bits(), Ordering::Relaxed);
            }
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
            stop_session(state, events, track_info);
            state.position_ms = 0;
            state.wants_playback = false;
            state.play_request_started_at = None;
            state.pending_session_start = false;
            events.emit(Event::Position {
                ms: state.position_ms,
            });
            set_state(state, events, PlayerState::Stopped);
        }
        Command::Shutdown => {
            stop_session(state, events, track_info);
            state.wants_playback = false;
            state.play_request_started_at = None;
            state.pending_session_start = false;
            return true;
        }
    }

    false
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

    state.output_spec_prewarm_inflight = true;
    let tx = internal_tx.clone();
    thread::Builder::new()
        .name("stellatune-output-spec".to_string())
        .spawn(move || {
            let t0 = Instant::now();
            match default_output_spec() {
                Ok(spec) => {
                    let _ = tx.send(InternalMsg::OutputSpecReady {
                        spec,
                        took_ms: t0.elapsed().as_millis() as u64,
                    });
                }
                Err(e) => {
                    let _ = tx.send(InternalMsg::OutputSpecFailed {
                        message: e.to_string(),
                        took_ms: t0.elapsed().as_millis() as u64,
                    });
                }
            }
        })
        .expect("failed to spawn stellatune-output-spec thread");
}

fn handle_tick(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
    plugins: &Arc<Mutex<PluginManager>>,
    track_info: &Arc<Mutex<Option<stellatune_core::TrackDecodeInfo>>>,
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
        let out_spec = state.cached_output_spec.expect("checked");
        match start_session(
            path,
            Arc::clone(events),
            internal_tx.clone(),
            out_spec,
            state.position_ms,
            Arc::clone(&state.volume_atomic),
            Arc::clone(plugins),
            state.lfe_mode,
        ) {
            Ok(session) => {
                if let Ok(mut g) = track_info.lock() {
                    *g = Some(session.track_info.clone());
                }
                state.session = Some(session);
                state.pending_session_start = false;
                if let Err(message) = apply_dsp_chain(state, plugins) {
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

    match state.player_state {
        PlayerState::Playing => {
            if buffered_ms <= BUFFER_LOW_WATERMARK_MS {
                session.output_enabled.store(false, Ordering::Release);
                set_state(state, events, PlayerState::Buffering);
                debug!("buffer low watermark reached: buffered_ms={buffered_ms}");
            } else {
                session.output_enabled.store(true, Ordering::Release);
            }
        }
        PlayerState::Buffering => {
            if buffered_ms >= BUFFER_HIGH_WATERMARK_MS {
                session.output_enabled.store(true, Ordering::Release);
                set_state(state, events, PlayerState::Playing);
                if let Some(t0) = state.play_request_started_at.take() {
                    debug!(
                        "buffering completed: buffered_ms={buffered_ms} elapsed_ms={}",
                        t0.elapsed().as_millis()
                    );
                } else {
                    debug!("buffering completed: buffered_ms={buffered_ms}");
                }
            } else {
                session.output_enabled.store(false, Ordering::Release);
            }
        }
        PlayerState::Paused | PlayerState::Stopped => {
            session.output_enabled.store(false, Ordering::Release);
        }
    }
}

fn apply_dsp_chain(
    state: &mut EngineState,
    plugins: &Arc<Mutex<PluginManager>>,
) -> Result<(), String> {
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

    let pm = plugins
        .lock()
        .map_err(|_| "plugins mutex poisoned".to_string())?;

    let mut chain = Vec::with_capacity(chain_spec.len());
    for item in &chain_spec {
        let Some(key) = pm.find_dsp_key(&item.plugin_id, &item.type_id) else {
            return Err(format!(
                "DSP type not found: plugin_id={} type_id={}",
                item.plugin_id, item.type_id
            ));
        };

        let inst = pm
            .create_dsp(
                key,
                session.out_sample_rate,
                session.out_channels,
                &item.config_json,
            )
            .map_err(|e| {
                format!(
                    "failed to create DSP {}::{}: {e}",
                    item.plugin_id, item.type_id
                )
            })?;
        chain.push(inst);
    }

    let _ = session.ctrl_tx.send(DecodeCtrl::SetDspChain { chain });
    Ok(())
}

fn stop_session(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    track_info: &Arc<Mutex<Option<stellatune_core::TrackDecodeInfo>>>,
) {
    if let Ok(mut g) = track_info.lock() {
        *g = None;
    }

    let Some(session) = state.session.take() else {
        return;
    };

    session.output_enabled.store(false, Ordering::Release);
    let _ = session.ctrl_tx.send(DecodeCtrl::Stop);
    let _ = session.decode_join.join();

    events.emit(Event::Log {
        message: "session stopped".to_string(),
    });
    info!("session stopped");
}
