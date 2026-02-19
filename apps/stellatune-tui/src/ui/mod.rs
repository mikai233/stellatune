use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap};
use ratatui::{Frame, prelude::Rect};
use stellatune_library::TrackLite;

use crate::app::App;
use crate::app::state::{AppState, Route, ToastLevel};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let state = &app.state;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(4)])
        .split(frame.area());

    let sidebar_width = if state.sidebar_collapsed { 0 } else { 30 };
    let main_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(10)])
        .split(rows[0]);
    if !state.sidebar_collapsed && main_cols[0].width > 2 {
        render_sidebar(frame, main_cols[0], state);
    }
    render_main(frame, main_cols[1], app);
    render_now_playing_bar(frame, rows[1], state);
    render_add_root_modal(frame, state);
    render_command_modal(frame, state);
    render_toast(frame, state);
}

fn render_sidebar(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let routes = [
        ("1", "Library", Route::Library),
        ("2", "Playlists", Route::Playlists),
        ("3", "Plugins", Route::Plugins),
        ("4", "Settings", Route::Settings),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from("PAGES").style(Style::default().fg(Color::Yellow)));
    for (key, label, route) in routes {
        let selected = route == state.route;
        let row = format!("{} [{}] {}", if selected { ">" } else { " " }, key, label);
        lines.push(selected_line(row, selected));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("KEYMAP").style(Style::default().fg(Color::Yellow)));
    lines.push(Line::from("j/k: move"));
    lines.push(Line::from("Enter: activate"));
    lines.push(Line::from("m: queue selected"));
    lines.push(Line::from("J/K: next/prev"));
    lines.push(Line::from("Left/Right: seek +/-5s"));
    lines.push(Line::from("/ ?: search list"));
    lines.push(Line::from("n/N: next/prev match"));
    lines.push(Line::from("gg/G: top/bottom"));
    lines.push(Line::from("Tab: next page"));
    lines.push(Line::from("Ctrl-h/l: prev/next"));
    lines.push(Line::from("b: toggle sidebar"));
    lines.push(Line::from("a: add root"));
    lines.push(Line::from(": command prompt"));
    lines.push(Line::from("/ ?: search prompt"));
    lines.push(Line::from("q: quit"));

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .scroll((0, 0))
            .block(Block::default().borders(Borders::ALL).title("Sidebar")),
        area,
    );
}

fn render_main(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match app.state.route {
        Route::Library => render_library(frame, area, &app.state),
        Route::Playlists => render_playlists(frame, area, &app.state),
        Route::Plugins => render_plugins(frame, area, app),
        Route::Settings => render_settings(frame, area, &app.state),
    }
}

fn render_library(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);

    let roots = if state.library.roots.is_empty() {
        vec![
            Line::from("No library roots yet."),
            Line::from("Press `a` to start adding one."),
            Line::from("Or use command mode: `:root add <path>`."),
            Line::from("Example: `:root add D:\\Music`"),
        ]
    } else {
        state
            .library
            .roots
            .iter()
            .enumerate()
            .map(|(idx, path)| {
                selected_line(
                    format!(
                        "{} {path}",
                        if idx == state.library.selected_root {
                            ">"
                        } else {
                            " "
                        }
                    ),
                    idx == state.library.selected_root,
                )
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        Paragraph::new(Text::from(roots))
            .scroll((
                follow_scroll(
                    state.library.selected_root,
                    state.library.roots.len(),
                    columns[0],
                ),
                0,
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Roots ([ / ])"),
            ),
        columns[0],
    );

    let tracks = if state.library.tracks.is_empty() {
        vec![
            Line::from("No tracks loaded."),
            Line::from("After adding a root, run `scan` or press `s`."),
        ]
    } else {
        state
            .library
            .tracks
            .iter()
            .enumerate()
            .map(|(idx, track)| {
                selected_line(
                    format!(
                        "{} {}",
                        if idx == state.library.selected_track {
                            ">"
                        } else {
                            " "
                        },
                        format_track(track)
                    ),
                    idx == state.library.selected_track,
                )
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        Paragraph::new(Text::from(tracks))
            .scroll((
                follow_scroll(
                    state.library.selected_track,
                    state.library.tracks.len(),
                    columns[1],
                ),
                0,
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(library_track_title(state)),
            ),
        columns[1],
    );
}

fn render_playlists(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(area);

    let left = state
        .playlists
        .playlists
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            selected_line(
                format!(
                    "{} {} ({})",
                    if idx == state.playlists.selected_playlist {
                        ">"
                    } else {
                        " "
                    },
                    item.name,
                    item.track_count
                ),
                idx == state.playlists.selected_playlist,
            )
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(Text::from(left))
            .scroll((
                follow_scroll(
                    state.playlists.selected_playlist,
                    state.playlists.playlists.len(),
                    cols[0],
                ),
                0,
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Playlists ([ / ])"),
            ),
        cols[0],
    );

    let right = state
        .playlists
        .tracks
        .iter()
        .enumerate()
        .map(|(idx, track)| {
            selected_line(
                format!(
                    "{} {}",
                    if idx == state.playlists.selected_track {
                        ">"
                    } else {
                        " "
                    },
                    format_track(track)
                ),
                idx == state.playlists.selected_track,
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Text::from(right))
            .scroll((
                follow_scroll(
                    state.playlists.selected_track,
                    state.playlists.tracks.len(),
                    cols[1],
                ),
                0,
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Playlist Tracks (j/k + Enter, /? search, gg/G jump)"),
            ),
        cols[1],
    );
}

fn render_plugins(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lines = app
        .state
        .plugins
        .installed
        .iter()
        .enumerate()
        .map(|(idx, plugin)| {
            let runtime_status = App::plugin_status(
                &app.state.plugins.disabled_ids,
                &app.state.plugins.active_ids,
                &plugin.id,
            );
            let install_state = plugin.install_state.as_deref().unwrap_or("unknown");
            selected_line(
                format!(
                    "{} {} [{} | {}]",
                    if idx == app.state.plugins.selected {
                        ">"
                    } else {
                        " "
                    },
                    plugin.display_name(),
                    runtime_status,
                    install_state
                ),
                idx == app.state.plugins.selected,
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .scroll((
                follow_scroll(
                    app.state.plugins.selected,
                    app.state.plugins.installed.len(),
                    area,
                ),
                0,
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Plugins (Enter toggle enable/disable)"),
            ),
        area,
    );
}

fn render_settings(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let lines = vec![
        selected_line(
            format!(
                "{} Resample Quality: {}",
                if state.settings.selected == 0 {
                    ">"
                } else {
                    " "
                },
                render_resample_quality(state.settings.resample_quality)
            ),
            state.settings.selected == 0,
        ),
        selected_line(
            format!(
                "{} Match Track Sample Rate: {}",
                if state.settings.selected == 1 {
                    ">"
                } else {
                    " "
                },
                on_off_text(state.settings.match_track_sample_rate)
            ),
            state.settings.selected == 1,
        ),
        selected_line(
            format!(
                "{} Gapless Playback: {}",
                if state.settings.selected == 2 {
                    ">"
                } else {
                    " "
                },
                on_off_text(state.settings.gapless_playback)
            ),
            state.settings.selected == 2,
        ),
        selected_line(
            format!(
                "{} Seek Track Fade: {}",
                if state.settings.selected == 3 {
                    ">"
                } else {
                    " "
                },
                on_off_text(state.settings.seek_track_fade)
            ),
            state.settings.selected == 3,
        ),
        Line::from(""),
        Line::from("Use j/k to select setting item."),
        Line::from("Use h/l or Left/Right to adjust current item."),
        Line::from("Tip: choose Fast/Balanced if playback stutters."),
    ];

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Settings (audio output)"),
            ),
        area,
    );
}

fn render_now_playing_bar(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Now Playing")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let track_name = pretty_track_name(&state.playback.current_track_display);
    let (badge, badge_color) = playback_state_badge(state);
    let queue_badge = if state.queue.is_empty() {
        "Q0".to_string()
    } else if let Some(cursor) = state.queue_index {
        format!("Q{}/{}", cursor + 1, state.queue.len())
    } else {
        format!("Q{}", state.queue.len())
    };

    let top = Line::from(vec![
        Span::styled(
            format!(" {badge} "),
            Style::default().fg(Color::Black).bg(badge_color),
        ),
        Span::raw(" "),
        Span::styled(
            format!(" {queue_badge} "),
            Style::default().fg(Color::Black).bg(Color::Blue),
        ),
        Span::raw(" "),
        Span::styled(
            ellipsize(&track_name, rows[0].width.saturating_sub(14) as usize),
            Style::default().fg(Color::White),
        ),
    ]);
    frame.render_widget(Paragraph::new(top), rows[0]);

    let current = format_duration_ms(state.playback.position_ms);
    let total = format_optional_duration_ms(state.playback.duration_ms);
    frame.render_widget(
        Gauge::default()
            .ratio(playback_progress_ratio(
                state.playback.position_ms,
                state.playback.duration_ms,
            ))
            .label(format!("{current} / {total}"))
            .gauge_style(
                Style::default()
                    .fg(playback_state_color(state))
                    .bg(Color::DarkGray),
            )
            .use_unicode(true),
        rows[1],
    );
}

fn render_command_modal(frame: &mut Frame<'_>, state: &AppState) {
    if !state.command_mode {
        return;
    }

    let area = centered_rect(72, 34, frame.area());
    frame.render_widget(Clear, area);

    let title = if state.command_prefix == ':' {
        "Command"
    } else {
        "Search"
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    let input_help = if state.command_prefix == ':' {
        "Enter run | Tab accept | Up/Down select | Esc cancel"
    } else {
        "Enter search | Tab accept | Up/Down select | Esc cancel"
    };

    let input_block = Block::default().borders(Borders::ALL).title("Input");
    let input_inner = input_block.inner(rows[0]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("{}{}_", state.command_prefix, state.command_input)),
            Line::from(input_help).style(Style::default().fg(Color::Gray)),
        ])
        .block(input_block),
        rows[0],
    );

    let lines = if state.command_suggestions.is_empty() {
        vec![Line::from("No completions").style(Style::default().fg(Color::DarkGray))]
    } else {
        state
            .command_suggestions
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                selected_line(
                    format!(
                        "{} {}",
                        if idx == state.command_suggestion_index {
                            ">"
                        } else {
                            " "
                        },
                        item.display
                    ),
                    idx == state.command_suggestion_index,
                )
            })
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .scroll((
                follow_scroll(
                    state.command_suggestion_index,
                    state.command_suggestions.len(),
                    rows[1],
                ),
                0,
            ))
            .block(Block::default().borders(Borders::ALL).title("Completions")),
        rows[1],
    );

    if input_inner.height > 0 {
        let cursor_x = input_inner
            .x
            .saturating_add(1 + state.command_input.chars().count() as u16)
            .min(
                input_inner
                    .x
                    .saturating_add(input_inner.width.saturating_sub(1)),
            );
        frame.set_cursor_position((cursor_x, input_inner.y));
    }
}

fn render_add_root_modal(frame: &mut Frame<'_>, state: &AppState) {
    if !state.add_root_mode {
        return;
    }

    let area = centered_rect(76, 24, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Add Library Root");
    let inner = block.inner(area);

    let lines = vec![
        Line::from("Type an absolute folder path and press Enter."),
        Line::from("Esc to cancel."),
        Line::from(""),
        Line::from(format!("> {}_", state.add_root_input)),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(Color::White).bg(Color::DarkGray))
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );

    if inner.height > 0 {
        let cursor_x = inner
            .x
            .saturating_add(2 + state.add_root_input.chars().count() as u16)
            .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
        let cursor_y = inner.y.saturating_add(3);
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn format_track(track: &TrackLite) -> String {
    let title = track.title.as_deref().unwrap_or("-");
    let artist = track.artist.as_deref().unwrap_or("-");
    let dur = track
        .duration_ms
        .map(format_duration_ms)
        .unwrap_or_else(|| "--:--".to_string());
    format!("{title} - {artist} [{dur}]")
}

fn format_duration_ms(ms: i64) -> String {
    let total = if ms < 0 { 0 } else { ms / 1000 };
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        return format!("{hours:02}:{minutes:02}:{seconds:02}");
    }
    format!("{minutes:02}:{seconds:02}")
}

fn format_optional_duration_ms(ms: Option<i64>) -> String {
    ms.filter(|value| *value > 0)
        .map(format_duration_ms)
        .unwrap_or_else(|| "--:--".to_string())
}

fn playback_progress_ratio(position_ms: i64, duration_ms: Option<i64>) -> f64 {
    let Some(total_ms) = duration_ms.filter(|value| *value > 0) else {
        return 0.0;
    };
    let current_ms = position_ms.max(0).min(total_ms);
    current_ms as f64 / total_ms as f64
}

fn pretty_track_name(track_display: &str) -> String {
    if track_display.trim().is_empty() || track_display == "-" {
        return "Nothing playing".to_string();
    }
    let path = std::path::Path::new(track_display);
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| track_display.to_string())
}

fn playback_state_badge(state: &AppState) -> (&'static str, Color) {
    match state.playback.player_state {
        stellatune_audio::config::engine::PlayerState::Playing => ("PLAY", Color::Green),
        stellatune_audio::config::engine::PlayerState::Paused => ("PAUSE", Color::Yellow),
        stellatune_audio::config::engine::PlayerState::Stopped => ("STOP", Color::DarkGray),
    }
}

fn playback_state_color(state: &AppState) -> Color {
    match state.playback.player_state {
        stellatune_audio::config::engine::PlayerState::Playing => Color::Cyan,
        stellatune_audio::config::engine::PlayerState::Paused => Color::Yellow,
        stellatune_audio::config::engine::PlayerState::Stopped => Color::DarkGray,
    }
}

fn ellipsize(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars = input.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return input.to_string();
    }
    let take = max_chars.saturating_sub(3);
    let prefix = chars.into_iter().take(take).collect::<String>();
    format!("{prefix}...")
}

fn selected_line(text: String, selected: bool) -> Line<'static> {
    if selected {
        Line::from(text).style(Style::default().fg(Color::Black).bg(Color::Cyan))
    } else {
        Line::from(text)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn follow_scroll(selected: usize, total_items: usize, area: Rect) -> u16 {
    let content_height = area.height.saturating_sub(2) as usize;
    if content_height == 0 || total_items <= content_height {
        return 0;
    }

    // Keep the focused row around the viewport center whenever possible.
    let half = content_height / 2;
    let desired = selected.saturating_sub(half);
    let max_scroll = total_items.saturating_sub(content_height);
    desired.min(max_scroll).min(u16::MAX as usize) as u16
}

fn render_resample_quality(
    quality: stellatune_audio::config::engine::ResampleQuality,
) -> &'static str {
    match quality {
        stellatune_audio::config::engine::ResampleQuality::Fast => "Fast",
        stellatune_audio::config::engine::ResampleQuality::Balanced => "Balanced",
        stellatune_audio::config::engine::ResampleQuality::High => "High",
        stellatune_audio::config::engine::ResampleQuality::Ultra => "Ultra",
    }
}

fn on_off_text(v: bool) -> &'static str {
    if v { "On" } else { "Off" }
}

fn library_track_title(state: &AppState) -> String {
    if let Some(query) = &state.library.search_query {
        return format!(
            "Global Search `{}` (j/k + Enter, /? local search)",
            ellipsize(query, 24)
        );
    }
    "Library Tracks (j/k + Enter, /? search, gg/G jump)".to_string()
}

fn render_toast(frame: &mut Frame<'_>, state: &AppState) {
    if state.command_mode || state.add_root_mode {
        return;
    }
    let Some(toast) = &state.toast else {
        return;
    };

    let max_width = frame.area().width.saturating_sub(2);
    let max_height = frame.area().height.saturating_sub(2);
    if max_width < 8 || max_height < 3 {
        return;
    }
    let width = max_width.min(56).max(20.min(max_width));
    let height = 3.min(max_height);
    let x = frame.area().x + frame.area().width.saturating_sub(width + 1);
    let y = frame.area().y + frame.area().height.saturating_sub(height + 1);
    let area = Rect {
        x,
        y,
        width,
        height,
    };

    let (title, color) = match toast.level {
        ToastLevel::Info => ("Info", Color::Cyan),
        ToastLevel::Warn => ("Warn", Color::Yellow),
        ToastLevel::Error => ("Error", Color::Red),
    };

    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(ellipsize(&toast.message, width.saturating_sub(4) as usize))
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(color))
                    .title(title),
            ),
        area,
    );
}
