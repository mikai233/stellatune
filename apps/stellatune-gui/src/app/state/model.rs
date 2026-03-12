use winit::dpi::PhysicalSize;

use crate::scene::SceneState;

#[derive(Debug, Clone)]
pub struct AppState {
    pub route: AppRoute,
    pub playback: PlaybackSummary,
    pub shell: ShellState,
    pub visual_mode: VisualMode,
    pub scene: SceneState,
    pub has_pending_redraw: bool,
}

impl AppState {
    pub fn new(viewport: PhysicalSize<u32>) -> Self {
        let mut state = Self {
            route: AppRoute::Library,
            playback: PlaybackSummary::default(),
            shell: ShellState::default(),
            visual_mode: VisualMode::Calm,
            scene: SceneState::bootstrap(viewport),
            has_pending_redraw: false,
        };
        state.sync_scene();
        state
    }

    pub(crate) fn sync_scene(&mut self) {
        self.scene.route_label = self.route.label().to_string();
        self.scene.playback_label = self.playback.status.label().to_string();
        self.scene.track_title = self.playback.track_title.clone();
        self.scene.track_subtitle = self.playback.track_subtitle.clone();
        self.scene.visual_mode_label = self.visual_mode.label().to_string();
        self.scene.sidebar_open = self.shell.sidebar_open;
        self.scene.queue_open = self.shell.queue_open;
        self.scene.animation_active =
            matches!(self.visual_mode, VisualMode::Pulse | VisualMode::Immersive);
        self.scene.rebuild_graph();
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AppRoute {
    Library,
    NowPlaying,
    Settings,
}

impl AppRoute {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Library => Self::NowPlaying,
            Self::NowPlaying => Self::Settings,
            Self::Settings => Self::Library,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::NowPlaying => "Now Playing",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PlaybackStatus {
    Stopped,
    Paused,
    Playing,
}

impl PlaybackStatus {
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Stopped | Self::Paused => Self::Playing,
            Self::Playing => Self::Paused,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Paused => "Paused",
            Self::Playing => "Playing",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlaybackSummary {
    pub status: PlaybackStatus,
    pub track_title: String,
    pub track_subtitle: String,
}

impl Default for PlaybackSummary {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            track_title: "No track loaded".to_string(),
            track_subtitle: "Waiting for runtime state".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShellState {
    pub sidebar_open: bool,
    pub queue_open: bool,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            sidebar_open: true,
            queue_open: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum VisualMode {
    Calm,
    Pulse,
    Immersive,
}

impl VisualMode {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Calm => Self::Pulse,
            Self::Pulse => Self::Immersive,
            Self::Immersive => Self::Calm,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Calm => "Calm",
            Self::Pulse => "Pulse",
            Self::Immersive => "Immersive",
        }
    }
}
