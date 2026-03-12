use crate::platform::input::{InputAction, KeyCommand};
use crate::runtime::{RuntimeCommand, RuntimeEvent};
use crate::scene::{SceneHit, SceneLayerKind};

use super::actions::{AppAction, AppEffect, AppUpdate};
use super::model::{AppRoute, AppState};

impl AppState {
    pub fn reduce(&mut self, action: AppAction) -> AppUpdate {
        let mut update = AppUpdate::default();

        match action {
            AppAction::Bootstrap => {
                self.scene.status = "bootstrapping renderer graph".to_string();
                update.send_runtime(RuntimeCommand::RequestBootstrapSnapshot);
                update.request_redraw();
            },
            AppAction::FramePresented => {
                self.has_pending_redraw = false;
            },
            AppAction::Input(action) => self.reduce_input(action, &mut update),
            AppAction::Runtime(event) => self.reduce_runtime(event, &mut update),
        }

        self.sync_scene();

        if update
            .effects
            .iter()
            .any(|effect| matches!(effect, AppEffect::RequestRedraw))
        {
            self.has_pending_redraw = true;
        }

        update
    }

    fn reduce_input(&mut self, action: InputAction, update: &mut AppUpdate) {
        match action {
            InputAction::CloseRequested => {},
            InputAction::Resized(size) => {
                self.scene.resize(size);
                update.request_redraw();
            },
            InputAction::PointerMoved { x, y } => {
                self.scene.pointer = Some((x, y));
                self.scene.hovered_node_id = self
                    .scene
                    .graph
                    .hit_test(x as f32, y as f32)
                    .map(|hit| format!("{}:{}", hit.layer.label(), hit.node_id));
                update.request_redraw();
            },
            InputAction::PointerLeft => {
                self.scene.pointer = None;
                self.scene.hovered_node_id = None;
                update.request_redraw();
            },
            InputAction::PointerPrimaryPressed => {
                if let Some((x, y)) = self.scene.pointer {
                    let hit = self.scene.graph.hit_test(x as f32, y as f32);
                    self.apply_click_hit(hit, update);
                } else {
                    self.scene.status = "pointer click ignored: no cursor position".to_string();
                }
                update.request_redraw();
            },
            InputAction::KeyPressed(command) => {
                self.apply_key_command(command);
                update.request_redraw();
            },
        }
    }

    fn reduce_runtime(&mut self, event: RuntimeEvent, update: &mut AppUpdate) {
        match event {
            RuntimeEvent::Heartbeat { tick } => {
                self.scene.status = format!("runtime heartbeat #{tick}");
                update.request_redraw();
            },
            RuntimeEvent::BootstrapSnapshotReady => {
                self.scene.status = "runtime ready".to_string();
                self.playback.track_title = "Stellatune Skeleton Session".to_string();
                self.playback.track_subtitle =
                    "AppState + reducer + renderer pipeline online".to_string();
                update.request_redraw();
            },
        }
    }

    fn apply_key_command(&mut self, command: KeyCommand) {
        match command {
            KeyCommand::ToggleDebugOverlay => {
                self.scene.debug_overlay = !self.scene.debug_overlay;
                self.scene.status = if self.scene.debug_overlay {
                    "debug overlay enabled".to_string()
                } else {
                    "debug overlay disabled".to_string()
                };
            },
            KeyCommand::TogglePlayback => {
                self.playback.status = self.playback.status.toggle();
                self.scene.status = format!("playback state: {}", self.playback.status.label());
            },
            KeyCommand::ToggleSidebar => {
                self.shell.sidebar_open = !self.shell.sidebar_open;
                self.scene.status = if self.shell.sidebar_open {
                    "sidebar opened".to_string()
                } else {
                    "sidebar collapsed".to_string()
                };
            },
            KeyCommand::ToggleQueue => {
                self.shell.queue_open = !self.shell.queue_open;
                self.scene.status = if self.shell.queue_open {
                    "queue panel opened".to_string()
                } else {
                    "queue panel hidden".to_string()
                };
            },
            KeyCommand::CycleVisualMode => {
                self.visual_mode = self.visual_mode.next();
                self.scene.status = format!("visual mode: {}", self.visual_mode.label());
            },
            KeyCommand::NextRoute => {
                self.route = self.route.next();
                self.scene.status = format!("route: {}", self.route.label());
            },
            KeyCommand::RouteLibrary => {
                self.route = AppRoute::Library;
                self.scene.status = "route: library".to_string();
            },
            KeyCommand::RouteNowPlaying => {
                self.route = AppRoute::NowPlaying;
                self.scene.status = "route: now playing".to_string();
            },
            KeyCommand::RouteSettings => {
                self.route = AppRoute::Settings;
                self.scene.status = "route: settings".to_string();
            },
        }
    }

    fn apply_click_hit(&mut self, hit: Option<SceneHit>, update: &mut AppUpdate) {
        let Some(hit) = hit else {
            self.scene.status = "clicked background".to_string();
            return;
        };

        match hit.node_id.as_str() {
            "nav-library" => {
                self.route = AppRoute::Library;
                self.scene.status = "route: library".to_string();
            },
            "nav-now-playing" | "main" | "cover-art" => {
                self.route = AppRoute::NowPlaying;
                self.scene.status = "route: now playing".to_string();
            },
            "nav-settings" => {
                self.route = AppRoute::Settings;
                self.scene.status = "route: settings".to_string();
            },
            "titlebar" | "app-title" | "titlebar-route" => {
                self.scene.status = "dragging window".to_string();
                update.start_window_drag();
            },
            "window-minimize" => {
                self.scene.status = "window minimized".to_string();
                update.minimize_window();
            },
            "window-maximize" => {
                self.scene.status = "window maximize toggled".to_string();
                update.toggle_maximize_window();
            },
            "window-close" => {
                self.scene.status = "window close requested".to_string();
                update.close_window();
            },
            "sidebar-toggle" | "sidebar" => {
                self.shell.sidebar_open = !self.shell.sidebar_open;
                self.scene.status = if self.shell.sidebar_open {
                    "sidebar opened".to_string()
                } else {
                    "sidebar collapsed".to_string()
                };
            },
            "playback-state" => {
                self.playback.status = self.playback.status.toggle();
                self.scene.status = format!("playback state: {}", self.playback.status.label());
            },
            "visual-mode" => {
                self.visual_mode = self.visual_mode.next();
                self.scene.status = format!("visual mode: {}", self.visual_mode.label());
            },
            "queue-toggle" | "status" => {
                self.shell.queue_open = !self.shell.queue_open;
                self.scene.status = if self.shell.queue_open {
                    "queue panel opened".to_string()
                } else {
                    "queue panel hidden".to_string()
                };
            },
            "queue" | "queue-list" => {
                self.scene.status = "queue panel focused".to_string();
            },
            "content-list" => {
                self.scene.status = format!("focused list for route: {}", self.route.label());
            },
            "debug-overlay" if matches!(hit.layer, SceneLayerKind::Overlay) => {
                self.scene.debug_overlay = false;
                self.scene.status = "debug overlay disabled".to_string();
            },
            other => {
                self.scene.status = format!("clicked: {other}");
            },
        }
    }
}
