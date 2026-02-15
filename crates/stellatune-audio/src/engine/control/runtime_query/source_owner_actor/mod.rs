pub(super) mod handlers;

use std::collections::HashMap;

use super::{
    CachedSourceInstance, RuntimeInstanceSlotKey, create_source_catalog_cached_instance,
    sync_source_runtime_control, with_runtime_service,
};

pub(crate) struct SourceOwnerActor {
    pub(super) slot: RuntimeInstanceSlotKey,
    pub(super) frozen: bool,
    pub(super) current: Option<SourceCatalogLease>,
    pub(super) retired: HashMap<u64, CachedSourceInstance>,
    pub(super) streams: HashMap<u64, SourceStreamLeaseRecord>,
    next_lease_id: u64,
}

pub(super) struct SourceStreamLeaseRecord {
    pub(super) lease_id: u64,
    pub(super) io_handle_addr: usize,
}

pub(super) struct SourceCatalogLease {
    pub(super) lease_id: u64,
    pub(super) config_json: String,
    pub(super) entry: CachedSourceInstance,
}

impl SourceOwnerActor {
    pub(super) fn new(plugin_id: String, type_id: String) -> Self {
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

    pub(super) fn ensure_current_entry_for_ops(
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
}
