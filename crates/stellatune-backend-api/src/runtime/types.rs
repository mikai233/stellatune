#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::sync::Arc;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::sync::atomic::AtomicU64;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::{Duration, Instant};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugin_protocol::RequestId;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use tokio::sync::broadcast;

use stellatune_core::{ControlCommand, ControlScope, PluginRuntimeEvent};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) struct PluginRuntimeRouter {
    pub(super) player_event_generation: AtomicU64,
    pub(super) library_event_generation: AtomicU64,
    pub(super) runtime_hub: Arc<PluginRuntimeEventHub>,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) const CONTROL_FINISH_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) struct PluginRuntimeEventHub {
    tx: broadcast::Sender<PluginRuntimeEvent>,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
impl PluginRuntimeEventHub {
    pub(super) fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    pub(super) fn subscribe(&self) -> broadcast::Receiver<PluginRuntimeEvent> {
        self.tx.subscribe()
    }

    pub(super) fn emit(&self, event: PluginRuntimeEvent) {
        let _ = self.tx.send(event);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlWaitKind {
    Immediate,
    PlayerState(stellatune_core::PlayerState),
    PlayerPosition,
    PlayerVolume,
    PlayerTrackChanged,
    LibraryScanFinished,
    LibraryChanged,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
#[derive(Debug, Clone)]
pub(super) struct PendingControlFinish {
    pub(super) plugin_id: String,
    pub(super) request_id: Option<RequestId>,
    pub(super) scope: ControlScope,
    pub(super) command: Option<ControlCommand>,
    pub(super) wait: ControlWaitKind,
    pub(super) deadline: Instant,
}
