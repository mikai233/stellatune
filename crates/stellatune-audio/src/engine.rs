use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};

use stellatune_core::{Command, Event, PlayerState};
use stellatune_decode::{Decoder, TrackSpec};
use stellatune_output::{OutputError, OutputHandle, default_output_spec};
use tracing::{error, info, warn};

use crate::ring_buffer::{RingBufferProducer, new_ring_buffer};

const CONTROL_TICK_MS: u64 = 50;
const RING_BUFFER_CAPACITY_MS: usize = 500;
const BUFFER_LOW_WATERMARK_MS: i64 = 60;
const BUFFER_HIGH_WATERMARK_MS: i64 = 200;
const UNDERRUN_LOG_INTERVAL: Duration = Duration::from_secs(1);

/// Handle used by higher layers (e.g. FFI) to drive the player.
#[derive(Clone)]
pub struct EngineHandle {
    cmd_tx: Sender<Command>,
    events: Arc<EventHub>,
}

impl EngineHandle {
    pub fn send_command(&self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn subscribe_events(&self) -> Receiver<Event> {
        self.events.subscribe()
    }
}

pub fn start_engine() -> EngineHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let (internal_tx, internal_rx) = crossbeam_channel::unbounded();

    let events = Arc::new(EventHub::new());
    let thread_events = Arc::clone(&events);

    let _join: JoinHandle<()> = thread::Builder::new()
        .name("stellatune-control".to_string())
        .spawn(move || run_control_loop(cmd_rx, internal_rx, internal_tx, thread_events))
        .expect("failed to spawn stellatune-control thread");

    EngineHandle { cmd_tx, events }
}

enum DecodeCtrl {
    Setup {
        producer: RingBufferProducer<f32>,
        target_sample_rate: u32,
        target_channels: u16,
        start_at_ms: i64,
    },
    Play,
    Pause,
    Stop,
}

enum InternalMsg {
    Eof,
    Error(String),
    OutputError(String),
    Position(i64),
}

struct PlaybackSession {
    ctrl_tx: Sender<DecodeCtrl>,
    decode_join: JoinHandle<()>,
    _output: OutputHandle,
    output_enabled: Arc<AtomicBool>,
    buffered_samples: Arc<AtomicUsize>,
    underrun_callbacks: Arc<AtomicU64>,
    out_sample_rate: u32,
    out_channels: u16,
}

struct EngineState {
    player_state: PlayerState,
    position_ms: i64,
    current_track: Option<String>,
    session: Option<PlaybackSession>,
    wants_playback: bool,
    last_underrun_total: u64,
    last_underrun_log_at: Instant,
}

impl EngineState {
    fn new() -> Self {
        Self {
            player_state: PlayerState::Stopped,
            position_ms: 0,
            current_track: None,
            session: None,
            wants_playback: false,
            last_underrun_total: 0,
            last_underrun_log_at: Instant::now(),
        }
    }
}

fn run_control_loop(
    cmd_rx: Receiver<Command>,
    internal_rx: Receiver<InternalMsg>,
    internal_tx: Sender<InternalMsg>,
    events: Arc<EventHub>,
) {
    info!("control thread started");
    let mut state = EngineState::new();
    let tick = crossbeam_channel::tick(Duration::from_millis(CONTROL_TICK_MS));

    loop {
        crossbeam_channel::select! {
            recv(cmd_rx) -> msg => {
                let Ok(cmd) = msg else { break };
                if handle_command(cmd, &mut state, &events, &internal_tx) {
                    break;
                }
            }
            recv(internal_rx) -> msg => {
                let Ok(msg) = msg else { break };
                handle_internal(msg, &mut state, &events, &internal_tx);
            }
            recv(tick) -> _ => {
                handle_tick(&mut state, &events);
            }
        }
    }

    stop_session(&mut state, &events);
    events.emit(Event::Log {
        message: "control thread exited".to_string(),
    });
    info!("control thread exited");
}

fn handle_internal(
    msg: InternalMsg,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
) {
    match msg {
        InternalMsg::Eof => {
            events.emit(Event::Log {
                message: "end of stream".to_string(),
            });
            stop_session(state, events);
            state.wants_playback = false;
            set_state(state, events, PlayerState::Stopped);
        }
        InternalMsg::Error(message) => {
            events.emit(Event::Error { message });
            stop_session(state, events);
            state.wants_playback = false;
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

            let Some(path) = state.current_track.clone() else {
                stop_session(state, events);
                state.wants_playback = false;
                set_state(state, events, PlayerState::Stopped);
                return;
            };

            let prev_state = state.player_state;
            stop_session(state, events);

            set_state(state, events, PlayerState::Buffering);
            match start_session(
                path,
                Arc::clone(events),
                internal_tx.clone(),
                state.position_ms,
            ) {
                Ok(session) => {
                    state.session = Some(session);

                    match prev_state {
                        PlayerState::Playing | PlayerState::Buffering => {
                            if let Some(session) = state.session.as_ref() {
                                session.output_enabled.store(false, Ordering::Release);
                                let _ = session.ctrl_tx.send(DecodeCtrl::Play);
                            }
                            state.wants_playback = true;
                            set_state(state, events, PlayerState::Buffering);
                            handle_tick(state, events);
                        }
                        PlayerState::Paused => {
                            if let Some(session) = state.session.as_ref() {
                                session.output_enabled.store(false, Ordering::Release);
                            }
                            state.wants_playback = false;
                            set_state(state, events, PlayerState::Paused);
                        }
                        PlayerState::Stopped => {
                            stop_session(state, events);
                            state.wants_playback = false;
                            set_state(state, events, PlayerState::Stopped);
                        }
                    }

                    events.emit(Event::Log {
                        message: "audio session restarted after output error".to_string(),
                    });
                }
                Err(err) => {
                    events.emit(Event::Error {
                        message: format!("failed to restart audio session: {err}"),
                    });
                    set_state(state, events, PlayerState::Stopped);
                }
            }
        }
        InternalMsg::Position(ms) => {
            state.position_ms = ms;
        }
    }
}

fn handle_command(
    cmd: Command,
    state: &mut EngineState,
    events: &Arc<EventHub>,
    internal_tx: &Sender<InternalMsg>,
) -> bool {
    match cmd {
        Command::LoadTrack { path } => {
            stop_session(state, events);
            state.current_track = Some(path.clone());
            state.position_ms = 0;
            state.wants_playback = false;
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

            if state.session.is_none() {
                set_state(state, events, PlayerState::Buffering);
                match start_session(path, events.clone(), internal_tx.clone(), state.position_ms) {
                    Ok(session) => state.session = Some(session),
                    Err(message) => {
                        events.emit(Event::Error { message });
                        set_state(state, events, PlayerState::Stopped);
                        return false;
                    }
                }
            }

            if let Some(session) = state.session.as_ref() {
                session.output_enabled.store(false, Ordering::Release);
                let _ = session.ctrl_tx.send(DecodeCtrl::Play);
            }

            // Enter Buffering until we have enough samples queued to start cleanly.
            set_state(state, events, PlayerState::Buffering);
            handle_tick(state, events);
        }
        Command::Pause => {
            if let Some(session) = state.session.as_ref() {
                session.output_enabled.store(false, Ordering::Release);
                let _ = session.ctrl_tx.send(DecodeCtrl::Pause);
            }
            state.wants_playback = false;
            set_state(state, events, PlayerState::Paused);
        }
        Command::Stop => {
            stop_session(state, events);
            state.position_ms = 0;
            state.wants_playback = false;
            events.emit(Event::Position {
                ms: state.position_ms,
            });
            set_state(state, events, PlayerState::Stopped);
        }
        Command::Shutdown => {
            stop_session(state, events);
            state.wants_playback = false;
            return true;
        }
    }

    false
}

fn set_state(state: &mut EngineState, events: &Arc<EventHub>, new_state: PlayerState) {
    if state.player_state == new_state {
        return;
    }
    state.player_state = new_state;
    events.emit(Event::StateChanged { state: new_state });
}

fn handle_tick(state: &mut EngineState, events: &Arc<EventHub>) {
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
            } else {
                session.output_enabled.store(true, Ordering::Release);
            }
        }
        PlayerState::Buffering => {
            if buffered_ms >= BUFFER_HIGH_WATERMARK_MS {
                session.output_enabled.store(true, Ordering::Release);
                set_state(state, events, PlayerState::Playing);
            } else {
                session.output_enabled.store(false, Ordering::Release);
            }
        }
        PlayerState::Paused | PlayerState::Stopped => {
            session.output_enabled.store(false, Ordering::Release);
        }
    }
}

fn stop_session(state: &mut EngineState, events: &Arc<EventHub>) {
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

fn start_session(
    path: String,
    events: Arc<EventHub>,
    internal_tx: Sender<InternalMsg>,
    start_at_ms: i64,
) -> Result<PlaybackSession, String> {
    info!("starting session");
    let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded();
    let (setup_tx, setup_rx) = crossbeam_channel::bounded::<DecodeCtrl>(1);
    let (spec_tx, spec_rx) = crossbeam_channel::bounded::<Result<TrackSpec, String>>(1);

    let thread_path = path.clone();
    let thread_events = Arc::clone(&events);
    let thread_internal_tx = internal_tx.clone();

    let decode_join = thread::Builder::new()
        .name("stellatune-decode".to_string())
        .spawn(move || {
            decode_thread(
                thread_path,
                thread_events,
                thread_internal_tx,
                ctrl_rx,
                setup_rx,
                spec_tx,
            )
        })
        .expect("failed to spawn stellatune-decode thread");

    let _track_spec = match spec_rx.recv() {
        Ok(Ok(spec)) => spec,
        Ok(Err(message)) => return Err(message),
        Err(_) => return Err("decoder thread exited unexpectedly".to_string()),
    };

    let out_spec = default_output_spec().map_err(|e| e.to_string())?;
    if out_spec.channels != 1 && out_spec.channels != 2 {
        warn!("unsupported output channels: {}", out_spec.channels);
        return Err(format!(
            "output channels = {}, only mono/stereo is supported",
            out_spec.channels
        ));
    }

    let capacity_samples =
        ((out_spec.sample_rate as usize * out_spec.channels as usize * RING_BUFFER_CAPACITY_MS)
            / 1000)
            .max(1024);
    let (producer, consumer) = new_ring_buffer::<f32>(capacity_samples);

    let output_enabled = Arc::new(AtomicBool::new(false));
    let buffered_samples = Arc::new(AtomicUsize::new(0));
    let underrun_callbacks = Arc::new(AtomicU64::new(0));
    let output_consumer = GatedConsumer {
        inner: consumer,
        enabled: Arc::clone(&output_enabled),
        buffered_samples: Arc::clone(&buffered_samples),
        underrun_callbacks: Arc::clone(&underrun_callbacks),
    };

    let output_internal_tx = internal_tx.clone();
    let output = OutputHandle::start(output_consumer, out_spec.sample_rate, move |err| {
        let _ = output_internal_tx.try_send(InternalMsg::OutputError(err.to_string()));
    })
    .map_err(|e| match e {
        OutputError::ConfigMismatch { message } => message,
        other => other.to_string(),
    })?;

    setup_tx
        .send(DecodeCtrl::Setup {
            producer,
            target_sample_rate: out_spec.sample_rate,
            target_channels: out_spec.channels,
            start_at_ms,
        })
        .map_err(|_| "decoder thread exited unexpectedly".to_string())?;

    Ok(PlaybackSession {
        ctrl_tx,
        decode_join,
        _output: output,
        output_enabled,
        buffered_samples,
        underrun_callbacks,
        out_sample_rate: out_spec.sample_rate,
        out_channels: out_spec.channels,
    })
}

struct GatedConsumer {
    inner: crate::ring_buffer::RingBufferConsumer<f32>,
    enabled: Arc<AtomicBool>,
    buffered_samples: Arc<AtomicUsize>,
    underrun_callbacks: Arc<AtomicU64>,
}

impl stellatune_output::SampleConsumer for GatedConsumer {
    fn pop_sample(&mut self) -> Option<f32> {
        if !self.enabled.load(Ordering::Acquire) {
            return None;
        }
        self.inner.pop_sample()
    }

    fn on_output(&mut self, requested: usize, provided: usize) {
        self.buffered_samples
            .store(self.inner.len(), Ordering::Relaxed);
        if self.enabled.load(Ordering::Relaxed) && provided < requested {
            self.underrun_callbacks.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn decode_thread(
    path: String,
    events: Arc<EventHub>,
    internal_tx: Sender<InternalMsg>,
    ctrl_rx: Receiver<DecodeCtrl>,
    setup_rx: Receiver<DecodeCtrl>,
    spec_tx: Sender<Result<TrackSpec, String>>,
) {
    let mut decoder = match Decoder::open(&path) {
        Ok(d) => d,
        Err(e) => {
            let _ = spec_tx.send(Err(format!("failed to open decoder: {e}")));
            return;
        }
    };

    let spec = decoder.spec();
    let _ = spec_tx.send(Ok(spec));

    let (mut producer, target_sample_rate, target_channels, start_at_ms) = loop {
        crossbeam_channel::select! {
            recv(setup_rx) -> msg => {
                let Ok(ctrl) = msg else { return };
                if let DecodeCtrl::Setup { producer, target_sample_rate, target_channels, start_at_ms } = ctrl {
                    break (producer, target_sample_rate, target_channels, start_at_ms);
                }
            }
            recv(ctrl_rx) -> msg => {
                let Ok(msg) = msg else { return };
                if matches!(msg, DecodeCtrl::Stop) {
                    return;
                }
            }
        }
    };

    let in_channels = spec.channels as usize;
    let out_channels = target_channels as usize;

    let base_ms = start_at_ms.max(0);
    if base_ms > 0 {
        let frames_to_skip = ((base_ms as i128 * spec.sample_rate as i128) / 1000) as u64;
        if !skip_frames_by_decoding(&mut decoder, frames_to_skip) {
            let _ = internal_tx.send(InternalMsg::Eof);
            return;
        }
    }

    let mut resampler =
        match create_resampler_if_needed(spec.sample_rate, target_sample_rate, out_channels) {
            Ok(r) => r,
            Err(e) => {
                let _ = internal_tx.send(InternalMsg::Error(e));
                return;
            }
        };

    let mut playing = false;
    let mut frames_written: u64 = 0;
    let mut last_emit = Instant::now();
    let mut decode_pending: Vec<f32> = Vec::new();
    let mut out_pending: Vec<f32> = Vec::new();

    loop {
        if !playing {
            match ctrl_rx.recv() {
                Ok(DecodeCtrl::Play) => {
                    playing = true;
                    last_emit = Instant::now();
                }
                Ok(DecodeCtrl::Pause) => {}
                Ok(DecodeCtrl::Setup { .. }) => {}
                Ok(DecodeCtrl::Stop) | Err(_) => break,
            }
            continue;
        }

        while let Ok(ctrl) = ctrl_rx.try_recv() {
            match ctrl {
                DecodeCtrl::Pause => {
                    playing = false;
                    break;
                }
                DecodeCtrl::Stop => return,
                DecodeCtrl::Play => {}
                DecodeCtrl::Setup { .. } => {}
            }
        }
        if !playing {
            continue;
        }

        if last_emit.elapsed() >= Duration::from_millis(200) {
            let buffered_frames = (producer.len() / out_channels) as u64;
            let played_frames = frames_written.saturating_sub(buffered_frames);
            let ms = base_ms.saturating_add(
                ((played_frames.saturating_mul(1000)) / target_sample_rate as u64) as i64,
            );
            events.emit(Event::Position { ms });
            let _ = internal_tx.try_send(InternalMsg::Position(ms));
            last_emit = Instant::now();
        }

        match decoder.next_block(4096) {
            Ok(Some(samples)) => {
                decode_pending.extend_from_slice(&samples);
                if resampler.is_none() {
                    // Channel adaptation only.
                    if in_channels == out_channels {
                        out_pending.extend_from_slice(&decode_pending);
                    } else {
                        out_pending.extend_from_slice(&adapt_channels_interleaved(
                            &decode_pending,
                            in_channels,
                            out_channels,
                        ));
                    }
                    decode_pending.clear();
                    if write_pending(
                        &mut producer,
                        &mut out_pending,
                        &mut frames_written,
                        out_channels,
                        &ctrl_rx,
                        &mut playing,
                    ) {
                        return;
                    }
                    continue;
                }

                while decode_pending.len() >= RESAMPLE_CHUNK_FRAMES * in_channels {
                    let chunk_in: Vec<f32> = decode_pending
                        .drain(..RESAMPLE_CHUNK_FRAMES * in_channels)
                        .collect();
                    let chunk = if in_channels == out_channels {
                        chunk_in
                    } else {
                        adapt_channels_interleaved(&chunk_in, in_channels, out_channels)
                    };

                    let processed = match resample_interleaved_chunk(
                        resampler.as_mut().expect("checked"),
                        &chunk,
                        out_channels,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            let _ = internal_tx.send(InternalMsg::Error(e));
                            return;
                        }
                    };
                    out_pending.extend_from_slice(&processed);

                    if write_pending(
                        &mut producer,
                        &mut out_pending,
                        &mut frames_written,
                        out_channels,
                        &ctrl_rx,
                        &mut playing,
                    ) {
                        return;
                    }
                    if !playing {
                        break;
                    }
                }
            }
            Ok(None) => {
                if let Some(resampler) = resampler.as_mut() {
                    if !decode_pending.is_empty() {
                        decode_pending.resize(RESAMPLE_CHUNK_FRAMES * in_channels, 0.0);
                        let chunk = if in_channels == out_channels {
                            decode_pending.clone()
                        } else {
                            adapt_channels_interleaved(&decode_pending, in_channels, out_channels)
                        };
                        match resample_interleaved_chunk(resampler, &chunk, out_channels) {
                            Ok(processed) => {
                                out_pending.extend_from_slice(&processed);
                                decode_pending.clear();
                            }
                            Err(e) => {
                                let _ = internal_tx.send(InternalMsg::Error(e));
                                return;
                            }
                        }
                    }
                    while !out_pending.is_empty() {
                        if write_pending(
                            &mut producer,
                            &mut out_pending,
                            &mut frames_written,
                            out_channels,
                            &ctrl_rx,
                            &mut playing,
                        ) {
                            return;
                        }
                        if !playing {
                            break;
                        }
                    }
                } else if !decode_pending.is_empty() {
                    if in_channels == out_channels {
                        out_pending.extend_from_slice(&decode_pending);
                    } else {
                        out_pending.extend_from_slice(&adapt_channels_interleaved(
                            &decode_pending,
                            in_channels,
                            out_channels,
                        ));
                    }
                    decode_pending.clear();
                    while !out_pending.is_empty() {
                        if write_pending(
                            &mut producer,
                            &mut out_pending,
                            &mut frames_written,
                            out_channels,
                            &ctrl_rx,
                            &mut playing,
                        ) {
                            return;
                        }
                        if !playing {
                            break;
                        }
                    }
                }
                let _ = internal_tx.send(InternalMsg::Eof);
                break;
            }
            Err(e) => {
                let _ = internal_tx.send(InternalMsg::Error(format!("{e}")));
                break;
            }
        }
    }
}

fn skip_frames_by_decoding(decoder: &mut Decoder, mut frames_to_skip: u64) -> bool {
    // Best-effort: decode and discard samples until reaching the requested frame offset.
    // This is only used during output reinitialization (rare), so it can be slower.
    while frames_to_skip > 0 {
        let want = (frames_to_skip.min(2048)) as usize;
        match decoder.next_block(want) {
            Ok(Some(block)) => {
                let got_frames = (block.len() / decoder.spec().channels as usize) as u64;
                if got_frames == 0 {
                    return false;
                }
                frames_to_skip = frames_to_skip.saturating_sub(got_frames);
            }
            Ok(None) => return false,
            Err(_) => return false,
        }
    }
    true
}

fn write_pending(
    producer: &mut RingBufferProducer<f32>,
    pending: &mut Vec<f32>,
    frames_written: &mut u64,
    channels_per_frame: usize,
    ctrl_rx: &Receiver<DecodeCtrl>,
    playing: &mut bool,
) -> bool {
    let mut offset = 0usize;
    while offset < pending.len() {
        while let Ok(ctrl) = ctrl_rx.try_recv() {
            match ctrl {
                DecodeCtrl::Pause => {
                    *playing = false;
                    break;
                }
                DecodeCtrl::Stop => return true,
                DecodeCtrl::Play => {}
                DecodeCtrl::Setup { .. } => {}
            }
        }
        if !*playing {
            break;
        }

        let written = producer.push_slice(&pending[offset..]);
        offset += written;
        *frames_written = (*frames_written).saturating_add((written / channels_per_frame) as u64);
        if written == 0 {
            thread::sleep(Duration::from_millis(2));
        }
    }

    if offset > 0 {
        pending.drain(..offset);
    }

    false
}

const RESAMPLE_CHUNK_FRAMES: usize = 1024;

fn create_resampler_if_needed(
    src_rate: u32,
    dst_rate: u32,
    channels: usize,
) -> Result<Option<rubato::Async<f32>>, String> {
    if src_rate == dst_rate {
        return Ok(None);
    }

    use rubato::{
        Async, FixedAsync, SincInterpolationParameters, SincInterpolationType, WindowFunction,
    };

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        oversampling_factor: 128,
        interpolation: SincInterpolationType::Linear,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = dst_rate as f64 / src_rate as f64;
    let resampler = Async::<f32>::new_sinc(
        ratio,
        2.0,
        &params,
        RESAMPLE_CHUNK_FRAMES,
        channels,
        FixedAsync::Input,
    )
    .map_err(|e| format!("failed to create resampler: {e}"))?;
    Ok(Some(resampler))
}

fn resample_interleaved_chunk(
    resampler: &mut rubato::Async<f32>,
    chunk_interleaved: &[f32],
    channels: usize,
) -> Result<Vec<f32>, String> {
    use audioadapter_buffers::direct::InterleavedSlice;
    use rubato::Resampler;

    let frames = chunk_interleaved.len() / channels;
    let input = InterleavedSlice::new(chunk_interleaved, channels, frames)
        .map_err(|e| format!("resample input buffer error: {e}"))?;

    let out = resampler
        .process(&input, 0, None)
        .map_err(|e| format!("resample error: {e}"))?;

    Ok(out.take_data())
}

fn adapt_channels_interleaved(input: &[f32], in_channels: usize, out_channels: usize) -> Vec<f32> {
    if in_channels == out_channels {
        return input.to_vec();
    }

    let frames = input.len() / in_channels;
    match (in_channels, out_channels) {
        (1, 2) => {
            let mut out = Vec::with_capacity(frames * 2);
            for i in 0..frames {
                let s = input[i];
                out.push(s);
                out.push(s);
            }
            out
        }
        (2, 1) => {
            let mut out = Vec::with_capacity(frames);
            for i in 0..frames {
                let l = input[i * 2];
                let r = input[i * 2 + 1];
                out.push((l + r) * 0.5);
            }
            out
        }
        _ => input.to_vec(),
    }
}

struct EventHub {
    subscribers: Mutex<Vec<Sender<Event>>>,
}

impl EventHub {
    fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.subscribers
            .lock()
            .expect("event hub mutex poisoned")
            .push(tx);
        rx
    }

    fn emit(&self, event: Event) {
        let mut subs = self.subscribers.lock().expect("event hub mutex poisoned");
        subs.retain(|tx| tx.send(event.clone()).is_ok());
    }
}
