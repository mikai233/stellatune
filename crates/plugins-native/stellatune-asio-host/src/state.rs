use crate::data_channel::DataIngressPump;
use crate::stream::StreamState;

pub(crate) struct RuntimeState {
    pub(crate) stream: Option<StreamState>,
    pub(crate) active_device_id: Option<String>,
    pub(crate) device_snapshot: Vec<DeviceSnapshotEntry>,
    pub(crate) data_ingress: Option<DataIngressPump>,
    pub(crate) pending_switch: Option<PreparedSwitch>,
    next_switch_id: u64,
}

impl RuntimeState {
    pub(crate) fn new(data_ingress: Option<DataIngressPump>) -> Self {
        Self {
            stream: None,
            active_device_id: None,
            device_snapshot: Vec::new(),
            data_ingress,
            pending_switch: None,
            next_switch_id: 1,
        }
    }

    pub(crate) fn clear_active_stream(&mut self) -> Result<bool, String> {
        if let Some(data_ingress) = self.data_ingress.as_ref() {
            data_ingress.set_ingress(None);
            data_ingress.request_reset_and_wait(std::time::Duration::from_millis(500))?;
        }
        let had_stream = self.stream.take().is_some();
        self.active_device_id = None;
        Ok(had_stream)
    }

    pub(crate) fn allocate_prepared_switch(
        &mut self,
        selection_session_id: &str,
        device_id: &str,
    ) -> u64 {
        let prepared_switch_id = self.next_switch_id;
        self.next_switch_id = self.next_switch_id.wrapping_add(1).max(1);
        self.pending_switch = Some(PreparedSwitch {
            prepared_switch_id,
            selection_session_id: selection_session_id.to_string(),
            device_id: device_id.to_string(),
        });
        prepared_switch_id
    }

    pub(crate) fn consume_prepared_switch(
        &mut self,
        prepared_switch_id: u64,
        selection_session_id: &str,
        device_id: &str,
    ) -> Result<(), String> {
        let Some(pending_switch) = self.pending_switch.take() else {
            return Err("missing prepare-device-switch step for this open request".to_string());
        };
        if pending_switch.prepared_switch_id != prepared_switch_id {
            return Err(format!(
                "stale prepare-device-switch token: expected {}, got {}",
                pending_switch.prepared_switch_id, prepared_switch_id
            ));
        }
        if pending_switch.selection_session_id != selection_session_id {
            return Err(format!(
                "prepare-device-switch session mismatch: expected `{}`, got `{}`",
                pending_switch.selection_session_id, selection_session_id
            ));
        }
        if pending_switch.device_id != device_id {
            return Err(format!(
                "prepare-device-switch device mismatch: expected `{}`, got `{}`",
                pending_switch.device_id, device_id
            ));
        }
        Ok(())
    }
}

pub(crate) struct PreparedSwitch {
    pub(crate) prepared_switch_id: u64,
    pub(crate) selection_session_id: String,
    pub(crate) device_id: String,
}

pub(crate) struct DeviceSnapshotEntry {
    pub(crate) selection_session_id: String,
    pub(crate) id: String,
    pub(crate) name: String,
}
