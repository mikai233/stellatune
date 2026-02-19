mod app;
mod backend;
mod cli;
mod paths;
mod ui;

use std::io;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, Event as CrosstermEvent};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tracing_subscriber::EnvFilter;

use app::{Action, App};
use backend::facade::BackendFacade;
use cli::Cli;
use paths::resolve_paths;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = resolve_paths(&cli)?;
    init_tui_tracing_to_file(paths.log_path.as_path())?;

    let backend = BackendFacade::new(&paths.db_path, &paths.plugins_dir, cli.page_size).await?;
    let mut app = App::new(backend);
    app.initialize().await;

    let mut terminal = init_terminal()?;
    let run_result = run_app_loop(&mut terminal, &mut app).await;
    let restore_result = restore_terminal(&mut terminal);
    let _ = stellatune_backend_api::runtime::runtime_shutdown().await;
    restore_result?;
    run_result
}

fn init_tui_tracing_to_file(log_path: &Path) -> Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .with_context(|| format!("open log file {}", log_path.display()))?;
    let writer = Arc::new(Mutex::new(file));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(move || FileLogWriter::new(Arc::clone(&writer)))
        .try_init();
    Ok(())
}

struct FileLogWriter {
    file: Arc<Mutex<std::fs::File>>,
}

impl FileLogWriter {
    fn new(file: Arc<Mutex<std::fs::File>>) -> Self {
        Self { file }
    }
}

impl Write for FileLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(mut guard) = self.file.lock() {
            guard.write_all(buf)?;
            return Ok(buf.len());
        }
        Err(io::Error::other("failed to lock log file"))
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Ok(mut guard) = self.file.lock() {
            guard.flush()?;
            return Ok(());
        }
        Err(io::Error::other("failed to lock log file"))
    }
}

async fn run_app_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let (action_tx, mut action_rx) = unbounded_channel::<Action>();

    spawn_input_task(action_tx.clone());
    spawn_player_event_task(action_tx.clone(), app.subscribe_player_events());
    match app.subscribe_library_events() {
        Ok(rx) => spawn_library_event_task(action_tx.clone(), rx),
        Err(error) => {
            app.state.status_line = format!("library event subscription failed: {error}");
        },
    }

    loop {
        app.on_tick();
        terminal.draw(|frame| ui::render(frame, app))?;
        if app.state.should_quit {
            break;
        }

        tokio::select! {
            maybe_action = action_rx.recv() => {
                if let Some(action) = maybe_action {
                    app.handle_action(action).await;
                } else {
                    break;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(120)) => {
                app.on_tick();
            }
        }
    }
    Ok(())
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn spawn_input_task(action_tx: UnboundedSender<Action>) {
    tokio::task::spawn_blocking(move || {
        loop {
            match event::poll(Duration::from_millis(100)) {
                Ok(true) => match event::read() {
                    Ok(CrosstermEvent::Key(key)) => {
                        if action_tx.send(Action::Key(key)).is_err() {
                            break;
                        }
                    },
                    Ok(_) => {},
                    Err(_) => break,
                },
                Ok(false) => {},
                Err(_) => break,
            }
        }
    });
}

fn spawn_player_event_task(
    action_tx: UnboundedSender<Action>,
    mut rx: tokio::sync::broadcast::Receiver<stellatune_audio::config::engine::Event>,
) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if action_tx.send(Action::EngineEvent(event)).is_err() {
                        break;
                    }
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn spawn_library_event_task(
    action_tx: UnboundedSender<Action>,
    mut rx: tokio::sync::broadcast::Receiver<stellatune_library::LibraryEvent>,
) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if action_tx.send(Action::LibraryEvent(event)).is_err() {
                        break;
                    }
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
