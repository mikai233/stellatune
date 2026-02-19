use std::collections::HashSet;
use std::time::Instant;

use stellatune_audio::config::engine::{PlayerState, ResampleQuality};
use stellatune_library::{PlaylistLite, TrackLite};

use crate::backend::models::InstalledPluginInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Library,
    Playlists,
    Plugins,
    Settings,
}

impl Route {
    pub fn next(self) -> Self {
        match self {
            Self::Library => Self::Playlists,
            Self::Playlists => Self::Plugins,
            Self::Plugins => Self::Settings,
            Self::Settings => Self::Library,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Library => Self::Settings,
            Self::Playlists => Self::Library,
            Self::Plugins => Self::Playlists,
            Self::Settings => Self::Plugins,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub player_state: PlayerState,
    pub position_ms: i64,
    pub duration_ms: Option<i64>,
    pub current_track_display: String,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            player_state: PlayerState::Stopped,
            position_ms: 0,
            duration_ms: None,
            current_track_display: "-".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommandSuggestion {
    pub insert: String,
    pub display: String,
}

#[derive(Debug, Clone, Default)]
pub struct QueueItem {
    pub path: String,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct ToastState {
    pub message: String,
    pub level: ToastLevel,
    pub expires_at: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct LibraryPaneState {
    pub roots: Vec<String>,
    pub selected_root: usize,
    pub tracks: Vec<TrackLite>,
    pub selected_track: usize,
    pub search_query: Option<String>,
    pub scan_progress: Option<String>,
}

impl LibraryPaneState {
    pub fn current_root(&self) -> String {
        self.roots
            .get(self.selected_root)
            .cloned()
            .unwrap_or_default()
    }

    pub fn current_track(&self) -> Option<&TrackLite> {
        self.tracks.get(self.selected_track)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlaylistsPaneState {
    pub playlists: Vec<PlaylistLite>,
    pub selected_playlist: usize,
    pub tracks: Vec<TrackLite>,
    pub selected_track: usize,
}

impl PlaylistsPaneState {
    pub fn current_playlist(&self) -> Option<&PlaylistLite> {
        self.playlists.get(self.selected_playlist)
    }

    pub fn current_track(&self) -> Option<&TrackLite> {
        self.tracks.get(self.selected_track)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PluginsPaneState {
    pub installed: Vec<InstalledPluginInfo>,
    pub selected: usize,
    pub disabled_ids: HashSet<String>,
    pub active_ids: HashSet<String>,
}

impl PluginsPaneState {
    pub fn current_plugin(&self) -> Option<&InstalledPluginInfo> {
        self.installed.get(self.selected)
    }
}

#[derive(Debug, Clone)]
pub struct SettingsPaneState {
    pub selected: usize,
    pub resample_quality: ResampleQuality,
    pub match_track_sample_rate: bool,
    pub gapless_playback: bool,
    pub seek_track_fade: bool,
}

impl Default for SettingsPaneState {
    fn default() -> Self {
        Self {
            selected: 0,
            resample_quality: ResampleQuality::High,
            match_track_sample_rate: false,
            gapless_playback: true,
            seek_track_fade: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub route: Route,
    pub should_quit: bool,
    pub sidebar_collapsed: bool,
    pub add_root_mode: bool,
    pub add_root_input: String,
    pub command_mode: bool,
    pub command_prefix: char,
    pub command_input: String,
    pub command_suggestions: Vec<CommandSuggestion>,
    pub command_suggestion_index: usize,
    pub pending_g: bool,
    pub last_search_query: String,
    pub last_search_forward: bool,
    pub status_line: String,
    pub toast: Option<ToastState>,
    pub queue: Vec<QueueItem>,
    pub queue_index: Option<usize>,
    pub playback: PlaybackState,
    pub library: LibraryPaneState,
    pub playlists: PlaylistsPaneState,
    pub plugins: PluginsPaneState,
    pub settings: SettingsPaneState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            route: Route::Library,
            should_quit: false,
            sidebar_collapsed: false,
            add_root_mode: false,
            add_root_input: String::new(),
            command_mode: false,
            command_prefix: ':',
            command_input: String::new(),
            command_suggestions: Vec::new(),
            command_suggestion_index: 0,
            pending_g: false,
            last_search_query: String::new(),
            last_search_forward: true,
            status_line: "ready".to_string(),
            toast: None,
            queue: Vec::new(),
            queue_index: None,
            playback: PlaybackState::default(),
            library: LibraryPaneState::default(),
            playlists: PlaylistsPaneState::default(),
            plugins: PluginsPaneState::default(),
            settings: SettingsPaneState::default(),
        }
    }
}

pub fn select_prev(index: &mut usize) {
    *index = index.saturating_sub(1);
}

pub fn select_next(index: &mut usize, len: usize) {
    if len == 0 {
        *index = 0;
        return;
    }
    if *index + 1 < len {
        *index += 1;
    }
}

pub fn clamp_selection(index: &mut usize, len: usize) {
    if len == 0 {
        *index = 0;
        return;
    }
    if *index >= len {
        *index = len - 1;
    }
}
