use crate::stream::StreamState;

#[derive(Default)]
pub(crate) struct RuntimeState {
    pub(crate) stream: Option<StreamState>,
    pub(crate) active_device_id: Option<String>,
    pub(crate) device_snapshot: Vec<DeviceSnapshotEntry>,
    pub(crate) last_live_device_ids: Vec<String>,
}

pub(crate) struct DeviceSnapshotEntry {
    pub(crate) selection_session_id: String,
    pub(crate) id: String,
    pub(crate) name: String,
}
