use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::{App, Route, command_mode_prefix};

impl App {
    pub(super) async fn handle_key(&mut self, key: KeyEvent) {
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

    pub(super) async fn handle_add_root_key(&mut self, key: KeyEvent) {
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
}
