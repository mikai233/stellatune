mod commands;
pub mod state;

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use stellatune_audio::config::engine::{Event as AudioEvent, ResampleQuality};
use stellatune_library::{LibraryEvent, TrackLite};

use crate::backend::facade::BackendFacade;
use crate::backend::track_token::decode_track_token_path;

use self::commands::{Command, build_command_suggestions, parse_command};
use self::state::{
    AppState, QueueItem, Route, ToastLevel, ToastState, clamp_selection, select_next, select_prev,
};

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
            self.state.playback.position_ms = snapshot.position_ms;
            let track_display = snapshot
                .current_track
                .as_deref()
                .map(decode_track_token_path)
                .unwrap_or_else(|| "-".to_string());
            self.state.playback.current_track_display = track_display.clone();
            self.state.playback.duration_ms = self.lookup_track_duration_hint(&track_display);
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

    async fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if self.state.add_root_mode {
            self.handle_add_root_key(key).await;
            return;
        }
        if self.state.command_mode {
            self.handle_command_key(key).await;
            return;
        }

        if let Some(prefix) = command_mode_prefix(&key) {
            self.open_command_mode(prefix);
            return;
        }

        if key.modifiers.is_empty() && matches!(key.code, KeyCode::Char('g')) {
            if self.state.pending_g {
                self.jump_to_top();
                self.state.pending_g = false;
            } else {
                self.state.pending_g = true;
                self.state.status_line = "g".to_string();
            }
            return;
        }

        self.state.pending_g = false;

        match key.code {
            KeyCode::Char('q') => self.state.should_quit = true,
            KeyCode::Tab => self.state.route = self.state.route.next(),
            KeyCode::BackTab => self.state.route = self.state.route.prev(),
            KeyCode::Char('b') | KeyCode::Char('B') => {
                self.state.sidebar_collapsed = !self.state.sidebar_collapsed;
                self.state.status_line = if self.state.sidebar_collapsed {
                    "sidebar collapsed".to_string()
                } else {
                    "sidebar expanded".to_string()
                };
            },
            KeyCode::Char('1') => self.state.route = Route::Library,
            KeyCode::Char('2') => self.state.route = Route::Playlists,
            KeyCode::Char('3') => self.state.route = Route::Plugins,
            KeyCode::Char('4') => self.state.route = Route::Settings,
            KeyCode::Char('m') => self.enqueue_selected_track(),
            KeyCode::Char(' ') => {
                let result = self.backend.toggle_play_pause().await;
                self.try_action("toggle play/pause", result);
            },
            KeyCode::Char('x') => {
                let result = self.backend.stop().await;
                self.try_action("stop", result);
            },
            KeyCode::Char('r') => {
                let result = self.refresh_all().await;
                self.try_action("refresh", result);
            },
            KeyCode::Char('s') => {
                let result = self.backend.scan_all(false).await;
                self.try_action("scan", result);
            },
            KeyCode::Char('S') => {
                let result = self.backend.scan_all(true).await;
                self.try_action("force scan", result);
            },
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.state.add_root_mode = true;
                self.state.add_root_input.clear();
            },
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.route = self.state.route.prev();
            },
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.route = self.state.route.next();
            },
            KeyCode::Left => {
                if self.state.route == Route::Settings {
                    self.adjust_settings(false).await;
                } else if let Err(error) = self.seek_by_ms(-5_000).await {
                    self.toast_error(format!("seek failed: {error}"));
                }
            },
            KeyCode::Right => {
                if self.state.route == Route::Settings {
                    self.adjust_settings(true).await;
                } else if let Err(error) = self.seek_by_ms(5_000).await {
                    self.toast_error(format!("seek failed: {error}"));
                }
            },
            KeyCode::Char('h') => self.adjust_settings(false).await,
            KeyCode::Char('l') => self.adjust_settings(true).await,
            KeyCode::Char('j') | KeyCode::Down => self.select_down(),
            KeyCode::Char('k') | KeyCode::Up => self.select_up(),
            KeyCode::Char('J') => {
                if let Err(error) = self.play_next_track().await {
                    self.toast_error(format!("next failed: {error}"));
                }
            },
            KeyCode::Char('K') => {
                if let Err(error) = self.play_prev_track().await {
                    self.toast_error(format!("prev failed: {error}"));
                }
            },
            KeyCode::Char('n') => self.repeat_last_search(false),
            KeyCode::Char('N') => self.repeat_last_search(true),
            KeyCode::Char('G') => self.jump_to_bottom(),
            KeyCode::Char('[') => self.select_prev_group().await,
            KeyCode::Char(']') => self.select_next_group().await,
            KeyCode::Enter => self.activate_selected().await,
            _ => {},
        }
    }

    async fn handle_add_root_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.state.add_root_mode = false;
                self.state.add_root_input.clear();
                self.toast_info("add root canceled");
            },
            KeyCode::Backspace => {
                self.state.add_root_input.pop();
            },
            KeyCode::Enter => {
                let path = self.state.add_root_input.trim().to_string();
                if path.is_empty() {
                    self.toast_warn("root path is empty");
                    return;
                }
                match self.backend.add_root(path.clone()).await {
                    Ok(()) => {
                        self.state.add_root_mode = false;
                        self.state.add_root_input.clear();
                        self.state.route = Route::Library;
                        if let Err(error) = self.refresh_library().await {
                            self.toast_warn(format!("root added but refresh failed: {error}"));
                        } else {
                            self.toast_info(format!("root added: {path} (press s to scan)"));
                        }
                    },
                    Err(error) => {
                        self.toast_error(format!("add root failed: {error}"));
                    },
                }
            },
            KeyCode::Char(c) => {
                self.state.add_root_input.push(c);
            },
            _ => {},
        }
    }

    fn handle_engine_event(&mut self, event: AudioEvent) {
        match event {
            AudioEvent::StateChanged { state } => {
                self.state.playback.player_state = state;
            },
            AudioEvent::TrackChanged { track_token } => {
                let track_display = decode_track_token_path(&track_token);
                self.state.playback.current_track_display = track_display.clone();
                self.state.playback.position_ms = 0;
                self.state.playback.duration_ms = self.lookup_track_duration_hint(&track_display);
                self.sync_queue_cursor_with_path(&track_display);
            },
            AudioEvent::Position { position_ms } => {
                self.state.playback.position_ms = position_ms;
            },
            AudioEvent::Error { message } => {
                self.toast_error(format!("playback error: {message}"));
            },
            AudioEvent::Recovering {
                attempt,
                backoff_ms,
            } => {
                self.toast_warn(format!(
                    "recovering output stream (attempt={attempt}, backoff_ms={backoff_ms})"
                ));
            },
            AudioEvent::AudioStart => {
                self.toast_info("audio started");
            },
            AudioEvent::AudioEnd => {
                self.toast_info("audio ended");
            },
            AudioEvent::Eof => {
                self.toast_info("track reached end");
                self.state.playback.position_ms = 0;
            },
            AudioEvent::VolumeChanged { .. } => {},
        }
    }

    async fn handle_library_event(&mut self, event: LibraryEvent) {
        match event {
            LibraryEvent::Changed => {
                if let Err(error) = self.refresh_library().await {
                    self.toast_error(format!("library refresh failed: {error}"));
                }
            },
            LibraryEvent::ScanProgress {
                scanned,
                updated,
                skipped,
                errors,
            } => {
                self.state.library.scan_progress = Some(format!(
                    "scanned={scanned}, updated={updated}, skipped={skipped}, errors={errors}"
                ));
            },
            LibraryEvent::ScanFinished {
                duration_ms,
                scanned,
                updated,
                skipped,
                errors,
            } => {
                self.state.library.scan_progress = None;
                self.state.status_line = format!(
                    "scan finished in {duration_ms}ms (scanned={scanned}, updated={updated}, skipped={skipped}, errors={errors})"
                );
                if let Err(error) = self.refresh_library().await {
                    self.state.status_line = format!("library refresh failed: {error}");
                }
            },
            LibraryEvent::Error { message } => {
                self.toast_error(format!("library error: {message}"));
            },
            LibraryEvent::Log { message } => {
                self.state.status_line = message;
            },
        }
    }

    async fn handle_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.state.command_mode = false;
                self.state.command_prefix = ':';
                self.state.command_input.clear();
                self.state.command_suggestions.clear();
                self.state.command_suggestion_index = 0;
            },
            KeyCode::Backspace => {
                self.state.command_input.pop();
                self.refresh_command_suggestions();
            },
            KeyCode::Tab => {
                self.accept_selected_command_suggestion();
            },
            KeyCode::Up => self.select_prev_command_suggestion(),
            KeyCode::Down => self.select_next_command_suggestion(),
            KeyCode::Enter => {
                let input = self.state.command_input.trim().to_string();
                let prefix = self.state.command_prefix;
                self.state.command_mode = false;
                self.state.command_prefix = ':';
                self.state.command_input.clear();
                self.state.command_suggestions.clear();
                self.state.command_suggestion_index = 0;
                match prefix {
                    ':' => self.execute_command(&input).await,
                    '/' => self.execute_inline_search(&input, true),
                    '?' => self.execute_inline_search(&input, false),
                    _ => self.toast_error(format!("unsupported command prefix: {prefix}")),
                }
            },
            KeyCode::Char(c) => {
                self.state.command_input.push(c);
                self.refresh_command_suggestions();
            },
            _ => {},
        }
    }

    async fn execute_command(&mut self, input: &str) {
        let command = match parse_command(input) {
            Ok(command) => command,
            Err(error) => {
                self.toast_error(error);
                return;
            },
        };
        let should_toast_success =
            !matches!(&command, Command::Help | Command::QueueShow | Command::Quit);

        let result = match command {
            Command::Help => {
                self.toast_info("commands: help | refresh | search <q> | scan | root add/rm <path> | play <path> | seek <ms|+ms|-ms|10s> | next/prev | queue add/add-current/show/clear | playlist ... | plugin ...");
                Ok(())
            },
            Command::Quit => {
                self.state.should_quit = true;
                Ok(())
            },
            Command::Refresh => self.refresh_all().await,
            Command::Search { query } => self.execute_global_search(query).await,
            Command::Scan { force } => self.backend.scan_all(force).await,
            Command::RootAdd { path } => self.backend.add_root(path).await,
            Command::RootRemove { path } => self.backend.remove_root(path).await,
            Command::Play { path } => self.play_track_with_hint(path, None).await,
            Command::SeekTo { position_ms } => self.seek_to_ms(position_ms).await,
            Command::SeekBy { delta_ms } => self.seek_by_ms(delta_ms).await,
            Command::Next => self.play_next_track().await,
            Command::Prev => self.play_prev_track().await,
            Command::QueueAdd { path } => {
                self.enqueue_path(path, None);
                Ok(())
            },
            Command::QueueAddCurrent => {
                self.enqueue_selected_track();
                Ok(())
            },
            Command::QueueClear => {
                self.state.queue.clear();
                self.state.queue_index = None;
                Ok(())
            },
            Command::QueueShow => {
                let size = self.state.queue.len();
                let current = self
                    .state
                    .queue_index
                    .map(|idx| idx + 1)
                    .unwrap_or_default();
                self.toast_info(format!("queue: {size} track(s), cursor={current}"));
                Ok(())
            },
            Command::PlaylistCreate { name } => self.backend.create_playlist(name).await,
            Command::PlaylistRename { id, name } => self.backend.rename_playlist(id, name).await,
            Command::PlaylistDelete { id } => self.backend.delete_playlist(id).await,
            Command::PlaylistAddTrack {
                playlist_id,
                track_id,
            } => {
                self.backend
                    .add_track_to_playlist(playlist_id, track_id)
                    .await
            },
            Command::PlaylistRemoveTrack {
                playlist_id,
                track_id,
            } => {
                self.backend
                    .remove_track_from_playlist(playlist_id, track_id)
                    .await
            },
            Command::PluginInstall { artifact_path } => self
                .backend
                .plugins_install_from_file(artifact_path)
                .await
                .map(|_| ()),
            Command::PluginUninstall { plugin_id } => {
                self.backend.plugins_uninstall_by_id(plugin_id).await
            },
            Command::PluginEnable { plugin_id } => self.backend.plugin_enable(plugin_id).await,
            Command::PluginDisable { plugin_id } => self.backend.plugin_disable(plugin_id).await,
            Command::PluginApply => self.backend.plugin_apply_state().await,
        };

        match result {
            Ok(()) => {
                if should_toast_success {
                    self.toast_info(format!("ok: {input}"));
                }
                if let Err(error) = self.refresh_all().await {
                    self.toast_error(format!("refresh failed after command: {error}"));
                }
            },
            Err(error) => {
                self.toast_error(format!("command failed: {error}"));
            },
        }
    }

    async fn activate_selected(&mut self) {
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

    fn select_up(&mut self) {
        match self.state.route {
            Route::Library => select_prev(&mut self.state.library.selected_track),
            Route::Playlists => select_prev(&mut self.state.playlists.selected_track),
            Route::Plugins => select_prev(&mut self.state.plugins.selected),
            Route::Settings => select_prev(&mut self.state.settings.selected),
        }
    }

    fn select_down(&mut self) {
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

    async fn select_prev_group(&mut self) {
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

    async fn select_next_group(&mut self) {
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

    fn open_command_mode(&mut self, prefix: char) {
        self.state.command_mode = true;
        self.state.command_prefix = prefix;
        self.state.command_input.clear();
        self.state.pending_g = false;
        self.state.command_suggestion_index = 0;
        self.refresh_command_suggestions();
    }

    fn refresh_command_suggestions(&mut self) {
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

    fn select_prev_command_suggestion(&mut self) {
        select_prev(&mut self.state.command_suggestion_index);
    }

    fn select_next_command_suggestion(&mut self) {
        select_next(
            &mut self.state.command_suggestion_index,
            self.state.command_suggestions.len(),
        );
    }

    fn accept_selected_command_suggestion(&mut self) {
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

    fn toast_with_level(&mut self, level: ToastLevel, message: impl Into<String>) {
        let message = message.into();
        self.state.status_line = message.clone();
        self.state.toast = Some(ToastState {
            message,
            level,
            expires_at: Instant::now() + Self::TOAST_TTL,
        });
    }

    fn toast_info(&mut self, message: impl Into<String>) {
        self.toast_with_level(ToastLevel::Info, message);
    }

    fn toast_warn(&mut self, message: impl Into<String>) {
        self.toast_with_level(ToastLevel::Warn, message);
    }

    fn toast_error(&mut self, message: impl Into<String>) {
        self.toast_with_level(ToastLevel::Error, message);
    }

    fn enqueue_path(&mut self, path: String, duration_ms: Option<i64>) {
        let item = QueueItem {
            path: path.clone(),
            duration_ms: duration_ms.filter(|value| *value > 0),
        };
        self.state.queue.push(item);
        self.toast_info(format!("queued: {path}"));
    }

    fn enqueue_selected_track(&mut self) {
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

    async fn seek_to_ms(&mut self, position_ms: i64) -> Result<()> {
        self.backend.seek_ms(position_ms.max(0)).await?;
        self.state.playback.position_ms = position_ms.max(0);
        Ok(())
    }

    async fn seek_by_ms(&mut self, delta_ms: i64) -> Result<()> {
        let target = self
            .state
            .playback
            .position_ms
            .saturating_add(delta_ms)
            .max(0);
        self.seek_to_ms(target).await
    }

    async fn play_next_track(&mut self) -> Result<()> {
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

    async fn play_prev_track(&mut self) -> Result<()> {
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

    fn next_queue_item(&self) -> Option<(usize, QueueItem)> {
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

    fn prev_queue_item(&self) -> Option<(usize, QueueItem)> {
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

    fn next_track_from_active_list(&mut self) -> Option<(String, Option<i64>)> {
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

    fn prev_track_from_active_list(&mut self) -> Option<(String, Option<i64>)> {
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

    async fn execute_global_search(&mut self, query: String) -> Result<()> {
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

    fn execute_inline_search(&mut self, raw_query: &str, forward: bool) {
        let query = raw_query.trim();
        if query.is_empty() {
            self.toast_warn("empty search query");
            return;
        }
        self.state.last_search_query = query.to_string();
        self.state.last_search_forward = forward;
        self.search_in_active_list(query, forward);
    }

    fn repeat_last_search(&mut self, invert_direction: bool) {
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

    fn jump_to_top(&mut self) {
        if self.active_list_len() == 0 {
            self.toast_warn("current list is empty");
            return;
        }
        self.set_active_list_selection(0);
        self.toast_info("jumped to top (gg)");
    }

    fn jump_to_bottom(&mut self) {
        let len = self.active_list_len();
        if len == 0 {
            self.toast_warn("current list is empty");
            return;
        }
        self.set_active_list_selection(len - 1);
        self.toast_info("jumped to bottom (G)");
    }

    fn search_in_active_list(&mut self, query: &str, forward: bool) {
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

    fn find_match_in_active_list(&self, query: &str, forward: bool) -> Option<usize> {
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

    fn active_list_len(&self) -> usize {
        match self.state.route {
            Route::Library => self.state.library.tracks.len(),
            Route::Playlists => self.state.playlists.tracks.len(),
            Route::Plugins => self.state.plugins.installed.len(),
            Route::Settings => 4,
        }
    }

    fn active_list_selected_idx(&self) -> usize {
        match self.state.route {
            Route::Library => self.state.library.selected_track,
            Route::Playlists => self.state.playlists.selected_track,
            Route::Plugins => self.state.plugins.selected,
            Route::Settings => self.state.settings.selected,
        }
    }

    fn set_active_list_selection(&mut self, index: usize) {
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

    fn active_item_matches(&self, index: usize, query_lower: &str) -> bool {
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

    async fn adjust_settings(&mut self, forward: bool) {
        if self.state.route != Route::Settings {
            return;
        }

        match self.state.settings.selected {
            0 => {
                self.state.settings.resample_quality =
                    next_resample_quality(self.state.settings.resample_quality, forward);
            },
            1 => {
                self.state.settings.match_track_sample_rate =
                    !self.state.settings.match_track_sample_rate;
            },
            2 => {
                self.state.settings.gapless_playback = !self.state.settings.gapless_playback;
            },
            3 => {
                self.state.settings.seek_track_fade = !self.state.settings.seek_track_fade;
            },
            _ => {},
        }

        match self.apply_audio_settings().await {
            Ok(()) => {
                self.state.status_line = format!(
                    "audio settings applied: quality={} match={} gapless={} seek_fade={}",
                    format_resample_quality(self.state.settings.resample_quality),
                    on_off(self.state.settings.match_track_sample_rate),
                    on_off(self.state.settings.gapless_playback),
                    on_off(self.state.settings.seek_track_fade),
                );
            },
            Err(error) => {
                self.state.status_line = format!("apply audio settings failed: {error}");
            },
        }
    }

    async fn refresh_all(&mut self) -> Result<()> {
        self.refresh_library().await?;
        self.refresh_playlists().await?;
        self.refresh_plugins().await?;
        Ok(())
    }

    async fn refresh_library(&mut self) -> Result<()> {
        self.state.library.roots = self.backend.list_roots().await?;
        clamp_selection(
            &mut self.state.library.selected_root,
            self.state.library.roots.len(),
        );
        self.refresh_library_tracks_only().await
    }

    async fn refresh_library_tracks_only(&mut self) -> Result<()> {
        let folder = self.state.library.current_root();
        self.state.library.tracks = self.backend.list_tracks(folder, String::new()).await?;
        self.state.library.search_query = None;
        clamp_selection(
            &mut self.state.library.selected_track,
            self.state.library.tracks.len(),
        );
        self.refresh_playback_duration_hint();
        Ok(())
    }

    async fn refresh_playlists(&mut self) -> Result<()> {
        self.state.playlists.playlists = self.backend.list_playlists().await?;
        clamp_selection(
            &mut self.state.playlists.selected_playlist,
            self.state.playlists.playlists.len(),
        );
        self.refresh_playlist_tracks().await
    }

    async fn refresh_playlist_tracks(&mut self) -> Result<()> {
        let playlist_id = self
            .state
            .playlists
            .current_playlist()
            .map(|p| p.id)
            .unwrap_or_default();
        self.state.playlists.tracks = if playlist_id <= 0 {
            Vec::new()
        } else {
            self.backend
                .list_playlist_tracks(playlist_id, String::new())
                .await?
        };
        clamp_selection(
            &mut self.state.playlists.selected_track,
            self.state.playlists.tracks.len(),
        );
        self.refresh_playback_duration_hint();
        Ok(())
    }

    async fn refresh_plugins(&mut self) -> Result<()> {
        self.state.plugins.installed = self.backend.plugins_list_installed().await?;
        self.state.plugins.disabled_ids = self.backend.list_disabled_plugin_ids().await?;
        self.state.plugins.active_ids =
            self.backend.active_plugin_ids().await.into_iter().collect();
        clamp_selection(
            &mut self.state.plugins.selected,
            self.state.plugins.installed.len(),
        );
        Ok(())
    }

    fn try_action(&mut self, label: &str, result: Result<()>) {
        match result {
            Ok(()) => self.toast_info(format!("ok: {label}")),
            Err(error) => self.toast_error(format!("{label} failed: {error}")),
        }
    }

    async fn play_track_with_hint(&mut self, path: String, duration_ms: Option<i64>) -> Result<()> {
        let result = self.backend.play_track_path(&path).await;
        if result.is_ok() {
            self.state.playback.current_track_display = path.clone();
            self.state.playback.position_ms = 0;
            self.state.playback.duration_ms = duration_ms
                .filter(|value| *value > 0)
                .or_else(|| self.lookup_track_duration_hint(&path));
            self.sync_queue_cursor_with_path(&path);
        }
        result
    }

    fn refresh_playback_duration_hint(&mut self) {
        if self.state.playback.duration_ms.is_some() {
            return;
        }
        self.state.playback.duration_ms =
            self.lookup_track_duration_hint(&self.state.playback.current_track_display);
    }

    fn lookup_track_duration_hint(&self, track_path: &str) -> Option<i64> {
        self.state
            .library
            .tracks
            .iter()
            .chain(self.state.playlists.tracks.iter())
            .find(|track| track_paths_match(&track.path, track_path))
            .and_then(|track| track.duration_ms.filter(|value| *value > 0))
    }

    fn sync_queue_cursor_with_path(&mut self, track_path: &str) {
        self.state.queue_index = self
            .state
            .queue
            .iter()
            .position(|item| track_paths_match(&item.path, track_path));
    }

    pub fn plugin_status(
        disabled_ids: &HashSet<String>,
        active_ids: &HashSet<String>,
        id: &str,
    ) -> &'static str {
        if disabled_ids.contains(id) {
            "disabled"
        } else if active_ids.contains(id) {
            "enabled"
        } else {
            "installed"
        }
    }

    async fn apply_audio_settings(&self) -> Result<()> {
        self.backend
            .set_audio_output_settings(
                self.state.settings.match_track_sample_rate,
                self.state.settings.resample_quality,
                self.state.settings.gapless_playback,
                self.state.settings.seek_track_fade,
            )
            .await
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
