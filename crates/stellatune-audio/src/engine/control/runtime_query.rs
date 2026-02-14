use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::oneshot::Sender as OneshotSender;
use tracing::debug;
use tracing::warn;

use stellatune_plugins::runtime::introspection::CapabilityKind;
use stellatune_plugins::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome,
};

use self::lyrics_owner_actor::LyricsOwnerActor;
use self::lyrics_owner_actor::handlers::fetch::LyricsFetchMessage;
use self::lyrics_owner_actor::handlers::freeze::LyricsFreezeMessage;
use self::lyrics_owner_actor::handlers::search::LyricsSearchMessage;
use self::lyrics_owner_actor::handlers::shutdown::LyricsShutdownMessage;
use self::source_owner_actor::SourceOwnerActor;
use self::source_owner_actor::handlers::close_stream::SourceCloseStreamMessage;
use self::source_owner_actor::handlers::freeze::SourceFreezeMessage;
use self::source_owner_actor::handlers::list_items::SourceListItemsMessage;
use self::source_owner_actor::handlers::open_stream::SourceOpenStreamMessage;
use self::source_owner_actor::handlers::shutdown::SourceShutdownMessage;
use super::{
    CachedLyricsInstance, CachedOutputSinkInstance, CachedSourceInstance, EngineState,
    RuntimeInstanceSlotKey, emit_config_update_runtime_event, runtime_default_config_json,
    with_runtime_service,
};

const OWNER_WORKER_CLEAR_TIMEOUT: Duration = Duration::from_millis(500);
const OWNER_WORKER_STREAM_TIMEOUT: Duration = Duration::from_secs(5);

mod lyrics_owner_actor;
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
    let (source_freeze_refs, source_shutdown_refs, lyrics_freeze_refs, lyrics_shutdown_refs) = {
        let mut registry = lock_runtime_owner_registry();

        let source_freeze_refs: Vec<_> = registry
            .source_tasks
            .iter()
            .filter_map(|(_, handle)| {
                if handle.frozen {
                    None
                } else {
                    Some(handle.actor_ref.clone())
                }
            })
            .collect();
        for handle in registry.source_tasks.values_mut() {
            handle.frozen = true;
        }

        let removable_source_slots: Vec<_> = registry
            .source_tasks
            .iter()
            .filter_map(|(slot, handle)| {
                if handle.active_streams == 0 {
                    Some(slot.clone())
                } else {
                    None
                }
            })
            .collect();
        let source_shutdown_refs: Vec<_> = removable_source_slots
            .iter()
            .filter_map(|slot| registry.source_tasks.remove(slot).map(|h| h.actor_ref))
            .collect();
        registry
            .source_stream_slots
            .retain(|_, slot| !removable_source_slots.iter().any(|s| s == slot));

        let lyrics_freeze_refs: Vec<_> = registry
            .lyrics_tasks
            .iter()
            .filter_map(|(_, handle)| {
                if handle.frozen {
                    None
                } else {
                    Some(handle.actor_ref.clone())
                }
            })
            .collect();
        for handle in registry.lyrics_tasks.values_mut() {
            handle.frozen = true;
        }

        let lyrics_slots: Vec<_> = registry.lyrics_tasks.keys().cloned().collect();
        let lyrics_shutdown_refs: Vec<_> = lyrics_slots
            .iter()
            .filter_map(|slot| registry.lyrics_tasks.remove(slot).map(|h| h.actor_ref))
            .collect();
        (
            source_freeze_refs,
            source_shutdown_refs,
            lyrics_freeze_refs,
            lyrics_shutdown_refs,
        )
    };

    for actor_ref in source_freeze_refs {
        send_source_task_freeze(actor_ref);
    }
    for actor_ref in source_shutdown_refs {
        send_source_task_shutdown(actor_ref);
    }
    for actor_ref in lyrics_freeze_refs {
        send_lyrics_task_freeze(actor_ref);
    }
    for actor_ref in lyrics_shutdown_refs {
        send_lyrics_task_shutdown(actor_ref);
    }
}

fn clear_runtime_owner_worker_cache_for_plugin(plugin_id: &str) -> (usize, usize, usize) {
    let (
        source_removed,
        lyrics_removed,
        source_freeze_refs,
        source_shutdown_refs,
        lyrics_freeze_refs,
        lyrics_shutdown_refs,
    ) = {
        let mut registry = lock_runtime_owner_registry();

        let source_freeze_refs: Vec<_> = registry
            .source_tasks
            .iter()
            .filter_map(|(slot, handle)| {
                if slot.plugin_id.as_str() == plugin_id && !handle.frozen {
                    Some(handle.actor_ref.clone())
                } else {
                    None
                }
            })
            .collect();
        for (slot, handle) in &mut registry.source_tasks {
            if slot.plugin_id.as_str() == plugin_id {
                handle.frozen = true;
            }
        }

        let removable_source_slots: Vec<_> = registry
            .source_tasks
            .iter()
            .filter_map(|(slot, handle)| {
                if slot.plugin_id.as_str() == plugin_id && handle.active_streams == 0 {
                    Some(slot.clone())
                } else {
                    None
                }
            })
            .collect();
        let source_shutdown_refs: Vec<_> = removable_source_slots
            .iter()
            .filter_map(|slot| registry.source_tasks.remove(slot).map(|h| h.actor_ref))
            .collect();
        registry
            .source_stream_slots
            .retain(|_, slot| !removable_source_slots.iter().any(|s| s == slot));

        let lyrics_freeze_refs: Vec<_> = registry
            .lyrics_tasks
            .iter()
            .filter_map(|(slot, handle)| {
                if slot.plugin_id.as_str() == plugin_id && !handle.frozen {
                    Some(handle.actor_ref.clone())
                } else {
                    None
                }
            })
            .collect();
        for (slot, handle) in &mut registry.lyrics_tasks {
            if slot.plugin_id.as_str() == plugin_id {
                handle.frozen = true;
            }
        }

        let removable_lyrics_slots: Vec<_> = registry
            .lyrics_tasks
            .keys()
            .filter(|slot| slot.plugin_id.as_str() == plugin_id)
            .cloned()
            .collect();
        let lyrics_shutdown_refs: Vec<_> = removable_lyrics_slots
            .iter()
            .filter_map(|slot| registry.lyrics_tasks.remove(slot).map(|h| h.actor_ref))
            .collect();

        (
            removable_source_slots.len(),
            removable_lyrics_slots.len(),
            source_freeze_refs,
            source_shutdown_refs,
            lyrics_freeze_refs,
            lyrics_shutdown_refs,
        )
    };

    for actor_ref in source_freeze_refs {
        send_source_task_freeze(actor_ref);
    }
    for actor_ref in source_shutdown_refs {
        send_source_task_shutdown(actor_ref);
    }
    for actor_ref in lyrics_freeze_refs {
        send_lyrics_task_freeze(actor_ref);
    }
    for actor_ref in lyrics_shutdown_refs {
        send_lyrics_task_shutdown(actor_ref);
    }

    (source_removed, lyrics_removed, 0)
}

struct SourceOwnerTaskHandle {
    actor_ref: stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>,
    active_streams: usize,
    frozen: bool,
}

struct LyricsOwnerTaskHandle {
    actor_ref: stellatune_runtime::tokio_actor::ActorRef<LyricsOwnerActor>,
    frozen: bool,
}

struct RuntimeOwnerRegistry {
    source_tasks: HashMap<RuntimeInstanceSlotKey, SourceOwnerTaskHandle>,
    lyrics_tasks: HashMap<RuntimeInstanceSlotKey, LyricsOwnerTaskHandle>,
    source_stream_slots: HashMap<u64, RuntimeInstanceSlotKey>,
    next_source_stream_id: u64,
}

impl RuntimeOwnerRegistry {
    fn new() -> Self {
        Self {
            source_tasks: HashMap::new(),
            lyrics_tasks: HashMap::new(),
            source_stream_slots: HashMap::new(),
            next_source_stream_id: 1,
        }
    }

    fn next_stream_id(&mut self) -> u64 {
        let mut id = self.next_source_stream_id;
        if id == 0 {
            id = 1;
        }
        self.next_source_stream_id = id.wrapping_add(1);
        id
    }
}

fn runtime_owner_registry() -> &'static Mutex<RuntimeOwnerRegistry> {
    static REGISTRY: OnceLock<Mutex<RuntimeOwnerRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(RuntimeOwnerRegistry::new()))
}

fn lock_runtime_owner_registry() -> std::sync::MutexGuard<'static, RuntimeOwnerRegistry> {
    runtime_owner_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn ensure_source_owner_task_locked(
    registry: &mut RuntimeOwnerRegistry,
    slot: &RuntimeInstanceSlotKey,
) -> stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor> {
    if let Some(handle) = registry.source_tasks.get(slot)
        && !handle.actor_ref.is_closed()
    {
        return handle.actor_ref.clone();
    }

    let plugin_id = slot.plugin_id.clone();
    let type_id = slot.type_id.clone();
    let active_streams = registry
        .source_tasks
        .get(slot)
        .map(|h| h.active_streams)
        .unwrap_or(0);
    let (actor_ref, _join) =
        stellatune_runtime::tokio_actor::spawn_actor(SourceOwnerActor::new(plugin_id, type_id));
    registry.source_tasks.insert(
        slot.clone(),
        SourceOwnerTaskHandle {
            actor_ref: actor_ref.clone(),
            active_streams,
            frozen: false,
        },
    );
    actor_ref
}

fn ensure_lyrics_owner_task_locked(
    registry: &mut RuntimeOwnerRegistry,
    slot: &RuntimeInstanceSlotKey,
) -> stellatune_runtime::tokio_actor::ActorRef<LyricsOwnerActor> {
    if let Some(handle) = registry.lyrics_tasks.get(slot)
        && !handle.actor_ref.is_closed()
    {
        return handle.actor_ref.clone();
    }
    let plugin_id = slot.plugin_id.clone();
    let type_id = slot.type_id.clone();
    let (actor_ref, _join) =
        stellatune_runtime::tokio_actor::spawn_actor(LyricsOwnerActor::new(plugin_id, type_id));
    registry.lyrics_tasks.insert(
        slot.clone(),
        LyricsOwnerTaskHandle {
            actor_ref: actor_ref.clone(),
            frozen: false,
        },
    );
    actor_ref
}

fn send_source_task_shutdown(
    actor_ref: stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>,
) {
    match stellatune_runtime::block_on(
        actor_ref.call(SourceShutdownMessage, OWNER_WORKER_CLEAR_TIMEOUT),
    ) {
        Ok(()) => {}
        Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
            warn!("source owner task shutdown timeout");
        }
        Err(_) => {}
    }
}

fn send_lyrics_task_shutdown(
    actor_ref: stellatune_runtime::tokio_actor::ActorRef<LyricsOwnerActor>,
) {
    match stellatune_runtime::block_on(
        actor_ref.call(LyricsShutdownMessage, OWNER_WORKER_CLEAR_TIMEOUT),
    ) {
        Ok(()) => {}
        Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
            warn!("lyrics owner task shutdown timeout");
        }
        Err(_) => {}
    }
}

fn send_source_task_freeze(actor_ref: stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>) {
    match stellatune_runtime::block_on(
        actor_ref.call(SourceFreezeMessage, OWNER_WORKER_CLEAR_TIMEOUT),
    ) {
        Ok(()) => {}
        Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
            warn!("source owner task freeze timeout");
        }
        Err(_) => {}
    }
}

fn send_lyrics_task_freeze(actor_ref: stellatune_runtime::tokio_actor::ActorRef<LyricsOwnerActor>) {
    match stellatune_runtime::block_on(
        actor_ref.call(LyricsFreezeMessage, OWNER_WORKER_CLEAR_TIMEOUT),
    ) {
        Ok(()) => {}
        Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
            warn!("lyrics owner task freeze timeout");
        }
        Err(_) => {}
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
    let actor_ref = {
        let mut registry = lock_runtime_owner_registry();
        ensure_source_owner_task_locked(&mut registry, &slot)
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
    let actor_ref = {
        let mut registry = lock_runtime_owner_registry();
        ensure_lyrics_owner_task_locked(&mut registry, &slot)
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
    let actor_ref = {
        let mut registry = lock_runtime_owner_registry();
        ensure_lyrics_owner_task_locked(&mut registry, &slot)
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
    let (actor_ref, stream_id) = {
        let mut registry = lock_runtime_owner_registry();
        let actor_ref = ensure_source_owner_task_locked(&mut registry, &slot);
        if let Some(handle) = registry.source_tasks.get_mut(&slot) {
            handle.active_streams = handle.active_streams.saturating_add(1);
        }
        let stream_id = registry.next_stream_id();
        (actor_ref, stream_id)
    };

    let rollback_active = || {
        let mut registry = lock_runtime_owner_registry();
        if let Some(handle) = registry.source_tasks.get_mut(&slot) {
            handle.active_streams = handle.active_streams.saturating_sub(1);
        }
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
            let mut registry = lock_runtime_owner_registry();
            registry.source_stream_slots.insert(lease.stream_id, slot);
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
    let (slot, actor_ref) = {
        let registry = lock_runtime_owner_registry();
        let Some(slot) = registry.source_stream_slots.get(&stream_id).cloned() else {
            return Ok(());
        };
        let Some(handle) = registry.source_tasks.get(&slot) else {
            return Err("runtime source owner task missing for close_stream".to_string());
        };
        (slot, handle.actor_ref.clone())
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
            let mut shutdown_ref: Option<
                stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>,
            > = None;
            let mut registry = lock_runtime_owner_registry();
            registry.source_stream_slots.remove(&stream_id);
            if let Some(handle) = registry.source_tasks.get_mut(&slot) {
                handle.active_streams = handle.active_streams.saturating_sub(1);
                if handle.active_streams == 0 && handle.frozen {
                    shutdown_ref = Some(handle.actor_ref.clone());
                }
            }
            if shutdown_ref.is_some() {
                registry.source_tasks.remove(&slot);
            }
            drop(registry);
            if let Some(actor_ref) = shutdown_ref {
                send_source_task_shutdown(actor_ref);
            }
            return Err("runtime source owner task unavailable".to_string());
        }
    };

    let mut shutdown_ref: Option<stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>> =
        None;
    let mut registry = lock_runtime_owner_registry();
    registry.source_stream_slots.remove(&stream_id);
    if let Some(handle) = registry.source_tasks.get_mut(&slot) {
        handle.active_streams = handle.active_streams.saturating_sub(1);
        if handle.active_streams == 0 && handle.frozen {
            shutdown_ref = Some(handle.actor_ref.clone());
        }
    }
    if shutdown_ref.is_some() {
        registry.source_tasks.remove(&slot);
    }
    drop(registry);
    if let Some(actor_ref) = shutdown_ref {
        send_source_task_shutdown(actor_ref);
    }
    result
}

pub(super) fn clear_runtime_query_instance_cache(state: &mut EngineState) {
    state.output_sink_instances.clear();
    state.output_sink_negotiation_cache = None;
    clear_runtime_owner_worker_cache();
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
