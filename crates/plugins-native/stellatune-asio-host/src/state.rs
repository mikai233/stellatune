use crate::data_channel::DataIngressPump;
use crate::stream::StreamState;

pub(crate) struct RuntimeState {
    pub(crate) stream: Option<StreamState>,
    pub(crate) active_device_id: Option<String>,
    pub(crate) device_snapshot: Vec<DeviceSnapshotEntry>,
    pub(crate) data_ingress: Option<DataIngressPump>,
}

impl RuntimeState {
    pub(crate) fn new(data_ingress: Option<DataIngressPump>) -> Self {
        Self {
            stream: None,
            active_device_id: None,
            device_snapshot: Vec::new(),
            data_ingress,
        }
    }
}

pub(crate) struct DeviceSnapshotEntry {
    pub(crate) selection_session_id: String,
    pub(crate) id: String,
    pub(crate) name: String,
}
