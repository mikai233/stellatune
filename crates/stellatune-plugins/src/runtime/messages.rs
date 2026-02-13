use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::Sender;
use tokio::sync::{mpsc, oneshot};

use crate::load::{RuntimeLoadReport, RuntimePluginInfo};
use crate::runtime::backend_control::BackendControlRequest;
use crate::runtime::introspection::{PluginLeaseInfo, PluginLeaseState};
use crate::runtime::model::{ModuleLease, ModuleLeaseRef, RuntimeSyncReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerControlMessage {
    Recreate { reason: String, seq: u64 },
    Destroy { reason: String, seq: u64 },
}

pub(crate) enum RuntimeActorMessage {
    RegisterWorkerControlSender {
        plugin_id: String,
        sender: Sender<WorkerControlMessage>,
        resp_tx: oneshot::Sender<bool>,
    },
    SubscribeBackendControlRequests {
        resp_tx: oneshot::Sender<mpsc::UnboundedReceiver<BackendControlRequest>>,
    },
    SetDisabledPluginIds {
        disabled_ids: HashSet<String>,
        resp_tx: oneshot::Sender<()>,
    },
    PushHostEventJson {
        plugin_id: String,
        event_json: String,
        resp_tx: oneshot::Sender<()>,
    },
    BroadcastHostEventJson {
        event_json: String,
        resp_tx: oneshot::Sender<()>,
    },
    SetPluginEnabled {
        plugin_id: String,
        enabled: bool,
        resp_tx: oneshot::Sender<()>,
    },
    DisabledPluginIds {
        resp_tx: oneshot::Sender<HashSet<String>>,
    },
    ListActivePlugins {
        resp_tx: oneshot::Sender<Vec<RuntimePluginInfo>>,
    },
    CurrentModuleLeaseRef {
        plugin_id: String,
        resp_tx: oneshot::Sender<Option<ModuleLeaseRef>>,
    },
    CurrentPluginLeaseInfo {
        plugin_id: String,
        resp_tx: oneshot::Sender<Option<PluginLeaseInfo>>,
    },
    PluginLeaseState {
        plugin_id: String,
        resp_tx: oneshot::Sender<Option<PluginLeaseState>>,
    },
    AcquireCurrentModuleLease {
        plugin_id: String,
        resp_tx: oneshot::Sender<Option<Arc<ModuleLease>>>,
    },
    ActivePluginIds {
        resp_tx: oneshot::Sender<Vec<String>>,
    },
    LoadDirAdditiveFromState {
        dir: PathBuf,
        resp_tx: oneshot::Sender<anyhow::Result<RuntimeLoadReport>>,
    },
    LoadDirAdditiveFiltered {
        dir: PathBuf,
        disabled_ids: HashSet<String>,
        resp_tx: oneshot::Sender<anyhow::Result<RuntimeLoadReport>>,
    },
    ReloadDirFiltered {
        dir: PathBuf,
        disabled_ids: HashSet<String>,
        resp_tx: oneshot::Sender<anyhow::Result<RuntimeLoadReport>>,
    },
    ReloadDirFromState {
        dir: PathBuf,
        resp_tx: oneshot::Sender<anyhow::Result<RuntimeLoadReport>>,
    },
    ReloadDirDetailedFromState {
        dir: PathBuf,
        resp_tx: oneshot::Sender<anyhow::Result<RuntimeSyncReport>>,
    },
    UnloadPlugin {
        plugin_id: String,
        resp_tx: oneshot::Sender<RuntimeLoadReport>,
    },
    ShutdownAndCleanup {
        resp_tx: oneshot::Sender<RuntimeLoadReport>,
    },
    CleanupShadowCopiesNow {
        resp_tx: oneshot::Sender<()>,
    },
    CollectRetiredModuleLeasesByRefcount {
        resp_tx: oneshot::Sender<usize>,
    },
}
