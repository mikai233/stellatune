use winit::dpi::PhysicalSize;

use super::builder::build_scene_graph;
use super::SceneGraph;

#[derive(Debug, Clone)]
pub struct SceneState {
    pub app_title: String,
    pub status: String,
    pub route_label: String,
    pub playback_label: String,
    pub track_title: String,
    pub track_subtitle: String,
    pub visual_mode_label: String,
    pub sidebar_open: bool,
    pub queue_open: bool,
    pub debug_overlay: bool,
    pub animation_active: bool,
    pub pointer: Option<(f64, f64)>,
    pub hovered_node_id: Option<String>,
    pub viewport: PhysicalSize<u32>,
    pub graph: SceneGraph,
}

impl SceneState {
    pub fn bootstrap(viewport: PhysicalSize<u32>) -> Self {
        let mut state = Self {
            app_title: "Stellatune".to_string(),
            status: "bootstrapping renderer graph".to_string(),
            route_label: "Library".to_string(),
            playback_label: "Stopped".to_string(),
            track_title: "No track loaded".to_string(),
            track_subtitle: "Waiting for backend bootstrap".to_string(),
            visual_mode_label: "Calm".to_string(),
            sidebar_open: true,
            queue_open: false,
            debug_overlay: false,
            animation_active: false,
            pointer: None,
            hovered_node_id: None,
            viewport,
            graph: SceneGraph::default(),
        };
        state.rebuild_graph();
        state
    }

    pub fn resize(&mut self, viewport: PhysicalSize<u32>) {
        self.viewport = viewport;
        self.rebuild_graph();
    }

    pub fn rebuild_graph(&mut self) {
        self.graph = build_scene_graph(self);
    }
}
