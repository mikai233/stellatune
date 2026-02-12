use std::path::PathBuf;
use std::sync::Arc;

use crate::load::LoadedPluginModule;
use crate::load::RuntimeLoadReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceLibraryFingerprint {
    pub(crate) library_path: PathBuf,
    pub(crate) file_size: u64,
    pub(crate) modified_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncMode {
    Additive,
    Reconcile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PluginSyncAction {
    LoadNew { plugin_id: String },
    ReloadChanged { plugin_id: String },
    DeactivateMissingOrDisabled { plugin_id: String },
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSyncPlanSummary {
    pub discovered: usize,
    pub disabled: usize,
    pub actions_total: usize,
    pub load_new: usize,
    pub reload_changed: usize,
    pub deactivate: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeSyncActionOutcome {
    pub action: String,
    pub plugin_id: String,
    pub outcome: String,
}

#[derive(Debug, Default)]
pub struct RuntimeSyncReport {
    pub load_report: RuntimeLoadReport,
    pub plan: RuntimeSyncPlanSummary,
    pub actions: Vec<RuntimeSyncActionOutcome>,
    pub plan_ms: u64,
    pub execute_ms: u64,
    pub total_ms: u64,
}

pub(crate) struct ModuleLease {
    pub(crate) plugin_id: String,
    pub(crate) plugin_name: String,
    pub(crate) metadata_json: String,
    pub(crate) source_fingerprint: SourceLibraryFingerprint,
    pub(crate) loaded: LoadedPluginModule,
}

#[derive(Debug, Clone)]
pub struct ModuleLeaseRef {
    pub plugin_id: String,
    pub plugin_name: String,
    pub library_path: PathBuf,
}

impl ModuleLeaseRef {
    pub(crate) fn from_arc(lease: &Arc<ModuleLease>) -> Self {
        Self {
            plugin_id: lease.plugin_id.clone(),
            plugin_name: lease.plugin_name.clone(),
            library_path: lease.loaded.library_path.clone(),
        }
    }
}
