use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use tokio::sync::mpsc;
use tracing::{error, info};

use stellatune_core::{LibraryCommand, LibraryEvent};

use crate::worker::{LibraryWorker, WorkerDeps};

#[derive(Clone)]
pub struct LibraryHandle {
    cmd_tx: Sender<LibraryCommand>,
    events: Arc<EventHub>,
}

impl LibraryHandle {
    pub fn send_command(&self, cmd: LibraryCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn subscribe_events(&self) -> Receiver<LibraryEvent> {
        self.events.subscribe()
    }
}

pub fn start_library(db_path: String) -> Result<LibraryHandle> {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<LibraryCommand>();
    let events = Arc::new(EventHub::new());
    let thread_events = Arc::clone(&events);
    let (init_tx, init_rx) = crossbeam_channel::bounded::<Result<()>>(1);

    thread::Builder::new()
        .name("stellatune-library".to_string())
        .spawn(move || {
            if let Err(e) = library_thread_main(db_path, cmd_rx, thread_events, init_tx) {
                error!("library thread exited with error: {e:?}");
            }
        })
        .expect("failed to spawn stellatune-library thread");

    init_rx.recv().context("library init channel closed")??;

    Ok(LibraryHandle { cmd_tx, events })
}

fn library_thread_main(
    db_path: String,
    cmd_rx: Receiver<LibraryCommand>,
    events: Arc<EventHub>,
    init_tx: Sender<Result<()>>,
) -> Result<()> {
    info!("library thread started");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .enable_io()
        .thread_name("stellatune-library-rt")
        .build()
        .context("failed to build tokio runtime")?;

    rt.block_on(async move {
        let (cmd_async_tx, mut cmd_async_rx) = mpsc::unbounded_channel::<LibraryCommand>();

        // Bridge crossbeam -> tokio so external callers don't depend on tokio.
        tokio::task::spawn_blocking(move || {
            for cmd in cmd_rx.iter() {
                if cmd_async_tx.send(cmd).is_err() {
                    break;
                }
            }
        });

        let db_path = PathBuf::from(db_path);
        if let Err(e) = ensure_parent_dir(&db_path) {
            let _ = init_tx.send(Err(e));
            return Ok::<_, anyhow::Error>(());
        }

        let deps = match WorkerDeps::new(&db_path, Arc::clone(&events)).await {
            Ok(v) => v,
            Err(e) => {
                let _ = init_tx.send(Err(e));
                return Ok::<_, anyhow::Error>(());
            }
        };
        let _ = init_tx.send(Ok(()));

        let mut worker = LibraryWorker::new(deps);

        while let Some(cmd) = cmd_async_rx.recv().await {
            let is_shutdown = matches!(cmd, LibraryCommand::Shutdown);
            if let Err(e) = worker.handle_command(cmd).await {
                events.emit(LibraryEvent::Error {
                    message: format!("{e:#}"),
                });
            }
            if is_shutdown {
                break;
            }
        }

        info!("library thread exiting");
        Ok::<_, anyhow::Error>(())
    })?;

    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create data dir: {}", parent.display()))?;
    Ok(())
}

pub(crate) struct EventHub {
    subscribers: std::sync::Mutex<Vec<Sender<LibraryEvent>>>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        Self {
            subscribers: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn subscribe(&self) -> Receiver<LibraryEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.subscribers
            .lock()
            .expect("event hub mutex poisoned")
            .push(tx);
        rx
    }

    pub(crate) fn emit(&self, event: LibraryEvent) {
        let mut subs = self.subscribers.lock().expect("event hub mutex poisoned");
        subs.retain(|tx| tx.send(event.clone()).is_ok());
    }
}
