use crossterm::event::{KeyCode, KeyEvent};

use super::App;
use super::commands::{Command, parse_command};

impl App {
    pub(super) async fn handle_command_key(&mut self, key: KeyEvent) {
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

    pub(super) async fn execute_command(&mut self, input: &str) {
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
}
