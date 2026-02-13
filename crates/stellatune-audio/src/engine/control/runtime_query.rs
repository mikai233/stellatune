use tracing::debug;

use stellatune_plugins::runtime::introspection::CapabilityKind;
use stellatune_plugins::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome,
};

use super::{
    CachedLyricsInstance, CachedOutputSinkInstance, CachedSourceInstance, EngineState,
    RuntimeInstanceSlotKey, emit_config_update_runtime_event, runtime_default_config_json,
    with_runtime_service,
};

fn create_source_catalog_cached_instance(
    service: &stellatune_plugins::runtime::handle::SharedPluginRuntimeService,
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
) -> Result<CachedSourceInstance, String> {
    let endpoint = stellatune_runtime::block_on(
        service.bind_source_catalog_worker_endpoint(plugin_id, type_id),
    )
    .map_err(|e| e.to_string())?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok(CachedSourceInstance {
                config_json: config_json.to_string(),
                controller,
                control_rx,
            })
        }
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "source controller did not create instance for {plugin_id}::{type_id}"
        )),
    }
}

fn create_lyrics_provider_cached_instance(
    service: &stellatune_plugins::runtime::handle::SharedPluginRuntimeService,
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
) -> Result<CachedLyricsInstance, String> {
    let endpoint = stellatune_runtime::block_on(
        service.bind_lyrics_provider_worker_endpoint(plugin_id, type_id),
    )
    .map_err(|e| e.to_string())?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok(CachedLyricsInstance {
                config_json: config_json.to_string(),
                controller,
                control_rx,
            })
        }
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "lyrics controller did not create instance for {plugin_id}::{type_id}"
        )),
    }
}

fn create_output_sink_cached_instance(
    service: &stellatune_plugins::runtime::handle::SharedPluginRuntimeService,
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
) -> Result<CachedOutputSinkInstance, String> {
    let endpoint =
        stellatune_runtime::block_on(service.bind_output_sink_worker_endpoint(plugin_id, type_id))
            .map_err(|e| e.to_string())?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok(CachedOutputSinkInstance {
                config_json: config_json.to_string(),
                controller,
                control_rx,
            })
        }
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "output sink controller did not create instance for {plugin_id}::{type_id}"
        )),
    }
}

fn recreate_source_instance(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedSourceInstance,
) -> Result<(), String> {
    let exported_state = entry
        .controller
        .instance()
        .and_then(|instance| instance.export_state_json().ok().flatten());
    entry.controller.request_recreate();
    match entry.controller.apply_pending().map_err(|e| {
        format!("source recreate apply_pending failed for {plugin_id}::{type_id}: {e}")
    })? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "source recreate did not produce instance for {plugin_id}::{type_id}"
            ));
        }
    }

    if let Some(state_json) = exported_state
        && let Some(instance) = entry.controller.instance_mut()
    {
        let _ = instance.import_state_json(&state_json);
    }

    Ok(())
}

fn recreate_lyrics_instance(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedLyricsInstance,
) -> Result<(), String> {
    let exported_state = entry
        .controller
        .instance()
        .and_then(|instance| instance.export_state_json().ok().flatten());
    entry.controller.request_recreate();
    match entry.controller.apply_pending().map_err(|e| {
        format!("lyrics recreate apply_pending failed for {plugin_id}::{type_id}: {e}")
    })? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "lyrics recreate did not produce instance for {plugin_id}::{type_id}"
            ));
        }
    }

    if let Some(state_json) = exported_state
        && let Some(instance) = entry.controller.instance_mut()
    {
        let _ = instance.import_state_json(&state_json);
    }

    Ok(())
}

fn recreate_output_sink_instance(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedOutputSinkInstance,
) -> Result<(), String> {
    let exported_state = entry
        .controller
        .instance()
        .and_then(|instance| instance.export_state_json().ok().flatten());
    entry.controller.request_recreate();
    match entry.controller.apply_pending().map_err(|e| {
        format!("output sink recreate apply_pending failed for {plugin_id}::{type_id}: {e}")
    })? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "output sink recreate did not produce instance for {plugin_id}::{type_id}"
            ));
        }
    }

    if let Some(state_json) = exported_state
        && let Some(instance) = entry.controller.instance_mut()
    {
        let _ = instance.import_state_json(&state_json);
    }

    Ok(())
}

fn sync_source_runtime_control(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedSourceInstance,
) -> Result<(), String> {
    while let Ok(message) = entry.control_rx.try_recv() {
        entry.controller.on_control_message(message);
    }

    if entry.controller.has_pending_destroy() {
        match entry.controller.apply_pending().map_err(|e| {
            format!("source destroy apply_pending failed for {plugin_id}::{type_id}: {e}")
        })? {
            WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
                debug!(
                    plugin_id,
                    type_id,
                    "source instance destroyed by runtime control; will recreate on demand"
                );
            }
            WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        }
    }

    if entry.controller.has_pending_recreate() {
        recreate_source_instance(plugin_id, type_id, entry)?;
    }

    if entry.controller.instance().is_none() {
        recreate_source_instance(plugin_id, type_id, entry)?;
    }

    Ok(())
}

fn sync_lyrics_runtime_control(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedLyricsInstance,
) -> Result<(), String> {
    while let Ok(message) = entry.control_rx.try_recv() {
        entry.controller.on_control_message(message);
    }

    if entry.controller.has_pending_destroy() {
        match entry.controller.apply_pending().map_err(|e| {
            format!("lyrics destroy apply_pending failed for {plugin_id}::{type_id}: {e}")
        })? {
            WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
                debug!(
                    plugin_id,
                    type_id,
                    "lyrics instance destroyed by runtime control; will recreate on demand"
                );
            }
            WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        }
    }

    if entry.controller.has_pending_recreate() {
        recreate_lyrics_instance(plugin_id, type_id, entry)?;
    }

    if entry.controller.instance().is_none() {
        recreate_lyrics_instance(plugin_id, type_id, entry)?;
    }

    Ok(())
}

fn sync_output_sink_runtime_control(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedOutputSinkInstance,
) -> Result<(), String> {
    while let Ok(message) = entry.control_rx.try_recv() {
        entry.controller.on_control_message(message);
    }

    if entry.controller.has_pending_destroy() {
        match entry.controller.apply_pending().map_err(|e| {
            format!("output sink destroy apply_pending failed for {plugin_id}::{type_id}: {e}")
        })? {
            WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
                debug!(
                    plugin_id,
                    type_id,
                    "output sink instance destroyed by runtime control; will recreate on demand"
                );
            }
            WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        }
    }

    if entry.controller.has_pending_recreate() {
        recreate_output_sink_instance(plugin_id, type_id, entry)?;
    }

    if entry.controller.instance().is_none() {
        recreate_output_sink_instance(plugin_id, type_id, entry)?;
    }

    Ok(())
}

pub(super) fn clear_runtime_query_instance_cache(state: &mut EngineState) {
    state.source_instances.clear();
    state.lyrics_instances.clear();
    state.output_sink_instances.clear();
    state.output_sink_negotiation_cache = None;
}

pub(super) fn clear_runtime_query_instance_cache_for_plugin(
    state: &mut EngineState,
    plugin_id: &str,
) -> (usize, usize, usize) {
    let source_before = state.source_instances.len();
    state
        .source_instances
        .retain(|k, _| k.plugin_id.as_str() != plugin_id);
    let source_removed = source_before.saturating_sub(state.source_instances.len());

    let lyrics_before = state.lyrics_instances.len();
    state
        .lyrics_instances
        .retain(|k, _| k.plugin_id.as_str() != plugin_id);
    let lyrics_removed = lyrics_before.saturating_sub(state.lyrics_instances.len());

    let output_sink_before = state.output_sink_instances.len();
    state
        .output_sink_instances
        .retain(|k, _| k.plugin_id.as_str() != plugin_id);
    let output_sink_removed = output_sink_before.saturating_sub(state.output_sink_instances.len());

    if state
        .output_sink_negotiation_cache
        .as_ref()
        .is_some_and(|cache| cache.route.plugin_id == plugin_id)
    {
        state.output_sink_negotiation_cache = None;
    }

    (source_removed, lyrics_removed, output_sink_removed)
}

pub(super) fn apply_or_recreate_source_instance(
    plugin_id: &str,
    type_id: &str,
    entry: &mut CachedSourceInstance,
    config_json: &str,
) -> Result<(), String> {
    sync_source_runtime_control(plugin_id, type_id, entry)?;
    if entry.config_json == config_json {
        return Ok(());
    }

    let result = entry
        .controller
        .apply_config_update(config_json.to_string())
        .map_err(|e| e.to_string())?;
    match result {
        WorkerConfigUpdateOutcome::Applied {
            revision: generation,
        } => {
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
        WorkerConfigUpdateOutcome::RequiresRecreate {
            revision: generation,
            reason,
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "source_catalog",
                type_id,
                "requires_recreate",
                generation,
                reason.as_deref(),
            );
            if let Err(error) = recreate_source_instance(plugin_id, type_id, entry) {
                emit_config_update_runtime_event(
                    plugin_id,
                    "source_catalog",
                    type_id,
                    "failed",
                    generation,
                    Some(&error),
                );
                return Err(error);
            }
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
        WorkerConfigUpdateOutcome::DeferredNoInstance => {
            emit_config_update_runtime_event(
                plugin_id,
                "source_catalog",
                type_id,
                "requires_recreate",
                0,
                Some("source instance missing; deferred to recreate"),
            );
            if let Err(error) = recreate_source_instance(plugin_id, type_id, entry) {
                emit_config_update_runtime_event(
                    plugin_id,
                    "source_catalog",
                    type_id,
                    "failed",
                    0,
                    Some(&error),
                );
                return Err(error);
            }
            entry.config_json = config_json.to_string();
            emit_config_update_runtime_event(
                plugin_id,
                "source_catalog",
                type_id,
                "recreated",
                0,
                Some("deferred_no_instance"),
            );
            Ok(())
        }
        WorkerConfigUpdateOutcome::Rejected {
            revision: generation,
            reason,
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
        WorkerConfigUpdateOutcome::Failed {
            revision: generation,
            error,
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
    sync_lyrics_runtime_control(plugin_id, type_id, entry)?;
    if entry.config_json == config_json {
        return Ok(());
    }

    let result = entry
        .controller
        .apply_config_update(config_json.to_string())
        .map_err(|e| e.to_string())?;
    match result {
        WorkerConfigUpdateOutcome::Applied {
            revision: generation,
        } => {
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
        WorkerConfigUpdateOutcome::RequiresRecreate {
            revision: generation,
            reason,
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "lyrics_provider",
                type_id,
                "requires_recreate",
                generation,
                reason.as_deref(),
            );
            if let Err(error) = recreate_lyrics_instance(plugin_id, type_id, entry) {
                emit_config_update_runtime_event(
                    plugin_id,
                    "lyrics_provider",
                    type_id,
                    "failed",
                    generation,
                    Some(&error),
                );
                return Err(error);
            }
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
        WorkerConfigUpdateOutcome::DeferredNoInstance => {
            emit_config_update_runtime_event(
                plugin_id,
                "lyrics_provider",
                type_id,
                "requires_recreate",
                0,
                Some("lyrics instance missing; deferred to recreate"),
            );
            if let Err(error) = recreate_lyrics_instance(plugin_id, type_id, entry) {
                emit_config_update_runtime_event(
                    plugin_id,
                    "lyrics_provider",
                    type_id,
                    "failed",
                    0,
                    Some(&error),
                );
                return Err(error);
            }
            entry.config_json = config_json.to_string();
            emit_config_update_runtime_event(
                plugin_id,
                "lyrics_provider",
                type_id,
                "recreated",
                0,
                Some("deferred_no_instance"),
            );
            Ok(())
        }
        WorkerConfigUpdateOutcome::Rejected {
            revision: generation,
            reason,
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
        WorkerConfigUpdateOutcome::Failed {
            revision: generation,
            error,
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
    sync_output_sink_runtime_control(plugin_id, type_id, entry)?;
    if entry.config_json == config_json {
        return Ok(());
    }

    let result = entry
        .controller
        .apply_config_update(config_json.to_string())
        .map_err(|e| e.to_string())?;
    match result {
        WorkerConfigUpdateOutcome::Applied {
            revision: generation,
        } => {
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
        WorkerConfigUpdateOutcome::RequiresRecreate {
            revision: generation,
            reason,
        } => {
            emit_config_update_runtime_event(
                plugin_id,
                "output_sink",
                type_id,
                "requires_recreate",
                generation,
                reason.as_deref(),
            );
            if let Err(error) = recreate_output_sink_instance(plugin_id, type_id, entry) {
                emit_config_update_runtime_event(
                    plugin_id,
                    "output_sink",
                    type_id,
                    "failed",
                    generation,
                    Some(&error),
                );
                return Err(error);
            }
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
        WorkerConfigUpdateOutcome::DeferredNoInstance => {
            emit_config_update_runtime_event(
                plugin_id,
                "output_sink",
                type_id,
                "requires_recreate",
                0,
                Some("output sink instance missing; deferred to recreate"),
            );
            if let Err(error) = recreate_output_sink_instance(plugin_id, type_id, entry) {
                emit_config_update_runtime_event(
                    plugin_id,
                    "output_sink",
                    type_id,
                    "failed",
                    0,
                    Some(&error),
                );
                return Err(error);
            }
            entry.config_json = config_json.to_string();
            emit_config_update_runtime_event(
                plugin_id,
                "output_sink",
                type_id,
                "recreated",
                0,
                Some("deferred_no_instance"),
            );
            Ok(())
        }
        WorkerConfigUpdateOutcome::Rejected {
            revision: generation,
            reason,
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
        WorkerConfigUpdateOutcome::Failed {
            revision: generation,
            error,
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
            create_source_catalog_cached_instance(service, plugin_id, type_id, &config_json)
        })?;
        state.source_instances.insert(key.clone(), created);
    }
    let entry = state
        .source_instances
        .get_mut(&key)
        .ok_or_else(|| "source instance cache insertion failed".to_string())?;
    apply_or_recreate_source_instance(plugin_id, type_id, entry, &config_json)?;

    let instance = entry
        .controller
        .instance_mut()
        .ok_or_else(|| format!("source instance unavailable for {plugin_id}::{type_id}"))?;
    instance
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
            create_lyrics_provider_cached_instance(service, plugin_id, type_id, &config_json)
        })?;
        state.lyrics_instances.insert(key.clone(), created);
    }
    let entry = state
        .lyrics_instances
        .get_mut(&key)
        .ok_or_else(|| "lyrics instance cache insertion failed".to_string())?;
    apply_or_recreate_lyrics_instance(plugin_id, type_id, entry, &config_json)?;

    let instance = entry
        .controller
        .instance_mut()
        .ok_or_else(|| format!("lyrics instance unavailable for {plugin_id}::{type_id}"))?;
    instance.search_json(&query_json).map_err(|e| e.to_string())
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
            create_lyrics_provider_cached_instance(service, plugin_id, type_id, &config_json)
        })?;
        state.lyrics_instances.insert(key.clone(), created);
    }
    let entry = state
        .lyrics_instances
        .get_mut(&key)
        .ok_or_else(|| "lyrics instance cache insertion failed".to_string())?;
    apply_or_recreate_lyrics_instance(plugin_id, type_id, entry, &config_json)?;

    let instance = entry
        .controller
        .instance_mut()
        .ok_or_else(|| format!("lyrics instance unavailable for {plugin_id}::{type_id}"))?;
    instance.fetch_json(&track_json).map_err(|e| e.to_string())
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
            create_output_sink_cached_instance(service, plugin_id, type_id, &config_json)
        })?;
        state.output_sink_instances.insert(key.clone(), created);
    }
    let entry = state
        .output_sink_instances
        .get_mut(&key)
        .ok_or_else(|| "output sink instance cache insertion failed".to_string())?;
    apply_or_recreate_output_sink_instance(plugin_id, type_id, entry, &config_json)?;

    let instance = entry
        .controller
        .instance_mut()
        .ok_or_else(|| format!("output sink instance unavailable for {plugin_id}::{type_id}"))?;
    instance.list_targets_json().map_err(|e| e.to_string())
}
