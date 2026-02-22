use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub(super) struct HealthResponse {
    pub(super) ok: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct PluginConfigResponse {
    pub(super) plugin_id: String,
    pub(super) config: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) apply_report: Option<ConfigApplyReport>,
}

#[derive(Debug, Serialize)]
pub(super) struct ActionInvokeResponse {
    pub(super) plugin_id: String,
    pub(super) action: String,
    pub(super) accepted: bool,
    pub(super) message: String,
    pub(super) data: Value,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConfigApplyOutcome {
    pub(super) kind: String,
    pub(super) type_id: String,
    pub(super) status: String,
    pub(super) detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConfigApplyReport {
    pub(super) plugin_id: String,
    pub(super) applied: usize,
    pub(super) skipped: usize,
    pub(super) failed: usize,
    pub(super) outcomes: Vec<ConfigApplyOutcome>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct PluginUiEvent {
    pub(super) plugin_id: String,
    pub(super) name: String,
    pub(super) payload: Value,
    pub(super) ts_ms: u64,
}
