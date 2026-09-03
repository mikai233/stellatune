mod command_dispatch;
mod commands;
mod events;
mod input;
mod navigation;
mod playback;
mod refresh;
mod settings;
pub mod state;

use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use stellatune_audio::config::engine::ResampleQuality;
use stellatune_audio::playback::event::PlaybackEvent as AudioEvent;
use stellatune_library::{LibraryEvent, TrackLite};

use crate::backend::facade::BackendFacade;

use self::state::{AppState, Route};

pub enum Action {
    Key(KeyEvent),
    EngineEvent(AudioEvent),
    LibraryEvent(LibraryEvent),
}

pub struct App {
    pub state: AppState,
    backend: BackendFacade,
}

impl App {
    const TOAST_TTL: Duration = Duration::from_secs(3);

    pub fn new(backend: BackendFacade) -> Self {
        Self {
            state: AppState::default(),
            backend,
        }
    }

    pub fn on_tick(&mut self) {
        if let Some(toast) = &self.state.toast
            && Instant::now() >= toast.expires_at
        {
            self.state.toast = None;
        }
    }

    pub fn subscribe_player_events(&self) -> tokio::sync::broadcast::Receiver<AudioEvent> {
        self.backend.subscribe_player_events()
    }

    pub fn subscribe_library_events(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<LibraryEvent>> {
        self.backend.subscribe_library_events()
    }

    pub async fn initialize(&mut self) {
        if let Err(error) = self.refresh_all().await {
            self.state.status_line = format!("init failed: {error}");
        }
        if let Ok(snapshot) = self.backend.snapshot().await {
            self.state.playback.player_state = snapshot.state;
            self.state.playback.position_ms = snapshot.consumed_position.as_millis() as i64;
        }
        if let Err(error) = self.apply_audio_settings().await {
            self.state.status_line = format!("failed to apply default audio settings: {error}");
        }
    }

    pub async fn handle_action(&mut self, action: Action) {
        match action {
            Action::Key(key) => self.handle_key(key).await,
            Action::EngineEvent(event) => self.handle_engine_event(event),
            Action::LibraryEvent(event) => self.handle_library_event(event).await,
        }
    }
}

fn command_mode_prefix(key: &KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(':') | KeyCode::Char('：') => Some(':'),
        KeyCode::Char(';') if key.modifiers.contains(KeyModifiers::SHIFT) => Some(':'),
        KeyCode::Char('/') => Some('/'),
        KeyCode::Char('?') => Some('?'),
        _ => None,
    }
}

fn track_matches_query(track: &TrackLite, query_lower: &str) -> bool {
    text_matches_query(&track.path, query_lower)
        || track
            .title
            .as_deref()
            .is_some_and(|value| text_matches_query(value, query_lower))
        || track
            .artist
            .as_deref()
            .is_some_and(|value| text_matches_query(value, query_lower))
        || track
            .album
            .as_deref()
            .is_some_and(|value| text_matches_query(value, query_lower))
}

fn setting_label(index: usize) -> &'static str {
    match index {
        0 => "resample quality",
        1 => "match track sample rate",
        2 => "gapless playback",
        3 => "seek track fade",
        _ => "",
    }
}

fn text_matches_query(text: &str, query_lower: &str) -> bool {
    text.to_lowercase().contains(query_lower)
}

fn track_paths_match(lhs: &str, rhs: &str) -> bool {
    normalize_track_path(lhs) == normalize_track_path(rhs)
}

fn normalize_track_path(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn next_resample_quality(current: ResampleQuality, forward: bool) -> ResampleQuality {
    use ResampleQuality::{Balanced, Fast, High, Ultra};
    if forward {
        match current {
            Fast => Balanced,
            Balanced => High,
            High => Ultra,
            Ultra => Fast,
        }
    } else {
        match current {
            Fast => Ultra,
            Balanced => Fast,
            High => Balanced,
            Ultra => High,
        }
    }
}

fn format_resample_quality(quality: ResampleQuality) -> &'static str {
    match quality {
        ResampleQuality::Fast => "fast",
        ResampleQuality::Balanced => "balanced",
        ResampleQuality::High => "high",
        ResampleQuality::Ultra => "ultra",
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}
