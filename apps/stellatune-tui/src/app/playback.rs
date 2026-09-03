use super::{App, Route};

impl App {
    pub(super) async fn activate_selected(&mut self) {
        match self.state.route {
            Route::Library => {
                if let Some(track) = self.state.library.current_track() {
                    let path = track.path.clone();
                    let duration_ms = track.duration_ms;
                    let result = self.play_track_with_hint(path, duration_ms).await;
                    self.try_action("play track", result);
                }
            },
            Route::Playlists => {
                if let Some(track) = self.state.playlists.current_track() {
                    let path = track.path.clone();
                    let duration_ms = track.duration_ms;
                    let result = self.play_track_with_hint(path, duration_ms).await;
                    self.try_action("play track", result);
                }
            },
            Route::Plugins => {
                let selected = self
                    .state
                    .plugins
                    .current_plugin()
                    .map(|plugin| plugin.id.clone());
                if let Some(plugin_id) = selected {
                    let is_disabled = self.state.plugins.disabled_ids.contains(&plugin_id);
                    let action_result = if is_disabled {
                        self.backend.plugin_enable(plugin_id.clone()).await
                    } else {
                        self.backend.plugin_disable(plugin_id.clone()).await
                    };
                    if action_result.is_ok() {
                        let _ = self.backend.plugin_apply_state().await;
                    }
                    self.try_action("toggle plugin", action_result);
                    let _ = self.refresh_plugins().await;
                }
            },
            Route::Settings => {
                self.adjust_settings(true).await;
            },
        }
    }
}
