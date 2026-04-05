use std::time::Instant;

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::Duration;

use crate::data_channel::{SharedByteRingMapped, SharedMemoryEndpoint};

#[derive(Default)]
pub(crate) struct DataIngressThreadPlatformState;

impl DataIngressThreadPlatformState {
    pub(crate) fn on_thread_start(&mut self) {}
}

#[cfg(unix)]
struct NamedSemaphore {
    handle: *mut libc::sem_t,
}

#[cfg(unix)]
unsafe impl Send for NamedSemaphore {}

#[cfg(unix)]
impl Drop for NamedSemaphore {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::sem_close(self.handle);
        }
    }
}

#[cfg(unix)]
struct SharedMemorySignals {
    data_available: NamedSemaphore,
    space_available: NamedSemaphore,
    data_is_signaled: AtomicBool,
    space_is_signaled: AtomicBool,
}

#[cfg(unix)]
impl SharedMemorySignals {
    fn open(endpoint: &SharedMemoryEndpoint) -> Result<Option<Self>, String> {
        let Some(data_event) = endpoint.host_to_sidecar_data_event.as_deref() else {
            return Ok(None);
        };
        let Some(space_event) = endpoint.host_to_sidecar_space_event.as_deref() else {
            return Ok(None);
        };
        Ok(Some(Self {
            data_available: open_named_semaphore(data_event)?,
            space_available: open_named_semaphore(space_event)?,
            data_is_signaled: AtomicBool::new(false),
            space_is_signaled: AtomicBool::new(false),
        }))
    }

    fn sync_after_ring_change(&self, ring: &SharedByteRingMapped) -> Result<(), String> {
        sync_named_semaphore_state(
            &self.data_available,
            &self.data_is_signaled,
            ring.occupied_len() > 0,
        )?;
        sync_named_semaphore_state(
            &self.space_available,
            &self.space_is_signaled,
            ring.free_len() > 0,
        )?;
        Ok(())
    }

    fn wait_for_data_until(&self, deadline: Instant) -> Result<bool, String> {
        wait_for_named_semaphore_until(&self.data_available, deadline)
    }
}

pub(crate) struct ReaderPlatformState {
    #[cfg(unix)]
    events: Option<SharedMemorySignals>,
}

impl ReaderPlatformState {
    pub(crate) fn open(endpoint: &SharedMemoryEndpoint) -> Result<Self, String> {
        #[cfg(unix)]
        {
            Ok(Self {
                events: SharedMemorySignals::open(endpoint)?,
            })
        }

        #[cfg(not(unix))]
        {
            let _ = (
                endpoint.host_to_sidecar_data_event.as_deref(),
                endpoint.host_to_sidecar_space_event.as_deref(),
            );
            Ok(Self {})
        }
    }

    pub(crate) fn sync_after_ring_change(&self, ring: &SharedByteRingMapped) -> Result<(), String> {
        #[cfg(unix)]
        if let Some(events) = self.events.as_ref() {
            events.sync_after_ring_change(ring)?;
        }

        #[cfg(not(unix))]
        let _ = (ring.occupied_len(), ring.free_len());

        Ok(())
    }

    pub(crate) fn wait_for_data_until(&self, deadline: Instant) -> Result<Option<bool>, String> {
        #[cfg(unix)]
        {
            self
                .events
                .as_ref()
                .map(|events| events.wait_for_data_until(deadline))
                .transpose()
        }

        #[cfg(not(unix))]
        {
            let _ = deadline;
            Ok(None)
        }
    }
}

#[cfg(unix)]
fn semaphore_name(name: &str) -> Result<CString, String> {
    let mut normalized = name.trim().to_string();
    if normalized.is_empty() {
        return Err("shared-memory semaphore name is empty".to_string());
    }
    if !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    CString::new(normalized).map_err(|_| "shared-memory semaphore name contains NUL".to_string())
}

#[cfg(unix)]
fn open_named_semaphore(name: &str) -> Result<NamedSemaphore, String> {
    let name = semaphore_name(name)?;
    let handle = unsafe { libc::sem_open(name.as_ptr(), 0) };
    if handle == libc::SEM_FAILED {
        return Err(format!(
            "open shared-memory semaphore `{}`: {}",
            name.to_string_lossy(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(NamedSemaphore { handle })
}

#[cfg(unix)]
fn sync_named_semaphore_state(
    semaphore: &NamedSemaphore,
    is_signaled: &AtomicBool,
    available: bool,
) -> Result<(), String> {
    let was_signaled = is_signaled.swap(available, Ordering::AcqRel);
    if available {
        if !was_signaled {
            post_named_semaphore(semaphore)?;
        }
    } else if was_signaled {
        drain_named_semaphore(semaphore)?;
    }
    Ok(())
}

#[cfg(unix)]
fn post_named_semaphore(semaphore: &NamedSemaphore) -> Result<(), String> {
    if unsafe { libc::sem_post(semaphore.handle) } == 0 {
        return Ok(());
    }
    Err(format!(
        "post shared-memory semaphore: {}",
        std::io::Error::last_os_error()
    ))
}

#[cfg(unix)]
fn drain_named_semaphore(semaphore: &NamedSemaphore) -> Result<(), String> {
    loop {
        let result = unsafe { libc::sem_trywait(semaphore.handle) };
        if result == 0 {
            continue;
        }

        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(code) if code == libc::EAGAIN => return Ok(()),
            Some(code) if code == libc::EINTR => continue,
            _ => return Err(format!("drain shared-memory semaphore: {error}")),
        }
    }
}

#[cfg(unix)]
fn wait_for_named_semaphore_until(
    semaphore: &NamedSemaphore,
    deadline: Instant,
) -> Result<bool, String> {
    let now = Instant::now();
    if now >= deadline {
        return Ok(false);
    }
    let timeout = deadline.saturating_duration_since(now);
    wait_for_named_semaphore_timeout(semaphore, timeout)
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn wait_for_named_semaphore_timeout(
    semaphore: &NamedSemaphore,
    timeout: Duration,
) -> Result<bool, String> {
    let mut deadline = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut deadline) } != 0 {
        return Err(format!(
            "read realtime clock for shared-memory semaphore: {}",
            std::io::Error::last_os_error()
        ));
    }

    deadline.tv_sec += timeout.as_secs() as libc::time_t;
    deadline.tv_nsec += timeout.subsec_nanos() as libc::c_long;
    if deadline.tv_nsec >= 1_000_000_000 {
        deadline.tv_sec += 1;
        deadline.tv_nsec -= 1_000_000_000;
    }

    loop {
        let result = unsafe { libc::sem_timedwait(semaphore.handle, &deadline) };
        if result == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(code) if code == libc::EINTR => continue,
            Some(code) if code == libc::ETIMEDOUT => return Ok(false),
            _ => return Err(format!("wait on shared-memory semaphore: {error}")),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn wait_for_named_semaphore_timeout(
    semaphore: &NamedSemaphore,
    timeout: Duration,
) -> Result<bool, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let result = unsafe { libc::sem_trywait(semaphore.handle) };
        if result == 0 {
            return Ok(true);
        }

        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(code) if code == libc::EINTR => continue,
            Some(code) if code == libc::EAGAIN => {
                if Instant::now() >= deadline {
                    return Ok(false);
                }
                thread::sleep(Duration::from_millis(1));
            },
            _ => return Err(format!("wait on shared-memory semaphore: {error}")),
        }
    }
}
