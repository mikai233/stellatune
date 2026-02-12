#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::sync::Arc;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::{Duration, Instant};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use crossbeam_channel::{Receiver as CbReceiver, Sender as CbSender};
use stellatune_plugin_protocol::RequestId;

use stellatune_core::{ControlCommand, ControlScope, Event, LibraryEvent, PluginRuntimeEvent};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) struct PluginRuntimeRouter {
    pub(super) engine: std::sync::Mutex<Option<stellatune_audio::EngineHandle>>,
    pub(super) library: std::sync::Mutex<Option<stellatune_library::LibraryHandle>>,
    pub(super) player_events: std::sync::Mutex<Option<CbReceiver<Event>>>,
    pub(super) library_events: std::sync::Mutex<Option<CbReceiver<LibraryEvent>>>,
    pub(super) runtime_hub: Arc<PluginRuntimeEventHub>,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) const CONTROL_FINISH_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) struct PluginRuntimeEventHub {
    subscribers: std::sync::Mutex<Vec<CbSender<PluginRuntimeEvent>>>,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
impl PluginRuntimeEventHub {
    pub(super) fn new() -> Self {
        Self {
            subscribers: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub(super) fn subscribe(&self) -> CbReceiver<PluginRuntimeEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.push(tx);
        }
        rx
    }

    pub(super) fn emit(&self, event: PluginRuntimeEvent) {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.retain(|tx| tx.send(event.clone()).is_ok());
        }
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
    LibraryRoots,
    LibraryFolders,
    LibraryExcludedFolders,
    LibraryTracks,
    LibrarySearchResult,
    LibraryPlaylists,
    LibraryPlaylistTracks,
    LibraryLikedTrackIds,
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
