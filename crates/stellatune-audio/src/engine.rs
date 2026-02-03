use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};

use stellatune_core::{Command, Event, PlayerState};

/// Handle used by higher layers (e.g. FFI) to drive the engine.
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
    let events = Arc::new(EventHub::new());

    let thread_events = Arc::clone(&events);
    let _join: JoinHandle<()> = thread::spawn(move || run_engine(cmd_rx, thread_events));

    EngineHandle { cmd_tx, events }
}

struct EngineState {
    player_state: PlayerState,
    position_ms: i64,
    current_track: Option<String>,
    queue: VecDeque<String>,
    volume: f64,
    muted: bool,
}

impl EngineState {
    fn new() -> Self {
        Self {
            player_state: PlayerState::Stopped,
            position_ms: 0,
            current_track: None,
            queue: VecDeque::new(),
            volume: 1.0,
            muted: false,
        }
    }
}

fn run_engine(cmd_rx: Receiver<Command>, events: Arc<EventHub>) {
    let mut state = EngineState::new();

    loop {
        let tick_duration = Duration::from_millis(200);

        if state.player_state == PlayerState::Playing {
            let tick = crossbeam_channel::after(tick_duration);
            crossbeam_channel::select! {
                recv(cmd_rx) -> msg => {
                    let Some(cmd) = msg.ok() else { break };
                    if handle_command(cmd, &mut state, &events) { break; }
                }
                recv(tick) -> _ => {
                    state.position_ms = state.position_ms.saturating_add(200);
                    events.emit(Event::Position { ms: state.position_ms });
                }
            }
        } else {
            match cmd_rx.recv() {
                Ok(cmd) => {
                    if handle_command(cmd, &mut state, &events) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    events.emit(Event::Log {
        message: "engine thread exited".to_string(),
    });
}

fn handle_command(cmd: Command, state: &mut EngineState, events: &EventHub) -> bool {
    match cmd {
        Command::Play => {
            state.player_state = PlayerState::Playing;
            events.emit(Event::StateChanged {
                state: state.player_state,
            });
        }
        Command::Pause => {
            state.player_state = PlayerState::Paused;
            events.emit(Event::StateChanged {
                state: state.player_state,
            });
        }
        Command::Stop => {
            state.player_state = PlayerState::Stopped;
            state.position_ms = 0;
            events.emit(Event::StateChanged {
                state: state.player_state,
            });
            events.emit(Event::Position {
                ms: state.position_ms,
            });
        }
        Command::Seek { ms } => {
            state.position_ms = ms.max(0);
            events.emit(Event::Position {
                ms: state.position_ms,
            });
        }
        Command::LoadTrack { path } => {
            state.current_track = Some(path.clone());
            state.position_ms = 0;
            state.player_state = PlayerState::Stopped;
            events.emit(Event::TrackChanged { path });
            events.emit(Event::StateChanged {
                state: state.player_state,
            });
            events.emit(Event::Position {
                ms: state.position_ms,
            });
        }
        Command::SetVolume { linear } => {
            state.volume = linear.clamp(0.0, 1.0);
            events.emit(Event::Log {
                message: format!("volume set to {:.3}", state.volume),
            });
        }
        Command::SetMuted { muted } => {
            state.muted = muted;
            events.emit(Event::Log {
                message: format!("muted = {}", state.muted),
            });
        }
        Command::Enqueue { path } => {
            state.queue.push_back(path.clone());
            events.emit(Event::Log {
                message: format!("enqueued: {}", path),
            });
        }
        Command::Next => {
            if let Some(next) = state.queue.pop_front() {
                state.current_track = Some(next.clone());
                state.position_ms = 0;
                state.player_state = PlayerState::Stopped;
                events.emit(Event::TrackChanged { path: next });
                events.emit(Event::StateChanged {
                    state: state.player_state,
                });
                events.emit(Event::Position {
                    ms: state.position_ms,
                });
            } else {
                events.emit(Event::Log {
                    message: "queue empty".to_string(),
                });
            }
        }
        Command::Previous => {
            events.emit(Event::Log {
                message: "previous: not implemented".to_string(),
            });
        }
        Command::Shutdown => {
            events.emit(Event::Log {
                message: "shutdown requested".to_string(),
            });
            return true;
        }
    }

    false
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
