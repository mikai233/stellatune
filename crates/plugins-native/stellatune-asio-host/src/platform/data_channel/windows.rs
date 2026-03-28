use std::time::Instant;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{
    AVRT_PRIORITY_HIGH, AvSetMmThreadCharacteristicsW, AvSetMmThreadPriority, EVENT_MODIFY_STATE,
    OpenEventW, ResetEvent, SYNCHRONIZATION_SYNCHRONIZE, SetEvent, WaitForSingleObject,
};
use windows::core::HSTRING;

use crate::data_channel::{SharedByteRingMapped, SharedMemoryEndpoint};

struct NamedEventHandle(HANDLE);

unsafe impl Send for NamedEventHandle {}

impl Drop for NamedEventHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct SharedMemoryEvents {
    data_available: NamedEventHandle,
    space_available: NamedEventHandle,
}

impl SharedMemoryEvents {
    fn open(data_event: Option<&str>, space_event: Option<&str>) -> Result<Option<Self>, String> {
        let Some(data_event) = data_event else {
            return Ok(None);
        };
        let Some(space_event) = space_event else {
            return Ok(None);
        };
        let access = EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE;
        let data_available = unsafe {
            OpenEventW(access, false, &HSTRING::from(data_event))
                .map_err(|error| format!("open shared-memory data event: {error}"))?
        };
        let space_available = unsafe {
            OpenEventW(access, false, &HSTRING::from(space_event))
                .map_err(|error| format!("open shared-memory space event: {error}"))?
        };
        Ok(Some(Self {
            data_available: NamedEventHandle(data_available),
            space_available: NamedEventHandle(space_available),
        }))
    }

    fn sync_after_ring_change(
        &self,
        occupied_bytes: usize,
        free_bytes: usize,
    ) -> Result<(), String> {
        unsafe {
            if occupied_bytes > 0 {
                SetEvent(self.data_available.0)
                    .map_err(|error| format!("set shared-memory data event: {error}"))?;
            } else {
                ResetEvent(self.data_available.0)
                    .map_err(|error| format!("reset shared-memory data event: {error}"))?;
            }

            if free_bytes > 0 {
                SetEvent(self.space_available.0)
                    .map_err(|error| format!("set shared-memory space event: {error}"))?;
            } else {
                ResetEvent(self.space_available.0)
                    .map_err(|error| format!("reset shared-memory space event: {error}"))?;
            }
        }
        Ok(())
    }

    fn wait_for_data_until(&self, deadline: Instant) -> Result<bool, String> {
        let wait_ms = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(u32::MAX as u128) as u32;
        let result = unsafe { WaitForSingleObject(self.data_available.0, wait_ms) };
        if result == WAIT_OBJECT_0 {
            Ok(true)
        } else if result == WAIT_TIMEOUT {
            Ok(false)
        } else {
            Err(format!("unexpected wait result: {result:?}"))
        }
    }
}

struct MmcssGuard(#[allow(dead_code)] HANDLE);

unsafe impl Send for MmcssGuard {}

#[derive(Default)]
pub(crate) struct DataIngressThreadPlatformState {
    attempted: bool,
    guard: Option<MmcssGuard>,
}

impl DataIngressThreadPlatformState {
    pub(crate) fn on_thread_start(&mut self) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        self.guard = enable_mmcss_pro_audio();
        if self.guard.is_some() {
            tracing::info!("asio host data ingress mmcss: Pro Audio enabled");
        } else {
            tracing::warn!("asio host data ingress mmcss: failed to enable Pro Audio");
        }
    }
}

pub(crate) struct ReaderPlatformState {
    events: Option<SharedMemoryEvents>,
}

impl ReaderPlatformState {
    pub(crate) fn open(endpoint: &SharedMemoryEndpoint) -> Result<Self, String> {
        Ok(Self {
            events: SharedMemoryEvents::open(
                endpoint.host_to_sidecar_data_event.as_deref(),
                endpoint.host_to_sidecar_space_event.as_deref(),
            )?,
        })
    }

    pub(crate) fn sync_after_ring_change(&self, ring: &SharedByteRingMapped) -> Result<(), String> {
        if let Some(events) = self.events.as_ref() {
            events.sync_after_ring_change(ring.occupied_len(), ring.free_len())?;
        }
        Ok(())
    }

    pub(crate) fn wait_for_data_until(&self, deadline: Instant) -> Result<Option<bool>, String> {
        self.events
            .as_ref()
            .map(|events| events.wait_for_data_until(deadline))
            .transpose()
    }
}

fn enable_mmcss_pro_audio() -> Option<MmcssGuard> {
    let mut task_index = 0u32;
    let task = HSTRING::from("Pro Audio");
    let handle = unsafe { AvSetMmThreadCharacteristicsW(&task, &mut task_index) }.ok()?;
    let _ = unsafe { AvSetMmThreadPriority(handle, AVRT_PRIORITY_HIGH) };
    Some(MmcssGuard(handle))
}
