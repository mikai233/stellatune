#![allow(unexpected_cfgs)]

use serde::{Deserialize, Serialize};

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Buffering,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Play,
    Pause,
    Stop,
    Seek { ms: i64 },
    LoadTrack { path: String },
    SetVolume { linear: f64 },
    SetMuted { muted: bool },
    Enqueue { path: String },
    Next,
    Previous,
    Shutdown,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    StateChanged { state: PlayerState },
    Position { ms: i64 },
    TrackChanged { path: String },
    Error { message: String },
    Log { message: String },
}
