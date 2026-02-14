use std::time::Duration;

use tokio::sync::oneshot::Sender as OneshotSender;
use tracing::debug;

use stellatune_plugins::runtime::introspection::CapabilityKind;
use stellatune_plugins::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome,
};

use self::lyrics_owner_actor::handlers::fetch::LyricsFetchMessage;
use self::lyrics_owner_actor::handlers::search::LyricsSearchMessage;
use self::output_sink_owner_actor::handlers::list_targets::OutputSinkListTargetsMessage;
use self::runtime_owner_registry_actor::handlers::clear_all::ClearAllRuntimeOwnersMessage;
use self::runtime_owner_registry_actor::handlers::clear_for_plugin::ClearRuntimeOwnersForPluginMessage;
use self::runtime_owner_registry_actor::handlers::commit_source_open_stream::CommitSourceOpenStreamMessage;
use self::runtime_owner_registry_actor::handlers::ensure_lyrics_owner_task::EnsureLyricsOwnerTaskMessage;
use self::runtime_owner_registry_actor::handlers::ensure_output_sink_owner_task::EnsureOutputSinkOwnerTaskMessage;
use self::runtime_owner_registry_actor::handlers::ensure_source_owner_task::EnsureSourceOwnerTaskMessage;
use self::runtime_owner_registry_actor::handlers::finalize_source_close_stream::FinalizeSourceCloseStreamMessage;
use self::runtime_owner_registry_actor::handlers::prepare_source_close_stream::PrepareSourceCloseStreamMessage;
use self::runtime_owner_registry_actor::handlers::prepare_source_open_stream::PrepareSourceOpenStreamMessage;
use self::runtime_owner_registry_actor::handlers::rollback_source_open_stream::RollbackSourceOpenStreamMessage;
use self::runtime_owner_registry_actor::{SourceCloseTarget, shared_runtime_owner_registry_actor};
use self::source_owner_actor::handlers::close_stream::SourceCloseStreamMessage;
use self::source_owner_actor::handlers::list_items::SourceListItemsMessage;
use self::source_owner_actor::handlers::open_stream::SourceOpenStreamMessage;
use super::{
    CachedLyricsInstance, CachedOutputSinkInstance, CachedSourceInstance, EngineState,
    RuntimeInstanceSlotKey, emit_config_update_runtime_event, runtime_default_config_json,
    with_runtime_service,
};

const OWNER_WORKER_CLEAR_TIMEOUT: Duration = Duration::from_millis(500);
const OWNER_WORKER_STREAM_TIMEOUT: Duration = Duration::from_secs(5);

mod lyrics_owner_actor;
mod output_sink_owner_actor;
mod runtime_owner_registry_actor;
mod source_owner_actor;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSourceStreamLease {
    pub stream_id: u64,
    pub lease_id: u64,
    pub io_vtable_addr: usize,
    pub io_handle_addr: usize,
    pub source_metadata_json: Option<String>,
}

fn create_source_catalog_cached_instance(
    service: &stellatune_plugins::runtime::handle::SharedPluginRuntimeHandle,
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
    service: &stellatune_plugins::runtime::handle::SharedPluginRuntimeHandle,
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
    service: &stellatune_plugins::runtime::handle::SharedPluginRuntimeHandle,
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

fn clear_runtime_owner_worker_cache() {
    let actor_ref = shared_runtime_owner_registry_actor();
    let _ = stellatune_runtime::block_on(
        actor_ref.call(ClearAllRuntimeOwnersMessage, OWNER_WORKER_CLEAR_TIMEOUT),
    );
}

fn clear_runtime_owner_worker_cache_for_plugin(plugin_id: &str) -> (usize, usize, usize) {
    let actor_ref = shared_runtime_owner_registry_actor();
    match stellatune_runtime::block_on(actor_ref.call(
        ClearRuntimeOwnersForPluginMessage {
            plugin_id: plugin_id.to_string(),
        },
        OWNER_WORKER_CLEAR_TIMEOUT,
    )) {
        Ok(result) => result,
        Err(_) => (0, 0, 0),
    }
}

pub(super) fn source_list_items_json_via_runtime_async(
    plugin_id: String,
    type_id: String,
    config_json: String,
    request_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    let slot = RuntimeInstanceSlotKey::new(&plugin_id, &type_id);
    let registry_actor_ref = shared_runtime_owner_registry_actor();
    let actor_ref = match stellatune_runtime::block_on(registry_actor_ref.call(
        EnsureSourceOwnerTaskMessage { slot },
        OWNER_WORKER_CLEAR_TIMEOUT,
    )) {
        Ok(actor_ref) => actor_ref,
        Err(_) => {
            let _ = resp_tx.send(Err("runtime source owner registry unavailable".to_string()));
            return;
        }
    };
    let _ = stellatune_runtime::spawn(async move {
        let result = match actor_ref
            .call(
                SourceListItemsMessage {
                    config_json,
                    request_json,
                },
                OWNER_WORKER_STREAM_TIMEOUT,
            )
            .await
        {
            Ok(result) => result,
            Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
                Err("runtime source owner task list_items timeout".to_string())
            }
            Err(_) => Err("runtime source owner task unavailable".to_string()),
        };
        let _ = resp_tx.send(result);
    });
}

pub(super) fn lyrics_search_json_via_runtime_async(
    plugin_id: String,
    type_id: String,
    query_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    let config_json =
        match runtime_default_config_json(&plugin_id, CapabilityKind::LyricsProvider, &type_id) {
            Ok(v) => v,
            Err(e) => {
                let _ = resp_tx.send(Err(e));
                return;
            }
        };
    let slot = RuntimeInstanceSlotKey::new(&plugin_id, &type_id);
    let registry_actor_ref = shared_runtime_owner_registry_actor();
    let actor_ref = match stellatune_runtime::block_on(registry_actor_ref.call(
        EnsureLyricsOwnerTaskMessage { slot },
        OWNER_WORKER_CLEAR_TIMEOUT,
    )) {
        Ok(actor_ref) => actor_ref,
        Err(_) => {
            let _ = resp_tx.send(Err("runtime lyrics owner registry unavailable".to_string()));
            return;
        }
    };
    let _ = stellatune_runtime::spawn(async move {
        let result = match actor_ref
            .call(
                LyricsSearchMessage {
                    config_json,
                    query_json,
                },
                OWNER_WORKER_STREAM_TIMEOUT,
            )
            .await
        {
            Ok(result) => result,
            Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
                Err("runtime lyrics owner task search timeout".to_string())
            }
            Err(_) => Err("runtime lyrics owner task unavailable".to_string()),
        };
        let _ = resp_tx.send(result);
    });
}

pub(super) fn lyrics_fetch_json_via_runtime_async(
    plugin_id: String,
    type_id: String,
    track_json: String,
    resp_tx: OneshotSender<Result<String, String>>,
) {
    let config_json =
        match runtime_default_config_json(&plugin_id, CapabilityKind::LyricsProvider, &type_id) {
            Ok(v) => v,
            Err(e) => {
                let _ = resp_tx.send(Err(e));
                return;
            }
        };
    let slot = RuntimeInstanceSlotKey::new(&plugin_id, &type_id);
    let registry_actor_ref = shared_runtime_owner_registry_actor();
    let actor_ref = match stellatune_runtime::block_on(registry_actor_ref.call(
        EnsureLyricsOwnerTaskMessage { slot },
        OWNER_WORKER_CLEAR_TIMEOUT,
    )) {
        Ok(actor_ref) => actor_ref,
        Err(_) => {
            let _ = resp_tx.send(Err("runtime lyrics owner registry unavailable".to_string()));
            return;
        }
    };
    let _ = stellatune_runtime::spawn(async move {
        let result = match actor_ref
            .call(
                LyricsFetchMessage {
                    config_json,
                    track_json,
                },
                OWNER_WORKER_STREAM_TIMEOUT,
            )
            .await
        {
            Ok(result) => result,
            Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
                Err("runtime lyrics owner task fetch timeout".to_string())
            }
            Err(_) => Err("runtime lyrics owner task unavailable".to_string()),
        };
        let _ = resp_tx.send(result);
    });
}

pub(crate) fn source_open_stream_via_runtime_blocking(
    plugin_id: &str,
    type_id: &str,
    config_json: String,
    track_json: String,
) -> Result<RuntimeSourceStreamLease, String> {
    let slot = RuntimeInstanceSlotKey::new(plugin_id, type_id);
    let registry_actor_ref = shared_runtime_owner_registry_actor();
    let (actor_ref, stream_id) = match stellatune_runtime::block_on(registry_actor_ref.call(
        PrepareSourceOpenStreamMessage { slot: slot.clone() },
        OWNER_WORKER_CLEAR_TIMEOUT,
    )) {
        Ok(v) => v,
        Err(_) => return Err("runtime source owner registry unavailable".to_string()),
    };

    let rollback_active = || {
        let registry_actor_ref = shared_runtime_owner_registry_actor();
        let _ = stellatune_runtime::block_on(registry_actor_ref.call(
            RollbackSourceOpenStreamMessage { slot: slot.clone() },
            OWNER_WORKER_CLEAR_TIMEOUT,
        ));
    };

    match stellatune_runtime::block_on(actor_ref.call(
        SourceOpenStreamMessage {
            config_json,
            track_json,
            stream_id,
        },
        OWNER_WORKER_STREAM_TIMEOUT,
    )) {
        Ok(Ok(lease)) => {
            let registry_actor_ref = shared_runtime_owner_registry_actor();
            let _ = stellatune_runtime::block_on(registry_actor_ref.call(
                CommitSourceOpenStreamMessage {
                    stream_id: lease.stream_id,
                    slot,
                },
                OWNER_WORKER_CLEAR_TIMEOUT,
            ));
            Ok(lease)
        }
        Ok(Err(e)) => {
            rollback_active();
            Err(e)
        }
        Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
            let _ = stellatune_runtime::block_on(actor_ref.call(
                SourceCloseStreamMessage { stream_id },
                OWNER_WORKER_CLEAR_TIMEOUT,
            ));
            rollback_active();
            Err("runtime source owner task open_stream timeout".to_string())
        }
        Err(_) => {
            rollback_active();
            Err("runtime source owner task unavailable".to_string())
        }
    }
}

pub(crate) fn source_close_stream_via_runtime_blocking(stream_id: u64) -> Result<(), String> {
    let registry_actor_ref = shared_runtime_owner_registry_actor();
    let (slot, actor_ref) = match stellatune_runtime::block_on(registry_actor_ref.call(
        PrepareSourceCloseStreamMessage { stream_id },
        OWNER_WORKER_CLEAR_TIMEOUT,
    )) {
        Ok(SourceCloseTarget::MissingStream) => return Ok(()),
        Ok(SourceCloseTarget::MissingTask) => {
            return Err("runtime source owner task missing for close_stream".to_string());
        }
        Ok(SourceCloseTarget::Ready { slot, actor_ref }) => (slot, actor_ref),
        Err(_) => return Err("runtime source owner registry unavailable".to_string()),
    };

    let result = match stellatune_runtime::block_on(actor_ref.call(
        SourceCloseStreamMessage { stream_id },
        OWNER_WORKER_STREAM_TIMEOUT,
    )) {
        Ok(result) => result,
        Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
            return Err("runtime source owner task close_stream timeout".to_string());
        }
        Err(_) => {
            let registry_actor_ref = shared_runtime_owner_registry_actor();
            let _ = stellatune_runtime::block_on(registry_actor_ref.call(
                FinalizeSourceCloseStreamMessage { slot, stream_id },
                OWNER_WORKER_CLEAR_TIMEOUT,
            ));
            return Err("runtime source owner task unavailable".to_string());
        }
    };

    let registry_actor_ref = shared_runtime_owner_registry_actor();
    let _ = stellatune_runtime::block_on(registry_actor_ref.call(
        FinalizeSourceCloseStreamMessage { slot, stream_id },
        OWNER_WORKER_CLEAR_TIMEOUT,
    ));
    result
}

pub(super) fn clear_runtime_query_instance_cache(state: &mut EngineState) {
    state.output_sink_instances.clear();
    state.output_sink_negotiation_cache = None;
    clear_runtime_owner_worker_cache();
}

pub(super) fn output_sink_list_targets_json_via_runtime(
    _state: &mut EngineState,
    plugin_id: &str,
    type_id: &str,
    config_json: String,
) -> Result<String, String> {
    let slot = RuntimeInstanceSlotKey::new(plugin_id, type_id);
    let registry_actor_ref = shared_runtime_owner_registry_actor();
    let actor_ref = match stellatune_runtime::block_on(registry_actor_ref.call(
        EnsureOutputSinkOwnerTaskMessage { slot },
        OWNER_WORKER_CLEAR_TIMEOUT,
    )) {
        Ok(actor_ref) => actor_ref,
        Err(_) => return Err("runtime output sink owner registry unavailable".to_string()),
    };
    match stellatune_runtime::block_on(actor_ref.call(
        OutputSinkListTargetsMessage { config_json },
        OWNER_WORKER_STREAM_TIMEOUT,
    )) {
        Ok(result) => result,
        Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
            Err("runtime output sink owner task list_targets timeout".to_string())
        }
        Err(_) => Err("runtime output sink owner task unavailable".to_string()),
    }
}

pub(super) fn clear_runtime_query_instance_cache_for_plugin(
    state: &mut EngineState,
    plugin_id: &str,
) -> (usize, usize, usize) {
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

    let (worker_source_removed, worker_lyrics_removed, worker_output_sink_removed) =
        clear_runtime_owner_worker_cache_for_plugin(plugin_id);

    (
        worker_source_removed,
        worker_lyrics_removed,
        output_sink_removed.saturating_add(worker_output_sink_removed),
    )
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
