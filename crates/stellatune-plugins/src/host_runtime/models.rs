#[derive(Debug, Clone)]
pub struct RuntimeActivePluginDescriptor {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Default)]
pub struct RuntimeLoadReport {
    pub loaded: Vec<String>,
    pub deactivated: Vec<String>,
    pub errors: Vec<anyhow::Error>,
}

#[derive(Debug, Default)]
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
