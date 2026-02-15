pub(crate) fn emit_config_update_runtime_event(
    plugin_id: &str,
    capability: &str,
    type_id: &str,
    status: &str,
    generation: u64,
    detail: Option<&str>,
) {
    let _ = (plugin_id, capability, type_id, status, generation, detail);
}
