use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::runtime::{Handle, Runtime};
#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    RequestBootstrapSnapshot,
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    BootstrapSnapshotReady,
    Heartbeat { tick: u64 },
}

#[derive(Clone)]
pub struct RuntimeServices {
    handle: Handle,
    command_tx: Sender<RuntimeCommand>,
    event_tx: Sender<RuntimeEvent>,
    event_rx: std::sync::Arc<std::sync::Mutex<Receiver<RuntimeEvent>>>,
}

impl RuntimeServices {
    pub fn new() -> Result<Self> {
        let runtime = Runtime::new().context("create tokio runtime")?;
        let handle = runtime.handle().clone();
        let _runtime_thread = thread::Builder::new()
            .name("stellatune-gui-runtime".to_string())
            .spawn(move || runtime.block_on(async { std::future::pending::<()>().await }))
            .context("spawn runtime thread")?;

        let (command_tx, command_rx) = mpsc::channel::<RuntimeCommand>();
        let (event_tx, event_rx) = mpsc::channel::<RuntimeEvent>();

        let bridge_event_tx = event_tx.clone();
        thread::Builder::new()
            .name("stellatune-gui-bridge".to_string())
            .spawn(move || {
                while let Ok(command) = command_rx.recv() {
                    match command {
                        RuntimeCommand::RequestBootstrapSnapshot => {
                            let _ = bridge_event_tx.send(RuntimeEvent::BootstrapSnapshotReady);
                        },
                    }
                }
            })
            .context("spawn runtime bridge thread")?;

        Ok(Self {
            handle,
            command_tx,
            event_tx,
            event_rx: std::sync::Arc::new(std::sync::Mutex::new(event_rx)),
        })
    }

    pub fn spawn_heartbeat(&self) {
        let event_tx = self.event_tx.clone();
        self.handle.spawn(async move {
            let mut tick = 0u64;
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                tick = tick.saturating_add(1);
                if event_tx.send(RuntimeEvent::Heartbeat { tick }).is_err() {
                    break;
                }
            }
        });
    }

    pub fn send(&self, command: RuntimeCommand) {
        let _ = self.command_tx.send(command);
    }

    pub fn try_recv(&self) -> Result<RuntimeEvent, mpsc::TryRecvError> {
        let guard = self.event_rx.lock().expect("runtime event rx poisoned");
        guard.try_recv()
    }
}
