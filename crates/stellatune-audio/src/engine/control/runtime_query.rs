use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crossbeam_channel::{Sender, bounded};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::Sender as OneshotSender;
use tracing::debug;
use tracing::warn;

use stellatune_plugins::runtime::introspection::CapabilityKind;
use stellatune_plugins::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome,
};

use super::{
    CachedLyricsInstance, CachedOutputSinkInstance, CachedSourceInstance, EngineState,
    RuntimeInstanceSlotKey, emit_config_update_runtime_event, runtime_default_config_json,
    with_runtime_service,
};

const OWNER_WORKER_CLEAR_TIMEOUT: Duration = Duration::from_millis(500);
const OWNER_WORKER_STREAM_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSourceStreamLease {
    pub stream_id: u64,
    pub lease_id: u64,
    pub io_vtable_addr: usize,
    pub io_handle_addr: usize,
    pub source_metadata_json: Option<String>,
}

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

fn clear_runtime_owner_worker_cache() {
    let (source_freeze_txs, source_shutdown_txs, lyrics_freeze_txs, lyrics_shutdown_txs) = {
        let mut registry = lock_runtime_owner_registry();

        let source_freeze_txs: Vec<_> = registry
            .source_tasks
            .iter()
            .filter_map(|(_, handle)| {
                if handle.frozen {
                    None
                } else {
                    Some(handle.tx.clone())
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
        let source_shutdown_txs: Vec<_> = removable_source_slots
            .iter()
            .filter_map(|slot| registry.source_tasks.remove(slot).map(|h| h.tx))
            .collect();
        registry
            .source_stream_slots
            .retain(|_, slot| !removable_source_slots.iter().any(|s| s == slot));

        let lyrics_freeze_txs: Vec<_> = registry
            .lyrics_tasks
            .iter()
            .filter_map(|(_, handle)| {
                if handle.frozen {
                    None
                } else {
                    Some(handle.tx.clone())
                }
            })
            .collect();
        for handle in registry.lyrics_tasks.values_mut() {
            handle.frozen = true;
        }

        let lyrics_slots: Vec<_> = registry.lyrics_tasks.keys().cloned().collect();
        let lyrics_shutdown_txs: Vec<_> = lyrics_slots
            .iter()
            .filter_map(|slot| registry.lyrics_tasks.remove(slot).map(|h| h.tx))
            .collect();
        (
            source_freeze_txs,
            source_shutdown_txs,
            lyrics_freeze_txs,
            lyrics_shutdown_txs,
        )
    };

    for tx in source_freeze_txs {
        send_source_task_freeze(tx);
    }
    for tx in source_shutdown_txs {
        send_source_task_shutdown(tx);
    }
    for tx in lyrics_freeze_txs {
        send_lyrics_task_freeze(tx);
    }
    for tx in lyrics_shutdown_txs {
        send_lyrics_task_shutdown(tx);
    }
}

fn clear_runtime_owner_worker_cache_for_plugin(plugin_id: &str) -> (usize, usize, usize) {
    let (
        source_removed,
        lyrics_removed,
        source_freeze_txs,
        source_shutdown_txs,
        lyrics_freeze_txs,
        lyrics_shutdown_txs,
    ) = {
        let mut registry = lock_runtime_owner_registry();

        let source_freeze_txs: Vec<_> = registry
            .source_tasks
            .iter()
            .filter_map(|(slot, handle)| {
                if slot.plugin_id.as_str() == plugin_id && !handle.frozen {
                    Some(handle.tx.clone())
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
        let source_shutdown_txs: Vec<_> = removable_source_slots
            .iter()
            .filter_map(|slot| registry.source_tasks.remove(slot).map(|h| h.tx))
            .collect();
        registry
            .source_stream_slots
            .retain(|_, slot| !removable_source_slots.iter().any(|s| s == slot));

        let lyrics_freeze_txs: Vec<_> = registry
            .lyrics_tasks
            .iter()
            .filter_map(|(slot, handle)| {
                if slot.plugin_id.as_str() == plugin_id && !handle.frozen {
                    Some(handle.tx.clone())
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
        let lyrics_shutdown_txs: Vec<_> = removable_lyrics_slots
            .iter()
            .filter_map(|slot| registry.lyrics_tasks.remove(slot).map(|h| h.tx))
            .collect();

        (
            removable_source_slots.len(),
            removable_lyrics_slots.len(),
            source_freeze_txs,
            source_shutdown_txs,
            lyrics_freeze_txs,
            lyrics_shutdown_txs,
        )
    };

    for tx in source_freeze_txs {
        send_source_task_freeze(tx);
    }
    for tx in source_shutdown_txs {
        send_source_task_shutdown(tx);
    }
    for tx in lyrics_freeze_txs {
        send_lyrics_task_freeze(tx);
    }
    for tx in lyrics_shutdown_txs {
        send_lyrics_task_shutdown(tx);
    }

    (source_removed, lyrics_removed, 0)
}

enum SourceOwnerTaskRequest {
    ListItems {
        config_json: String,
        request_json: String,
        resp_tx: OneshotSender<Result<String, String>>,
    },
    OpenStream {
        config_json: String,
        track_json: String,
        stream_id: u64,
        resp_tx: Sender<Result<RuntimeSourceStreamLease, String>>,
    },
    CloseStream {
        stream_id: u64,
        resp_tx: Sender<Result<(), String>>,
    },
    Freeze {
        ack_tx: Sender<()>,
    },
    Shutdown {
        ack_tx: Sender<()>,
    },
}

enum LyricsOwnerTaskRequest {
    Search {
        config_json: String,
        query_json: String,
        resp_tx: OneshotSender<Result<String, String>>,
    },
    Fetch {
        config_json: String,
        track_json: String,
        resp_tx: OneshotSender<Result<String, String>>,
    },
    Freeze {
        ack_tx: Sender<()>,
    },
    Shutdown {
        ack_tx: Sender<()>,
    },
}

struct SourceOwnerTaskHandle {
    tx: UnboundedSender<SourceOwnerTaskRequest>,
    active_streams: usize,
    frozen: bool,
}

struct LyricsOwnerTaskHandle {
    tx: UnboundedSender<LyricsOwnerTaskRequest>,
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

struct SourceStreamLeaseRecord {
    lease_id: u64,
    io_handle_addr: usize,
}

struct SourceCatalogLease {
    lease_id: u64,
    config_json: String,
    entry: CachedSourceInstance,
}

struct SourceOwnerTaskState {
    slot: RuntimeInstanceSlotKey,
    frozen: bool,
    current: Option<SourceCatalogLease>,
    retired: HashMap<u64, CachedSourceInstance>,
    streams: HashMap<u64, SourceStreamLeaseRecord>,
    next_lease_id: u64,
}

impl SourceOwnerTaskState {
    fn new(plugin_id: String, type_id: String) -> Self {
        Self {
            slot: RuntimeInstanceSlotKey { plugin_id, type_id },
            frozen: false,
            current: None,
            retired: HashMap::new(),
            streams: HashMap::new(),
            next_lease_id: 1,
        }
    }

    fn next_lease_id(&mut self) -> u64 {
        let mut id = self.next_lease_id;
        if id == 0 {
            id = 1;
        }
        self.next_lease_id = id.wrapping_add(1);
        id
    }

    fn active_streams_for_lease(&self, lease_id: u64) -> usize {
        self.streams
            .values()
            .filter(|v| v.lease_id == lease_id)
            .count()
    }

    fn observe_runtime_control_only(entry: &mut CachedSourceInstance) {
        while let Ok(message) = entry.control_rx.try_recv() {
            entry.controller.on_control_message(message);
        }
    }

    fn create_source_entry(&self, config_json: &str) -> Result<CachedSourceInstance, String> {
        with_runtime_service(|service| {
            create_source_catalog_cached_instance(
                service,
                &self.slot.plugin_id,
                &self.slot.type_id,
                config_json,
            )
        })
    }

    fn move_current_to_retired_if_needed(&mut self) {
        let Some(current) = self.current.take() else {
            return;
        };
        if self.active_streams_for_lease(current.lease_id) > 0 {
            self.retired.insert(current.lease_id, current.entry);
        }
    }

    fn install_new_current(&mut self, config_json: &str) -> Result<(), String> {
        let lease_id = self.next_lease_id();
        let created = self.create_source_entry(config_json)?;
        self.current = Some(SourceCatalogLease {
            lease_id,
            config_json: config_json.to_string(),
            entry: created,
        });
        Ok(())
    }

    fn ensure_current_entry_for_ops(
        &mut self,
        config_json: &str,
    ) -> Result<&mut SourceCatalogLease, String> {
        if self.frozen {
            return Err(format!(
                "source owner frozen for {}::{}",
                self.slot.plugin_id, self.slot.type_id
            ));
        }

        if self.current.is_none() {
            self.install_new_current(config_json)?;
        }

        let config_mismatch = self
            .current
            .as_ref()
            .map(|c| c.config_json.as_str() != config_json)
            .unwrap_or(false);
        if config_mismatch {
            self.move_current_to_retired_if_needed();
            self.install_new_current(config_json)?;
        }

        let lease_id = self
            .current
            .as_ref()
            .map(|c| c.lease_id)
            .ok_or_else(|| "source current lease missing".to_string())?;
        let active = self.active_streams_for_lease(lease_id);
        let plugin_id = self.slot.plugin_id.clone();
        let type_id = self.slot.type_id.clone();

        if active > 0 {
            {
                let current = self
                    .current
                    .as_mut()
                    .ok_or_else(|| "source current lease unavailable".to_string())?;
                Self::observe_runtime_control_only(&mut current.entry);
                if current.entry.controller.has_pending_destroy()
                    || current.entry.controller.has_pending_recreate()
                    || current.entry.controller.instance().is_none()
                {
                    self.frozen = true;
                    return Err(format!(
                        "source owner frozen waiting active streams to drain for {plugin_id}::{type_id}"
                    ));
                }
            }
            return self
                .current
                .as_mut()
                .ok_or_else(|| "source current lease unavailable".to_string());
        }

        let current_missing = {
            let current = self
                .current
                .as_mut()
                .ok_or_else(|| "source current lease unavailable".to_string())?;
            sync_source_runtime_control(&plugin_id, &type_id, &mut current.entry)?;
            current.entry.controller.instance().is_none()
        };
        if current_missing {
            self.move_current_to_retired_if_needed();
            self.install_new_current(config_json)?;
            return self
                .current
                .as_mut()
                .ok_or_else(|| "source current lease unavailable after recreate".to_string());
        }
        self.current
            .as_mut()
            .ok_or_else(|| "source current lease unavailable".to_string())
    }

    fn close_stream(&mut self, stream_id: u64) -> Result<(), String> {
        let Some(record) = self.streams.remove(&stream_id) else {
            return Ok(());
        };

        if self
            .current
            .as_ref()
            .is_some_and(|c| c.lease_id == record.lease_id)
        {
            let current = self
                .current
                .as_mut()
                .ok_or_else(|| "source current lease missing while closing stream".to_string())?;
            let instance = current.entry.controller.instance_mut().ok_or_else(|| {
                format!("source instance unavailable while closing stream_id={stream_id}")
            })?;
            instance.close_stream(record.io_handle_addr as *mut core::ffi::c_void);
        } else if let Some(entry) = self.retired.get_mut(&record.lease_id) {
            let instance = entry.controller.instance_mut().ok_or_else(|| {
                format!(
                    "source retired instance unavailable while closing stream_id={stream_id} lease_id={}",
                    record.lease_id
                )
            })?;
            instance.close_stream(record.io_handle_addr as *mut core::ffi::c_void);
        } else {
            return Err(format!(
                "source lease missing while closing stream_id={stream_id} lease_id={}",
                record.lease_id
            ));
        }

        if self.active_streams_for_lease(record.lease_id) == 0 {
            self.retired.remove(&record.lease_id);
        }
        Ok(())
    }
}

struct LyricsOwnerTaskState {
    slot: RuntimeInstanceSlotKey,
    frozen: bool,
    entry: Option<CachedLyricsInstance>,
}

impl LyricsOwnerTaskState {
    fn new(plugin_id: String, type_id: String) -> Self {
        Self {
            slot: RuntimeInstanceSlotKey { plugin_id, type_id },
            frozen: false,
            entry: None,
        }
    }

    fn ensure_entry(&mut self, config_json: &str) -> Result<&mut CachedLyricsInstance, String> {
        if self.frozen {
            return Err(format!(
                "lyrics owner frozen for {}::{}",
                self.slot.plugin_id, self.slot.type_id
            ));
        }
        if self.entry.is_none() {
            let created = with_runtime_service(|service| {
                create_lyrics_provider_cached_instance(
                    service,
                    &self.slot.plugin_id,
                    &self.slot.type_id,
                    config_json,
                )
            })?;
            self.entry = Some(created);
        }
        let entry = self
            .entry
            .as_mut()
            .ok_or_else(|| "lyrics owner task cache insertion failed".to_string())?;
        apply_or_recreate_lyrics_instance(
            &self.slot.plugin_id,
            &self.slot.type_id,
            entry,
            config_json,
        )?;
        Ok(entry)
    }
}

async fn run_source_owner_task(
    plugin_id: String,
    type_id: String,
    mut rx: UnboundedReceiver<SourceOwnerTaskRequest>,
) {
    let mut state = SourceOwnerTaskState::new(plugin_id, type_id);
    while let Some(request) = rx.recv().await {
        match request {
            SourceOwnerTaskRequest::ListItems {
                config_json,
                request_json,
                resp_tx,
            } => {
                let plugin_id = state.slot.plugin_id.clone();
                let type_id = state.slot.type_id.clone();
                let result = match state.ensure_current_entry_for_ops(&config_json) {
                    Ok(current) => {
                        let instance = current.entry.controller.instance_mut().ok_or_else(|| {
                            format!("source instance unavailable for {}::{}", plugin_id, type_id)
                        });
                        match instance {
                            Ok(instance) => instance
                                .list_items_json(&request_json)
                                .await
                                .map_err(|e| e.to_string()),
                            Err(err) => Err(err),
                        }
                    }
                    Err(err) => Err(err),
                };
                let _ = resp_tx.send(result);
            }
            SourceOwnerTaskRequest::OpenStream {
                config_json,
                track_json,
                stream_id,
                resp_tx,
            } => {
                let plugin_id = state.slot.plugin_id.clone();
                let type_id = state.slot.type_id.clone();
                let result = match state.ensure_current_entry_for_ops(&config_json) {
                    Ok(current) => {
                        let lease_id = current.lease_id;
                        let instance = current.entry.controller.instance_mut().ok_or_else(|| {
                            format!("source instance unavailable for {}::{}", plugin_id, type_id)
                        });
                        match instance {
                            Ok(instance) => {
                                let opened = instance
                                    .open_stream(&track_json)
                                    .await
                                    .map_err(|e| e.to_string());
                                match opened {
                                    Ok((stream, source_metadata_json)) => {
                                        let io_vtable_addr = stream.io_vtable as usize;
                                        let io_handle_addr = stream.io_handle as usize;
                                        if io_vtable_addr == 0 || io_handle_addr == 0 {
                                            if io_handle_addr != 0 {
                                                instance.close_stream(stream.io_handle);
                                            }
                                            Err(
                                                "source open_stream returned null io_vtable/io_handle"
                                                    .to_string(),
                                            )
                                        } else {
                                            state.streams.insert(
                                                stream_id,
                                                SourceStreamLeaseRecord {
                                                    lease_id,
                                                    io_handle_addr,
                                                },
                                            );
                                            Ok(RuntimeSourceStreamLease {
                                                stream_id,
                                                lease_id,
                                                io_vtable_addr,
                                                io_handle_addr,
                                                source_metadata_json,
                                            })
                                        }
                                    }
                                    Err(err) => Err(err),
                                }
                            }
                            Err(err) => Err(err),
                        }
                    }
                    Err(err) => Err(err),
                };
                let _ = resp_tx.send(result);
            }
            SourceOwnerTaskRequest::CloseStream { stream_id, resp_tx } => {
                let result = state.close_stream(stream_id);
                let _ = resp_tx.send(result);
            }
            SourceOwnerTaskRequest::Freeze { ack_tx } => {
                state.frozen = true;
                let _ = ack_tx.send(());
            }
            SourceOwnerTaskRequest::Shutdown { ack_tx } => {
                if state.streams.is_empty() {
                    state.current = None;
                    state.retired.clear();
                    let _ = ack_tx.send(());
                    break;
                }
                state.frozen = true;
                let _ = ack_tx.send(());
            }
        }
    }
}

async fn run_lyrics_owner_task(
    plugin_id: String,
    type_id: String,
    mut rx: UnboundedReceiver<LyricsOwnerTaskRequest>,
) {
    let mut state = LyricsOwnerTaskState::new(plugin_id, type_id);
    while let Some(request) = rx.recv().await {
        match request {
            LyricsOwnerTaskRequest::Search {
                config_json,
                query_json,
                resp_tx,
            } => {
                let plugin_id = state.slot.plugin_id.clone();
                let type_id = state.slot.type_id.clone();
                let result = (|| {
                    let entry = state.ensure_entry(&config_json)?;
                    let instance = entry.controller.instance_mut().ok_or_else(|| {
                        format!("lyrics instance unavailable for {}::{}", plugin_id, type_id)
                    })?;
                    instance.search_json(&query_json).map_err(|e| e.to_string())
                })();
                let _ = resp_tx.send(result);
            }
            LyricsOwnerTaskRequest::Fetch {
                config_json,
                track_json,
                resp_tx,
            } => {
                let plugin_id = state.slot.plugin_id.clone();
                let type_id = state.slot.type_id.clone();
                let result = (|| {
                    let entry = state.ensure_entry(&config_json)?;
                    let instance = entry.controller.instance_mut().ok_or_else(|| {
                        format!("lyrics instance unavailable for {}::{}", plugin_id, type_id)
                    })?;
                    instance.fetch_json(&track_json).map_err(|e| e.to_string())
                })();
                let _ = resp_tx.send(result);
            }
            LyricsOwnerTaskRequest::Freeze { ack_tx } => {
                state.frozen = true;
                let _ = ack_tx.send(());
            }
            LyricsOwnerTaskRequest::Shutdown { ack_tx } => {
                state.entry = None;
                let _ = ack_tx.send(());
                break;
            }
        }
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
) -> UnboundedSender<SourceOwnerTaskRequest> {
    if let Some(handle) = registry.source_tasks.get(slot)
        && !handle.tx.is_closed()
    {
        return handle.tx.clone();
    }

    let (tx, rx) = mpsc::unbounded_channel::<SourceOwnerTaskRequest>();
    let plugin_id = slot.plugin_id.clone();
    let type_id = slot.type_id.clone();
    let active_streams = registry
        .source_tasks
        .get(slot)
        .map(|h| h.active_streams)
        .unwrap_or(0);
    let _ = stellatune_runtime::spawn(run_source_owner_task(plugin_id, type_id, rx));
    registry.source_tasks.insert(
        slot.clone(),
        SourceOwnerTaskHandle {
            tx: tx.clone(),
            active_streams,
            frozen: false,
        },
    );
    tx
}

fn ensure_lyrics_owner_task_locked(
    registry: &mut RuntimeOwnerRegistry,
    slot: &RuntimeInstanceSlotKey,
) -> UnboundedSender<LyricsOwnerTaskRequest> {
    if let Some(handle) = registry.lyrics_tasks.get(slot)
        && !handle.tx.is_closed()
    {
        return handle.tx.clone();
    }
    let (tx, rx) = mpsc::unbounded_channel::<LyricsOwnerTaskRequest>();
    let plugin_id = slot.plugin_id.clone();
    let type_id = slot.type_id.clone();
    let _ = stellatune_runtime::spawn(run_lyrics_owner_task(plugin_id, type_id, rx));
    registry.lyrics_tasks.insert(
        slot.clone(),
        LyricsOwnerTaskHandle {
            tx: tx.clone(),
            frozen: false,
        },
    );
    tx
}

fn send_source_task_shutdown(tx: UnboundedSender<SourceOwnerTaskRequest>) {
    let (ack_tx, ack_rx) = bounded::<()>(1);
    if tx.send(SourceOwnerTaskRequest::Shutdown { ack_tx }).is_ok()
        && ack_rx.recv_timeout(OWNER_WORKER_CLEAR_TIMEOUT).is_err()
    {
        warn!("source owner task shutdown timeout");
    }
}

fn send_lyrics_task_shutdown(tx: UnboundedSender<LyricsOwnerTaskRequest>) {
    let (ack_tx, ack_rx) = bounded::<()>(1);
    if tx.send(LyricsOwnerTaskRequest::Shutdown { ack_tx }).is_ok()
        && ack_rx.recv_timeout(OWNER_WORKER_CLEAR_TIMEOUT).is_err()
    {
        warn!("lyrics owner task shutdown timeout");
    }
}

fn send_source_task_freeze(tx: UnboundedSender<SourceOwnerTaskRequest>) {
    let (ack_tx, ack_rx) = bounded::<()>(1);
    if tx.send(SourceOwnerTaskRequest::Freeze { ack_tx }).is_ok()
        && ack_rx.recv_timeout(OWNER_WORKER_CLEAR_TIMEOUT).is_err()
    {
        warn!("source owner task freeze timeout");
    }
}

fn send_lyrics_task_freeze(tx: UnboundedSender<LyricsOwnerTaskRequest>) {
    let (ack_tx, ack_rx) = bounded::<()>(1);
    if tx.send(LyricsOwnerTaskRequest::Freeze { ack_tx }).is_ok()
        && ack_rx.recv_timeout(OWNER_WORKER_CLEAR_TIMEOUT).is_err()
    {
        warn!("lyrics owner task freeze timeout");
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
    let tx = {
        let mut registry = lock_runtime_owner_registry();
        ensure_source_owner_task_locked(&mut registry, &slot)
    };
    let req = SourceOwnerTaskRequest::ListItems {
        config_json,
        request_json,
        resp_tx,
    };
    if let Err(err) = tx.send(req)
        && let SourceOwnerTaskRequest::ListItems { resp_tx, .. } = err.0
    {
        let _ = resp_tx.send(Err("runtime source owner task unavailable".to_string()));
    }
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
    let tx = {
        let mut registry = lock_runtime_owner_registry();
        ensure_lyrics_owner_task_locked(&mut registry, &slot)
    };
    let req = LyricsOwnerTaskRequest::Search {
        config_json,
        query_json,
        resp_tx,
    };
    if let Err(err) = tx.send(req)
        && let LyricsOwnerTaskRequest::Search { resp_tx, .. } = err.0
    {
        let _ = resp_tx.send(Err("runtime lyrics owner task unavailable".to_string()));
    }
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
    let tx = {
        let mut registry = lock_runtime_owner_registry();
        ensure_lyrics_owner_task_locked(&mut registry, &slot)
    };
    let req = LyricsOwnerTaskRequest::Fetch {
        config_json,
        track_json,
        resp_tx,
    };
    if let Err(err) = tx.send(req)
        && let LyricsOwnerTaskRequest::Fetch { resp_tx, .. } = err.0
    {
        let _ = resp_tx.send(Err("runtime lyrics owner task unavailable".to_string()));
    }
}

pub(crate) fn source_open_stream_via_runtime_blocking(
    plugin_id: &str,
    type_id: &str,
    config_json: String,
    track_json: String,
) -> Result<RuntimeSourceStreamLease, String> {
    let slot = RuntimeInstanceSlotKey::new(plugin_id, type_id);
    let (tx, stream_id) = {
        let mut registry = lock_runtime_owner_registry();
        let tx = ensure_source_owner_task_locked(&mut registry, &slot);
        if let Some(handle) = registry.source_tasks.get_mut(&slot) {
            handle.active_streams = handle.active_streams.saturating_add(1);
        }
        let stream_id = registry.next_stream_id();
        (tx, stream_id)
    };

    let rollback_active = || {
        let mut registry = lock_runtime_owner_registry();
        if let Some(handle) = registry.source_tasks.get_mut(&slot) {
            handle.active_streams = handle.active_streams.saturating_sub(1);
        }
    };

    let (resp_tx, resp_rx) = bounded::<Result<RuntimeSourceStreamLease, String>>(1);
    if tx
        .send(SourceOwnerTaskRequest::OpenStream {
            config_json,
            track_json,
            stream_id,
            resp_tx,
        })
        .is_err()
    {
        rollback_active();
        return Err("runtime source owner task unavailable".to_string());
    }

    match resp_rx.recv_timeout(OWNER_WORKER_STREAM_TIMEOUT) {
        Ok(Ok(lease)) => {
            let mut registry = lock_runtime_owner_registry();
            registry.source_stream_slots.insert(lease.stream_id, slot);
            Ok(lease)
        }
        Ok(Err(e)) => {
            rollback_active();
            Err(e)
        }
        Err(_) => {
            let (close_tx, _close_rx) = bounded::<Result<(), String>>(1);
            let _ = tx.send(SourceOwnerTaskRequest::CloseStream {
                stream_id,
                resp_tx: close_tx,
            });
            rollback_active();
            Err("runtime source owner task open_stream timeout".to_string())
        }
    }
}

pub(crate) fn source_close_stream_via_runtime_blocking(stream_id: u64) -> Result<(), String> {
    let (slot, tx) = {
        let registry = lock_runtime_owner_registry();
        let Some(slot) = registry.source_stream_slots.get(&stream_id).cloned() else {
            return Ok(());
        };
        let Some(handle) = registry.source_tasks.get(&slot) else {
            return Err("runtime source owner task missing for close_stream".to_string());
        };
        (slot, handle.tx.clone())
    };

    let (resp_tx, resp_rx) = bounded::<Result<(), String>>(1);
    if tx
        .send(SourceOwnerTaskRequest::CloseStream { stream_id, resp_tx })
        .is_err()
    {
        let mut shutdown_tx: Option<UnboundedSender<SourceOwnerTaskRequest>> = None;
        let mut registry = lock_runtime_owner_registry();
        registry.source_stream_slots.remove(&stream_id);
        if let Some(handle) = registry.source_tasks.get_mut(&slot) {
            handle.active_streams = handle.active_streams.saturating_sub(1);
            if handle.active_streams == 0 && handle.frozen {
                shutdown_tx = Some(handle.tx.clone());
            }
        }
        if shutdown_tx.is_some() {
            registry.source_tasks.remove(&slot);
        }
        drop(registry);
        if let Some(tx) = shutdown_tx {
            send_source_task_shutdown(tx);
        }
        return Err("runtime source owner task unavailable".to_string());
    }

    let result = resp_rx
        .recv_timeout(OWNER_WORKER_STREAM_TIMEOUT)
        .map_err(|_| "runtime source owner task close_stream timeout".to_string())?;
    let mut shutdown_tx: Option<UnboundedSender<SourceOwnerTaskRequest>> = None;
    let mut registry = lock_runtime_owner_registry();
    registry.source_stream_slots.remove(&stream_id);
    if let Some(handle) = registry.source_tasks.get_mut(&slot) {
        handle.active_streams = handle.active_streams.saturating_sub(1);
        if handle.active_streams == 0 && handle.frozen {
            shutdown_tx = Some(handle.tx.clone());
        }
    }
    if shutdown_tx.is_some() {
        registry.source_tasks.remove(&slot);
    }
    drop(registry);
    if let Some(tx) = shutdown_tx {
        send_source_task_shutdown(tx);
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
