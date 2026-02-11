use tracing::debug;

use stellatune_plugins::runtime::{CapabilityKind, InstanceUpdateResult};

use super::{
    CachedLyricsInstance, CachedOutputSinkInstance, CachedSourceInstance, EngineState,
    RuntimeInstanceSlotKey, emit_config_update_runtime_event, runtime_default_config_json,
    with_runtime_service,
};

pub(super) fn clear_runtime_query_instance_cache(state: &mut EngineState) {
    state.source_instances.clear();
    state.lyrics_instances.clear();
    state.output_sink_instances.clear();
    state.output_sink_negotiation_cache = None;
}

pub(super) fn apply_or_recreate_source_instance(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedSourceInstance,
    config_json: &str,
) -> Result<(), String> {
    if entry.config_json == config_json {
        return Ok(());
    }
    let result = entry
        .instance
        .apply_config_update_json(config_json)
        .map_err(|e| e.to_string())?;
    match result {
        InstanceUpdateResult::Applied { generation, .. } => {
            emit_config_update_runtime_event(
                plugin_id,
                "source_catalog",
                type_id,
                "applied",
                generation,
                None,
            );
            entry.config_json = config_json.to_string();
            Ok(())
        }
        InstanceUpdateResult::RequiresRecreate {
            generation, reason, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "source_catalog",
                type_id,
                "requires_recreate",
                generation,
                reason.as_deref(),
            );
            let mut next = match with_runtime_service(|service| {
                service
                    .create_source_catalog_instance(plugin_id, type_id, config_json)
                    .map_err(|e| e.to_string())
            }) {
                Ok(v) => v,
                Err(error) => {
                    emit_config_update_runtime_event(
                        plugin_id,
                        "source_catalog",
                        type_id,
                        "failed",
                        generation,
                        Some(&error),
                    );
                    return Err(format!(
                        "source recreate failed for {plugin_id}::{type_id}: {error}"
                    ));
                }
            };
            if let Ok(Some(state_json)) = entry.instance.export_state_json() {
                let _ = next.import_state_json(&state_json);
            }
            entry.instance = next;
            entry.config_json = config_json.to_string();
            emit_config_update_runtime_event(
                plugin_id,
                "source_catalog",
                type_id,
                "recreated",
                generation,
                None,
            );
            if let Some(reason) = reason {
                debug!(plugin_id, type_id, "source config recreate: {reason}");
            }
            Ok(())
        }
        InstanceUpdateResult::Rejected {
            generation, reason, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "source_catalog",
                type_id,
                "rejected",
                generation,
                Some(&reason),
            );
            Err(format!(
                "source config update rejected for {plugin_id}::{type_id}: {reason}"
            ))
        }
        InstanceUpdateResult::Failed {
            generation, error, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "source_catalog",
                type_id,
                "failed",
                generation,
                Some(&error),
            );
            Err(format!(
                "source config update failed for {plugin_id}::{type_id}: {error}"
            ))
        }
    }
}

pub(super) fn apply_or_recreate_lyrics_instance(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedLyricsInstance,
    config_json: &str,
) -> Result<(), String> {
    if entry.config_json == config_json {
        return Ok(());
    }
    let result = entry
        .instance
        .apply_config_update_json(config_json)
        .map_err(|e| e.to_string())?;
    match result {
        InstanceUpdateResult::Applied { generation, .. } => {
            emit_config_update_runtime_event(
                plugin_id,
                "lyrics_provider",
                type_id,
                "applied",
                generation,
                None,
            );
            entry.config_json = config_json.to_string();
            Ok(())
        }
        InstanceUpdateResult::RequiresRecreate {
            generation, reason, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "lyrics_provider",
                type_id,
                "requires_recreate",
                generation,
                reason.as_deref(),
            );
            let mut next = match with_runtime_service(|service| {
                service
                    .create_lyrics_provider_instance(plugin_id, type_id, config_json)
                    .map_err(|e| e.to_string())
            }) {
                Ok(v) => v,
                Err(error) => {
                    emit_config_update_runtime_event(
                        plugin_id,
                        "lyrics_provider",
                        type_id,
                        "failed",
                        generation,
                        Some(&error),
                    );
                    return Err(format!(
                        "lyrics recreate failed for {plugin_id}::{type_id}: {error}"
                    ));
                }
            };
            if let Ok(Some(state_json)) = entry.instance.export_state_json() {
                let _ = next.import_state_json(&state_json);
            }
            entry.instance = next;
            entry.config_json = config_json.to_string();
            emit_config_update_runtime_event(
                plugin_id,
                "lyrics_provider",
                type_id,
                "recreated",
                generation,
                None,
            );
            if let Some(reason) = reason {
                debug!(plugin_id, type_id, "lyrics config recreate: {reason}");
            }
            Ok(())
        }
        InstanceUpdateResult::Rejected {
            generation, reason, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "lyrics_provider",
                type_id,
                "rejected",
                generation,
                Some(&reason),
            );
            Err(format!(
                "lyrics config update rejected for {plugin_id}::{type_id}: {reason}"
            ))
        }
        InstanceUpdateResult::Failed {
            generation, error, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "lyrics_provider",
                type_id,
                "failed",
                generation,
                Some(&error),
            );
            Err(format!(
                "lyrics config update failed for {plugin_id}::{type_id}: {error}"
            ))
        }
    }
}

pub(super) fn apply_or_recreate_output_sink_instance(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedOutputSinkInstance,
    config_json: &str,
) -> Result<(), String> {
    if entry.config_json == config_json {
        return Ok(());
    }
    let result = entry
        .instance
        .apply_config_update_json(config_json)
        .map_err(|e| e.to_string())?;
    match result {
        InstanceUpdateResult::Applied { generation, .. } => {
            emit_config_update_runtime_event(
                plugin_id,
                "output_sink",
                type_id,
                "applied",
                generation,
                None,
            );
            entry.config_json = config_json.to_string();
            Ok(())
        }
        InstanceUpdateResult::RequiresRecreate {
            generation, reason, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "output_sink",
                type_id,
                "requires_recreate",
                generation,
                reason.as_deref(),
            );
            let mut next = match with_runtime_service(|service| {
                service
                    .create_output_sink_instance(plugin_id, type_id, config_json)
                    .map_err(|e| e.to_string())
            }) {
                Ok(v) => v,
                Err(error) => {
                    emit_config_update_runtime_event(
                        plugin_id,
                        "output_sink",
                        type_id,
                        "failed",
                        generation,
                        Some(&error),
                    );
                    return Err(format!(
                        "output sink recreate failed for {plugin_id}::{type_id}: {error}"
                    ));
                }
            };
            if let Ok(Some(state_json)) = entry.instance.export_state_json() {
                let _ = next.import_state_json(&state_json);
            }
            entry.instance = next;
            entry.config_json = config_json.to_string();
            emit_config_update_runtime_event(
                plugin_id,
                "output_sink",
                type_id,
                "recreated",
                generation,
                None,
            );
            if let Some(reason) = reason {
                debug!(plugin_id, type_id, "output sink config recreate: {reason}");
            }
            Ok(())
        }
        InstanceUpdateResult::Rejected {
            generation, reason, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "output_sink",
                type_id,
                "rejected",
                generation,
                Some(&reason),
            );
            Err(format!(
                "output sink config update rejected for {plugin_id}::{type_id}: {reason}"
            ))
        }
        InstanceUpdateResult::Failed {
            generation, error, ..
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "output_sink",
                type_id,
                "failed",
                generation,
                Some(&error),
            );
            Err(format!(
                "output sink config update failed for {plugin_id}::{type_id}: {error}"
            ))
        }
    }
}

pub(super) fn source_list_items_json_via_runtime(
    state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    config_json: String,
    request_json: String,
) -> Result<String, String> {
    let key = RuntimeInstanceSlotKey::new(plugin_id, type_id);
    if !state.source_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            service
                .create_source_catalog_instance(plugin_id, type_id, &config_json)
                .map_err(|e| e.to_string())
        })?;
        state.source_instances.insert(
            key.clone(),
            CachedSourceInstance {
                config_json: config_json.clone(),
                instance: created,
            },
        );
    }
    let entry = state
        .source_instances
        .get_mut(&key)
        .ok_or_else(|| "source instance cache insertion failed".to_string())?;
    if entry.config_json != config_json {
        apply_or_recreate_source_instance(plugin_id, type_id, entry, &config_json)?;
    }
    entry
        .instance
        .list_items_json(&request_json)
        .map_err(|e| e.to_string())
}

pub(super) fn lyrics_search_json_via_runtime(
    state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    query_json: String,
) -> Result<String, String> {
    let config_json =
        runtime_default_config_json(plugin_id, CapabilityKind::LyricsProvider, type_id)?;
    let key = RuntimeInstanceSlotKey::new(plugin_id, type_id);
    if !state.lyrics_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            service
                .create_lyrics_provider_instance(plugin_id, type_id, &config_json)
                .map_err(|e| e.to_string())
        })?;
        state.lyrics_instances.insert(
            key.clone(),
            CachedLyricsInstance {
                config_json: config_json.clone(),
                instance: created,
            },
        );
    }
    let entry = state
        .lyrics_instances
        .get_mut(&key)
        .ok_or_else(|| "lyrics instance cache insertion failed".to_string())?;
    if entry.config_json != config_json {
        apply_or_recreate_lyrics_instance(plugin_id, type_id, entry, &config_json)?;
    }
    entry
        .instance
        .search_json(&query_json)
        .map_err(|e| e.to_string())
}

pub(super) fn lyrics_fetch_json_via_runtime(
    state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    track_json: String,
) -> Result<String, String> {
    let config_json =
        runtime_default_config_json(plugin_id, CapabilityKind::LyricsProvider, type_id)?;
    let key = RuntimeInstanceSlotKey::new(plugin_id, type_id);
    if !state.lyrics_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            service
                .create_lyrics_provider_instance(plugin_id, type_id, &config_json)
                .map_err(|e| e.to_string())
        })?;
        state.lyrics_instances.insert(
            key.clone(),
            CachedLyricsInstance {
                config_json: config_json.clone(),
                instance: created,
            },
        );
    }
    let entry = state
        .lyrics_instances
        .get_mut(&key)
        .ok_or_else(|| "lyrics instance cache insertion failed".to_string())?;
    if entry.config_json != config_json {
        apply_or_recreate_lyrics_instance(plugin_id, type_id, entry, &config_json)?;
    }
    entry
        .instance
        .fetch_json(&track_json)
        .map_err(|e| e.to_string())
}

pub(super) fn output_sink_list_targets_json_via_runtime(
    state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    config_json: String,
) -> Result<String, String> {
    let key = RuntimeInstanceSlotKey::new(plugin_id, type_id);
    if !state.output_sink_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            service
                .create_output_sink_instance(plugin_id, type_id, &config_json)
                .map_err(|e| e.to_string())
        })?;
        state.output_sink_instances.insert(
            key.clone(),
            CachedOutputSinkInstance {
                config_json: config_json.clone(),
                instance: created,
            },
        );
    }
    let entry = state
        .output_sink_instances
        .get_mut(&key)
        .ok_or_else(|| "output sink instance cache insertion failed".to_string())?;
    if entry.config_json != config_json {
        apply_or_recreate_output_sink_instance(plugin_id, type_id, entry, &config_json)?;
    }
    entry
        .instance
        .list_targets_json()
        .map_err(|e| e.to_string())
}
