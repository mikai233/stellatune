use std::time::Instant;

use anyhow::Result;

use super::commands::build_command_suggestions;
use super::state::{
    QueueItem, Route, ToastLevel, ToastState, clamp_selection, select_next, select_prev,
};
use super::{App, setting_label, text_matches_query, track_matches_query};

impl App {
    pub(super) fn select_up(&mut self) {
        match self.state.route {
            Route::Library => select_prev(&mut self.state.library.selected_track),
            Route::Playlists => select_prev(&mut self.state.playlists.selected_track),
            Route::Plugins => select_prev(&mut self.state.plugins.selected),
            Route::Settings => select_prev(&mut self.state.settings.selected),
        }
    }

    pub(super) fn select_down(&mut self) {
        match self.state.route {
            Route::Library => select_next(
                &mut self.state.library.selected_track,
                self.state.library.tracks.len(),
            ),
            Route::Playlists => select_next(
                &mut self.state.playlists.selected_track,
                self.state.playlists.tracks.len(),
            ),
            Route::Plugins => select_next(
                &mut self.state.plugins.selected,
                self.state.plugins.installed.len(),
            ),
            Route::Settings => select_next(&mut self.state.settings.selected, 4),
        }
    }

    pub(super) async fn select_prev_group(&mut self) {
        match self.state.route {
            Route::Library => {
                select_prev(&mut self.state.library.selected_root);
                let _ = self.refresh_library_tracks_only().await;
            },
            Route::Playlists => {
                select_prev(&mut self.state.playlists.selected_playlist);
                let _ = self.refresh_playlist_tracks().await;
            },
            _ => {},
        }
    }

    pub(super) async fn select_next_group(&mut self) {
        match self.state.route {
            Route::Library => {
                select_next(
                    &mut self.state.library.selected_root,
                    self.state.library.roots.len(),
                );
                let _ = self.refresh_library_tracks_only().await;
            },
            Route::Playlists => {
                select_next(
                    &mut self.state.playlists.selected_playlist,
                    self.state.playlists.playlists.len(),
                );
                let _ = self.refresh_playlist_tracks().await;
            },
            _ => {},
        }
    }

    pub(super) fn open_command_mode(&mut self, prefix: char) {
        self.state.command_mode = true;
        self.state.command_prefix = prefix;
        self.state.command_input.clear();
        self.state.pending_g = false;
        self.state.command_suggestion_index = 0;
        self.refresh_command_suggestions();
    }

    pub(super) fn refresh_command_suggestions(&mut self) {
        self.state.command_suggestions = build_command_suggestions(
            self.state.command_prefix,
            &self.state.command_input,
            &self.state.last_search_query,
        );
        clamp_selection(
            &mut self.state.command_suggestion_index,
            self.state.command_suggestions.len(),
        );
    }

    pub(super) fn select_prev_command_suggestion(&mut self) {
        select_prev(&mut self.state.command_suggestion_index);
    }

    pub(super) fn select_next_command_suggestion(&mut self) {
        select_next(
            &mut self.state.command_suggestion_index,
            self.state.command_suggestions.len(),
        );
    }

    pub(super) fn accept_selected_command_suggestion(&mut self) {
        let suggestion = self
            .state
            .command_suggestions
            .get(self.state.command_suggestion_index)
            .cloned();
        if let Some(suggestion) = suggestion {
            if suggestion.insert.is_empty() {
                return;
            }
            self.state.command_input = suggestion.insert;
            self.refresh_command_suggestions();
        }
    }

    pub(super) fn toast_with_level(&mut self, level: ToastLevel, message: impl Into<String>) {
        let message = message.into();
        self.state.status_line = message.clone();
        self.state.toast = Some(ToastState {
            message,
            level,
            expires_at: Instant::now() + Self::TOAST_TTL,
        });
    }

    pub(super) fn toast_info(&mut self, message: impl Into<String>) {
        self.toast_with_level(ToastLevel::Info, message);
    }

    pub(super) fn toast_warn(&mut self, message: impl Into<String>) {
        self.toast_with_level(ToastLevel::Warn, message);
    }

    pub(super) fn toast_error(&mut self, message: impl Into<String>) {
        self.toast_with_level(ToastLevel::Error, message);
    }

    pub(super) fn enqueue_path(&mut self, path: String, duration_ms: Option<i64>) {
        let item = QueueItem {
            path: path.clone(),
            duration_ms: duration_ms.filter(|value| *value > 0),
        };
        self.state.queue.push(item);
        self.toast_info(format!("queued: {path}"));
    }

    pub(super) fn enqueue_selected_track(&mut self) {
        let selected = match self.state.route {
            Route::Library => self
                .state
                .library
                .current_track()
                .map(|track| (track.path.clone(), track.duration_ms)),
            Route::Playlists => self
                .state
                .playlists
                .current_track()
                .map(|track| (track.path.clone(), track.duration_ms)),
            _ => None,
        };
        if let Some((path, duration_ms)) = selected {
            self.enqueue_path(path, duration_ms);
        } else {
            self.toast_warn("no track selected to queue");
        }
    }

    pub(super) async fn seek_to_ms(&mut self, position_ms: i64) -> Result<()> {
        self.backend.seek_ms(position_ms.max(0)).await?;
        self.state.playback.position_ms = position_ms.max(0);
        Ok(())
    }

    pub(super) async fn seek_by_ms(&mut self, delta_ms: i64) -> Result<()> {
        let target = self
            .state
            .playback
            .position_ms
            .saturating_add(delta_ms)
            .max(0);
        self.seek_to_ms(target).await
    }

    pub(super) async fn play_next_track(&mut self) -> Result<()> {
        if let Some((index, item)) = self.next_queue_item() {
            let result = self
                .play_track_with_hint(item.path.clone(), item.duration_ms)
                .await;
            if result.is_ok() {
                self.state.queue_index = Some(index);
            }
            return result;
        }

        if let Some((path, duration_ms)) = self.next_track_from_active_list() {
            return self.play_track_with_hint(path, duration_ms).await;
        }

        self.toast_warn("no next track");
        Ok(())
    }

    pub(super) async fn play_prev_track(&mut self) -> Result<()> {
        if let Some((index, item)) = self.prev_queue_item() {
            let result = self
                .play_track_with_hint(item.path.clone(), item.duration_ms)
                .await;
            if result.is_ok() {
                self.state.queue_index = Some(index);
            }
            return result;
        }

        if let Some((path, duration_ms)) = self.prev_track_from_active_list() {
            return self.play_track_with_hint(path, duration_ms).await;
        }

        self.toast_warn("no previous track");
        Ok(())
    }

    pub(super) fn next_queue_item(&self) -> Option<(usize, QueueItem)> {
        if self.state.queue.is_empty() {
            return None;
        }
        let next_index = match self.state.queue_index {
            Some(current) if current + 1 < self.state.queue.len() => current + 1,
            Some(_) => return None,
            None => 0,
        };
        self.state
            .queue
            .get(next_index)
            .cloned()
            .map(|item| (next_index, item))
    }

    pub(super) fn prev_queue_item(&self) -> Option<(usize, QueueItem)> {
        if self.state.queue.is_empty() {
            return None;
        }
        let prev_index = match self.state.queue_index {
            Some(current) if current > 0 => current - 1,
            Some(_) => return None,
            None => return None,
        };
        self.state
            .queue
            .get(prev_index)
            .cloned()
            .map(|item| (prev_index, item))
    }

    pub(super) fn next_track_from_active_list(&mut self) -> Option<(String, Option<i64>)> {
        match self.state.route {
            Route::Library => {
                let next = self.state.library.selected_track + 1;
                if next < self.state.library.tracks.len() {
                    self.state.library.selected_track = next;
                    self.state
                        .library
                        .tracks
                        .get(next)
                        .map(|track| (track.path.clone(), track.duration_ms))
                } else {
                    None
                }
            },
            Route::Playlists => {
                let next = self.state.playlists.selected_track + 1;
                if next < self.state.playlists.tracks.len() {
                    self.state.playlists.selected_track = next;
                    self.state
                        .playlists
                        .tracks
                        .get(next)
                        .map(|track| (track.path.clone(), track.duration_ms))
                } else {
                    None
                }
            },
            _ => None,
        }
    }

    pub(super) fn prev_track_from_active_list(&mut self) -> Option<(String, Option<i64>)> {
        match self.state.route {
            Route::Library => {
                let prev = self.state.library.selected_track.checked_sub(1)?;
                self.state.library.selected_track = prev;
                self.state
                    .library
                    .tracks
                    .get(prev)
                    .map(|track| (track.path.clone(), track.duration_ms))
            },
            Route::Playlists => {
                let prev = self.state.playlists.selected_track.checked_sub(1)?;
                self.state.playlists.selected_track = prev;
                self.state
                    .playlists
                    .tracks
                    .get(prev)
                    .map(|track| (track.path.clone(), track.duration_ms))
            },
            _ => None,
        }
    }

    pub(super) async fn execute_global_search(&mut self, query: String) -> Result<()> {
        let q = query.trim().to_string();
        if q.is_empty() {
            self.state.library.search_query = None;
            self.refresh_library_tracks_only().await?;
            self.toast_info("cleared global search");
            return Ok(());
        }

        self.state.route = Route::Library;
        self.state.library.search_query = Some(q.clone());
        self.state.library.tracks = self.backend.search_tracks(q.clone()).await?;
        clamp_selection(
            &mut self.state.library.selected_track,
            self.state.library.tracks.len(),
        );
        self.toast_info(format!(
            "global search `{q}`: {} match(es)",
            self.state.library.tracks.len()
        ));
        Ok(())
    }

    pub(super) fn execute_inline_search(&mut self, raw_query: &str, forward: bool) {
        let query = raw_query.trim();
        if query.is_empty() {
            self.toast_warn("empty search query");
            return;
        }
        self.state.last_search_query = query.to_string();
        self.state.last_search_forward = forward;
        self.search_in_active_list(query, forward);
    }

    pub(super) fn repeat_last_search(&mut self, invert_direction: bool) {
        if self.state.last_search_query.is_empty() {
            self.toast_warn("no previous search");
            return;
        }
        let forward = if invert_direction {
            !self.state.last_search_forward
        } else {
            self.state.last_search_forward
        };
        let query = self.state.last_search_query.clone();
        self.search_in_active_list(&query, forward);
    }

    pub(super) fn jump_to_top(&mut self) {
        if self.active_list_len() == 0 {
            self.toast_warn("current list is empty");
            return;
        }
        self.set_active_list_selection(0);
        self.toast_info("jumped to top (gg)");
    }

    pub(super) fn jump_to_bottom(&mut self) {
        let len = self.active_list_len();
        if len == 0 {
            self.toast_warn("current list is empty");
            return;
        }
        self.set_active_list_selection(len - 1);
        self.toast_info("jumped to bottom (G)");
    }

    pub(super) fn search_in_active_list(&mut self, query: &str, forward: bool) {
        if let Some(index) = self.find_match_in_active_list(query, forward) {
            self.set_active_list_selection(index);
            self.toast_info(format!(
                "match {} `{query}`",
                if forward { "forward" } else { "backward" }
            ));
        } else {
            self.toast_warn(format!("not found: `{query}`"));
        }
    }

    pub(super) fn find_match_in_active_list(&self, query: &str, forward: bool) -> Option<usize> {
        let len = self.active_list_len();
        if len == 0 {
            return None;
        }

        let query_lower = query.to_lowercase();
        let start = self.active_list_selected_idx().min(len - 1);
        for step in 1..=len {
            let index = if forward {
                (start + step) % len
            } else {
                (start + len - (step % len)) % len
            };
            if self.active_item_matches(index, &query_lower) {
                return Some(index);
            }
        }
        None
    }

    pub(super) fn active_list_len(&self) -> usize {
        match self.state.route {
            Route::Library => self.state.library.tracks.len(),
            Route::Playlists => self.state.playlists.tracks.len(),
            Route::Plugins => self.state.plugins.installed.len(),
            Route::Settings => 4,
        }
    }

    pub(super) fn active_list_selected_idx(&self) -> usize {
        match self.state.route {
            Route::Library => self.state.library.selected_track,
            Route::Playlists => self.state.playlists.selected_track,
            Route::Plugins => self.state.plugins.selected,
            Route::Settings => self.state.settings.selected,
        }
    }

    pub(super) fn set_active_list_selection(&mut self, index: usize) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }
        let clamped = index.min(len - 1);
        match self.state.route {
            Route::Library => self.state.library.selected_track = clamped,
            Route::Playlists => self.state.playlists.selected_track = clamped,
            Route::Plugins => self.state.plugins.selected = clamped,
            Route::Settings => self.state.settings.selected = clamped,
        }
    }

    pub(super) fn active_item_matches(&self, index: usize, query_lower: &str) -> bool {
        match self.state.route {
            Route::Library => self
                .state
                .library
                .tracks
                .get(index)
                .is_some_and(|track| track_matches_query(track, query_lower)),
            Route::Playlists => self
                .state
                .playlists
                .tracks
                .get(index)
                .is_some_and(|track| track_matches_query(track, query_lower)),
            Route::Plugins => self
                .state
                .plugins
                .installed
                .get(index)
                .is_some_and(|plugin| {
                    text_matches_query(&plugin.display_name(), query_lower)
                        || text_matches_query(&plugin.id, query_lower)
                        || plugin
                            .install_state
                            .as_deref()
                            .is_some_and(|state| text_matches_query(state, query_lower))
                }),
            Route::Settings => text_matches_query(setting_label(index), query_lower),
        }
    }
}
