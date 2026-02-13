pub(crate) fn emit_config_update_runtime_event(
    plugin_id: &str,
    capability: &str,
    type_id: &str,
    status: &str,
    generation: u64,
    detail: Option<&str>,
) {
    let mut payload = serde_json::json!({
        "topic": "host.instance.config_update",
        "capability": capability,
        "type_id": type_id,
        "status": status,
        "generation": generation,
    });
    if let Some(detail) = detail
        && let Some(obj) = payload.as_object_mut()
    {
        obj.insert(
            "detail".to_string(),
            serde_json::Value::String(detail.to_string()),
        );
    }
    if let Ok(payload_json) = serde_json::to_string(&payload) {
        stellatune_plugins::runtime::handle::shared_runtime_service()
            .push_host_event_json(plugin_id, &payload_json);
    }
}
