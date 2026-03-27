use std::time::Instant;

use crate::data_channel::{SharedByteRingMapped, SharedMemoryEndpoint};

#[derive(Default)]
pub(crate) struct DataIngressThreadPlatformState;

impl DataIngressThreadPlatformState {
    pub(crate) fn on_thread_start(&mut self) {}
}

pub(crate) struct ReaderPlatformState;

impl ReaderPlatformState {
    pub(crate) fn open(endpoint: &SharedMemoryEndpoint) -> Result<Self, String> {
        let _ = (
            endpoint.host_to_sidecar_data_event.as_deref(),
            endpoint.host_to_sidecar_space_event.as_deref(),
        );
        Ok(Self)
    }

    pub(crate) fn sync_after_ring_change(&self, ring: &SharedByteRingMapped) -> Result<(), String> {
        let _ = (ring.occupied_len(), ring.free_len());
        Ok(())
    }

    pub(crate) fn wait_for_data_until(&self, _deadline: Instant) -> Result<Option<bool>, String> {
        Ok(None)
    }
}
