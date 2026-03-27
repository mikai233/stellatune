use std::collections::BTreeMap;
#[cfg(unix)]
use std::ffi::CString;
use std::fs::OpenOptions;
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use memmap2::{MmapMut, MmapOptions};
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    CreateEventW, EVENT_MODIFY_STATE, OpenEventW, ResetEvent, SYNCHRONIZATION_SYNCHRONIZE,
    SetEvent, WaitForSingleObject,
};
#[cfg(windows)]
use windows::core::HSTRING;

use crate::error::{Error, Result};

use super::types::{SidecarTransportKind, SidecarTransportOption};

pub(super) const SHM_MIN_CAPACITY: usize = 4 * 1024;
const SHM_MAGIC: u32 = 0x53544D52;
const SHM_VERSION: u32 = 1;
const SHM_MAX_CAPACITY: usize = 64 * 1024 * 1024;
const SHM_DEFAULT_CAPACITY: usize = 1024 * 1024;
const SHM_POLL_INTERVAL: Duration = Duration::from_millis(1);

#[repr(C)]
struct SharedByteRingHeader {
    magic: u32,
    version: u32,
    capacity_bytes: u32,
    _reserved: u32,
    write_pos: AtomicU64,
    read_pos: AtomicU64,
}

pub(super) struct SharedByteRingMapped {
    map: MmapMut,
    capacity_bytes: usize,
}

impl SharedByteRingMapped {
    fn header_size() -> usize {
        mem::size_of::<SharedByteRingHeader>()
    }

    pub(super) fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|error| Error::operation("sidecar.shm.open", error.to_string()))?;
        let map = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(|error| Error::operation("sidecar.shm.map", error.to_string()))?
        };
        if map.len() < Self::header_size() {
            return Err(Error::operation(
                "sidecar.shm.open",
                format!("ring file too small: {}", path.display()),
            ));
        }

        let header = unsafe { &*(map.as_ptr() as *const SharedByteRingHeader) };
        if header.magic != SHM_MAGIC || header.version != SHM_VERSION {
            return Err(Error::operation(
                "sidecar.shm.open",
                format!("invalid ring header: {}", path.display()),
            ));
        }
        let capacity_bytes = header.capacity_bytes as usize;
        if !(SHM_MIN_CAPACITY..=SHM_MAX_CAPACITY).contains(&capacity_bytes) {
            return Err(Error::operation(
                "sidecar.shm.open",
                format!(
                    "invalid ring capacity {} for {}",
                    capacity_bytes,
                    path.display()
                ),
            ));
        }
        let expected = Self::header_size()
            .checked_add(capacity_bytes)
            .ok_or_else(|| Error::operation("sidecar.shm.open", "capacity overflow"))?;
        if expected != map.len() {
            return Err(Error::operation(
                "sidecar.shm.open",
                format!(
                    "ring size mismatch for {}: expect {}, got {}",
                    path.display(),
                    expected,
                    map.len()
                ),
            ));
        }
        Ok(Self {
            map,
            capacity_bytes,
        })
    }

    pub(super) fn create(path: &Path, capacity_bytes: usize) -> Result<Self> {
        if !(SHM_MIN_CAPACITY..=SHM_MAX_CAPACITY).contains(&capacity_bytes) {
            return Err(Error::operation(
                "sidecar.shm.create",
                format!("invalid capacity {}", capacity_bytes),
            ));
        }
        let total = Self::header_size()
            .checked_add(capacity_bytes)
            .ok_or_else(|| Error::operation("sidecar.shm.create", "capacity overflow"))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| Error::operation("sidecar.shm.create", error.to_string()))?;
        file.set_len(total as u64)
            .map_err(|error| Error::operation("sidecar.shm.create", error.to_string()))?;
        let mut map = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(|error| Error::operation("sidecar.shm.create", error.to_string()))?
        };
        unsafe {
            let header = map.as_mut_ptr() as *mut SharedByteRingHeader;
            ptr::write(
                header,
                SharedByteRingHeader {
                    magic: SHM_MAGIC,
                    version: SHM_VERSION,
                    capacity_bytes: capacity_bytes as u32,
                    _reserved: 0,
                    write_pos: AtomicU64::new(0),
                    read_pos: AtomicU64::new(0),
                },
            );
            ptr::write_bytes(map.as_mut_ptr().add(Self::header_size()), 0, capacity_bytes);
        }
        Ok(Self {
            map,
            capacity_bytes,
        })
    }

    fn header(&self) -> &SharedByteRingHeader {
        unsafe { &*(self.map.as_ptr() as *const SharedByteRingHeader) }
    }

    pub(super) fn write_bytes(&mut self, input: &[u8]) -> usize {
        if input.is_empty() {
            return 0;
        }
        let header = self.header();
        let read_pos = header.read_pos.load(Ordering::Acquire);
        let write_pos = header.write_pos.load(Ordering::Relaxed);
        let used = write_pos
            .saturating_sub(read_pos)
            .min(self.capacity_bytes as u64) as usize;
        let available = self.capacity_bytes.saturating_sub(used);
        let count = available.min(input.len());
        if count == 0 {
            return 0;
        }

        let start = (write_pos as usize) % self.capacity_bytes;
        let first = count.min(self.capacity_bytes - start);
        unsafe {
            let base = self.map.as_ptr() as *mut u8;
            let data = base.add(Self::header_size());
            ptr::copy_nonoverlapping(input.as_ptr(), data.add(start), first);
            if first < count {
                ptr::copy_nonoverlapping(input.as_ptr().add(first), data, count - first);
            }
        }
        header
            .write_pos
            .store(write_pos + count as u64, Ordering::Release);
        count
    }

    pub(super) fn read_bytes(&mut self, out: &mut [u8]) -> usize {
        if out.is_empty() {
            return 0;
        }
        let header = self.header();
        let write_pos = header.write_pos.load(Ordering::Acquire);
        let read_pos = header.read_pos.load(Ordering::Relaxed);
        let available = write_pos
            .saturating_sub(read_pos)
            .min(self.capacity_bytes as u64) as usize;
        let count = available.min(out.len());
        if count == 0 {
            return 0;
        }

        let start = (read_pos as usize) % self.capacity_bytes;
        let first = count.min(self.capacity_bytes - start);
        unsafe {
            let base = self.map.as_ptr();
            let data = base.add(Self::header_size());
            ptr::copy_nonoverlapping(data.add(start), out.as_mut_ptr(), first);
            if first < count {
                ptr::copy_nonoverlapping(data, out.as_mut_ptr().add(first), count - first);
            }
        }
        header
            .read_pos
            .store(read_pos + count as u64, Ordering::Release);
        count
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub(super) fn occupied_len(&self) -> usize {
        let header = self.header();
        let write_pos = header.write_pos.load(Ordering::Acquire);
        let read_pos = header.read_pos.load(Ordering::Relaxed);
        write_pos
            .saturating_sub(read_pos)
            .min(self.capacity_bytes as u64) as usize
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub(super) fn free_len(&self) -> usize {
        self.capacity_bytes.saturating_sub(self.occupied_len())
    }
}

pub(super) struct SharedMemoryChannelIo {
    tx: SharedByteRingMapped,
    rx: SharedByteRingMapped,
    #[cfg(any(unix, windows))]
    tx_events: Option<SharedMemoryEvents>,
    #[cfg(any(unix, windows))]
    rx_events: Option<SharedMemoryEvents>,
}

impl SharedMemoryChannelIo {
    pub(super) fn open(endpoint: &str) -> Result<Self> {
        let config = parse_shared_memory_endpoint(endpoint)?;
        let tx = SharedByteRingMapped::open(Path::new(config.tx_path.as_str()))?;
        let rx = SharedByteRingMapped::open(Path::new(config.rx_path.as_str()))?;
        Ok(Self {
            tx,
            rx,
            #[cfg(any(unix, windows))]
            tx_events: SharedMemoryEvents::open(
                config.tx_data_event.as_deref(),
                config.tx_space_event.as_deref(),
            )?,
            #[cfg(any(unix, windows))]
            rx_events: SharedMemoryEvents::open(
                config.rx_data_event.as_deref(),
                config.rx_space_event.as_deref(),
            )?,
        })
    }

    pub(super) fn write(&mut self, data: &[u8]) -> Result<u32> {
        if data.is_empty() {
            return Ok(0);
        }

        let wrote = self.tx.write_bytes(data);
        #[cfg(any(unix, windows))]
        if let Some(events) = self.tx_events.as_ref() {
            events.sync_after_ring_change(self.tx.occupied_len(), self.tx.free_len())?;
            if wrote == 0 {
                let _ = events.wait_for_space(Some(1))?;
            }
        }
        Ok(wrote as u32)
    }

    pub(super) fn read(&mut self, max_bytes: u32, timeout_ms: Option<u32>) -> Result<Vec<u8>> {
        let max_bytes = max_bytes as usize;
        if max_bytes == 0 {
            return Ok(Vec::new());
        }

        let deadline = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms as u64));
        let mut out = vec![0_u8; max_bytes];

        loop {
            let read = self.rx.read_bytes(&mut out);
            if read > 0 {
                #[cfg(any(unix, windows))]
                if let Some(events) = self.rx_events.as_ref() {
                    events.sync_after_ring_change(self.rx.occupied_len(), self.rx.free_len())?;
                }
                out.truncate(read);
                return Ok(out);
            }

            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    return Ok(Vec::new());
                }
            } else {
                return Ok(Vec::new());
            }

            #[cfg(any(unix, windows))]
            if let Some(events) = self.rx_events.as_ref() {
                let wait_ms = deadline.map(|deadline| {
                    deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis()
                        .min(u32::MAX as u128) as u32
                });
                if events.wait_for_data(wait_ms)? {
                    continue;
                }
                return Ok(Vec::new());
            }

            thread::sleep(SHM_POLL_INTERVAL);
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(super) struct SharedMemoryEndpoint {
    pub(super) tx_path: String,
    pub(super) rx_path: String,
    tx_data_event: Option<String>,
    tx_space_event: Option<String>,
    rx_data_event: Option<String>,
    rx_space_event: Option<String>,
}

pub(super) fn parse_shared_memory_endpoint(endpoint: &str) -> Result<SharedMemoryEndpoint> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Err(Error::invalid_input("shared-memory endpoint is empty"));
    }

    if !endpoint.contains('=') {
        return Ok(SharedMemoryEndpoint {
            tx_path: endpoint.to_string(),
            rx_path: endpoint.to_string(),
            tx_data_event: None,
            tx_space_event: None,
            rx_data_event: None,
            rx_space_event: None,
        });
    }

    let mut tx_path = None::<String>;
    let mut rx_path = None::<String>;
    let mut tx_data_event = None::<String>;
    let mut tx_space_event = None::<String>;
    let mut rx_data_event = None::<String>;
    let mut rx_space_event = None::<String>;
    for part in endpoint.split([';', ',']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = part.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value.trim();
        if value.is_empty() {
            continue;
        }
        if key == "tx" || key == "write" || key == "host_to_sidecar" {
            tx_path = Some(value.to_string());
        } else if key == "rx" || key == "read" || key == "sidecar_to_host" {
            rx_path = Some(value.to_string());
        } else if key == "tx_data_event" || key == "host_to_sidecar_data_event" {
            tx_data_event = Some(value.to_string());
        } else if key == "tx_space_event" || key == "host_to_sidecar_space_event" {
            tx_space_event = Some(value.to_string());
        } else if key == "rx_data_event" || key == "sidecar_to_host_data_event" {
            rx_data_event = Some(value.to_string());
        } else if key == "rx_space_event" || key == "sidecar_to_host_space_event" {
            rx_space_event = Some(value.to_string());
        } else if key == "path" || key == "ring" {
            let value = value.to_string();
            if tx_path.is_none() {
                tx_path = Some(value.clone());
            }
            if rx_path.is_none() {
                rx_path = Some(value);
            }
        }
    }

    let tx_path = tx_path
        .or_else(|| rx_path.clone())
        .ok_or_else(|| Error::invalid_input("shared-memory endpoint missing `tx` or `path`"))?;
    let rx_path = rx_path
        .or_else(|| Some(tx_path.clone()))
        .ok_or_else(|| Error::invalid_input("shared-memory endpoint missing `rx` or `path`"))?;
    Ok(SharedMemoryEndpoint {
        tx_path,
        rx_path,
        tx_data_event,
        tx_space_event,
        rx_data_event,
        rx_space_event,
    })
}

pub(super) fn prepare_shared_memory_env(
    preferred: &[SidecarTransportOption],
    full_key: &'static str,
    short_key: &'static str,
    env: &mut Vec<(String, String)>,
    env_map: &mut BTreeMap<String, String>,
    created_ring_paths: &mut Vec<PathBuf>,
    #[cfg(unix)] created_semaphore_names: &mut Vec<String>,
    #[cfg(windows)] created_event_handles: &mut Vec<NamedEventHandle>,
) -> Result<()> {
    if !preferred
        .iter()
        .any(|option| option.kind == SidecarTransportKind::SharedMemoryRing)
    {
        return Ok(());
    }

    if let Some(value) = first_non_empty_env(env_map, &[full_key, short_key]) {
        ensure_env_entry(env, full_key, &value);
        ensure_env_entry(env, short_key, &value);
        env_map.insert(full_key.to_string(), value.clone());
        env_map.insert(short_key.to_string(), value);
        return Ok(());
    }

    let capacity = preferred
        .iter()
        .filter(|option| option.kind == SidecarTransportKind::SharedMemoryRing)
        .filter_map(|option| option.max_frame_bytes)
        .map(|bytes| bytes as usize)
        .max()
        .unwrap_or(SHM_DEFAULT_CAPACITY)
        .clamp(SHM_MIN_CAPACITY, SHM_MAX_CAPACITY);
    let endpoint = create_shared_memory_endpoint(
        capacity,
        created_ring_paths,
        #[cfg(unix)]
        created_semaphore_names,
        #[cfg(windows)]
        created_event_handles,
    )?;

    ensure_env_entry(env, full_key, endpoint.as_str());
    ensure_env_entry(env, short_key, endpoint.as_str());
    env_map.insert(full_key.to_string(), endpoint.clone());
    env_map.insert(short_key.to_string(), endpoint);
    Ok(())
}

fn create_shared_memory_endpoint(
    capacity_bytes: usize,
    created_ring_paths: &mut Vec<PathBuf>,
    #[cfg(unix)] created_semaphore_names: &mut Vec<String>,
    #[cfg(windows)] created_event_handles: &mut Vec<NamedEventHandle>,
) -> Result<String> {
    let base_dir = std::env::temp_dir().join("stellatune-sidecar-shm");
    std::fs::create_dir_all(base_dir.as_path())
        .map_err(|error| Error::operation("sidecar.shm.create-dir", error.to_string()))?;

    let tx_path = unique_ring_path(base_dir.as_path(), "tx");
    let rx_path = unique_ring_path(base_dir.as_path(), "rx");
    let _ = SharedByteRingMapped::create(tx_path.as_path(), capacity_bytes)?;
    let _ = SharedByteRingMapped::create(rx_path.as_path(), capacity_bytes)?;
    created_ring_paths.push(tx_path.clone());
    created_ring_paths.push(rx_path.clone());

    #[cfg(windows)]
    {
        let tx_data_event = unique_event_name("tx-data");
        let tx_space_event = unique_event_name("tx-space");
        let rx_data_event = unique_event_name("rx-data");
        let rx_space_event = unique_event_name("rx-space");
        created_event_handles.push(create_named_event(tx_data_event.as_str(), false)?);
        created_event_handles.push(create_named_event(tx_space_event.as_str(), true)?);
        created_event_handles.push(create_named_event(rx_data_event.as_str(), false)?);
        created_event_handles.push(create_named_event(rx_space_event.as_str(), true)?);

        Ok(format!(
            "tx={};rx={};tx_data_event={};tx_space_event={};rx_data_event={};rx_space_event={}",
            tx_path.to_string_lossy(),
            rx_path.to_string_lossy(),
            tx_data_event,
            tx_space_event,
            rx_data_event,
            rx_space_event
        ))
    }

    #[cfg(unix)]
    {
        let tx_data_event = unique_unix_semaphore_name("tx-data");
        let tx_space_event = unique_unix_semaphore_name("tx-space");
        let rx_data_event = unique_unix_semaphore_name("rx-data");
        let rx_space_event = unique_unix_semaphore_name("rx-space");
        create_named_semaphore(tx_data_event.as_str())?;
        create_named_semaphore(tx_space_event.as_str())?;
        create_named_semaphore(rx_data_event.as_str())?;
        create_named_semaphore(rx_space_event.as_str())?;
        created_semaphore_names.push(tx_data_event.clone());
        created_semaphore_names.push(tx_space_event.clone());
        created_semaphore_names.push(rx_data_event.clone());
        created_semaphore_names.push(rx_space_event.clone());

        Ok(format!(
            "tx={};rx={};tx_data_event={};tx_space_event={};rx_data_event={};rx_space_event={}",
            tx_path.to_string_lossy(),
            rx_path.to_string_lossy(),
            tx_data_event,
            tx_space_event,
            rx_data_event,
            rx_space_event
        ))
    }

    #[cfg(not(any(unix, windows)))]
    {
        Ok(format!(
            "tx={};rx={}",
            tx_path.to_string_lossy(),
            rx_path.to_string_lossy()
        ))
    }
}

#[cfg(windows)]
pub(super) struct NamedEventHandle {
    handle: HANDLE,
}

#[cfg(windows)]
unsafe impl Send for NamedEventHandle {}

#[cfg(windows)]
impl Drop for NamedEventHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
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

#[cfg(windows)]
fn unique_event_name(direction: &str) -> String {
    let pid = std::process::id();
    let epoch_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("Local\\stellatune-sidecar-{pid}-{epoch_ns}-{direction}")
}

#[cfg(windows)]
fn create_named_event(name: &str, initial_state: bool) -> Result<NamedEventHandle> {
    let handle = unsafe {
        CreateEventW(None, true, initial_state, &HSTRING::from(name))
            .map_err(|error| Error::operation("sidecar.shm.create-event", error.to_string()))?
    };
    Ok(NamedEventHandle { handle })
}

#[cfg(unix)]
fn unique_unix_semaphore_name(direction: &str) -> String {
    let pid = std::process::id();
    let epoch_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("/stellatune-sidecar-{pid}-{epoch_ns}-{direction}")
}

#[cfg(unix)]
fn create_named_semaphore(name: &str) -> Result<()> {
    let name = semaphore_name(name)?;
    let handle = unsafe { libc::sem_open(name.as_ptr(), libc::O_CREAT | libc::O_EXCL, 0o600, 0) };
    if handle == libc::SEM_FAILED {
        let error = std::io::Error::last_os_error();
        return Err(Error::operation(
            "sidecar.shm.create-semaphore",
            error.to_string(),
        ));
    }

    if unsafe { libc::sem_close(handle) } != 0 {
        let error = std::io::Error::last_os_error();
        return Err(Error::operation(
            "sidecar.shm.create-semaphore",
            error.to_string(),
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn unlink_named_semaphore(name: &str) -> Result<()> {
    let name = semaphore_name(name)?;
    if unsafe { libc::sem_unlink(name.as_ptr()) } == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if matches!(error.raw_os_error(), Some(code) if code == libc::ENOENT) {
        return Ok(());
    }
    Err(Error::operation(
        "sidecar.shm.unlink-semaphore",
        error.to_string(),
    ))
}

#[cfg(unix)]
fn open_named_semaphore(name: &str) -> Result<NamedSemaphore> {
    let name = semaphore_name(name)?;
    let handle = unsafe { libc::sem_open(name.as_ptr(), 0) };
    if handle == libc::SEM_FAILED {
        let error = std::io::Error::last_os_error();
        return Err(Error::operation(
            "sidecar.shm.open-semaphore",
            error.to_string(),
        ));
    }
    Ok(NamedSemaphore { handle })
}

#[cfg(unix)]
fn semaphore_name(name: &str) -> Result<CString> {
    let mut normalized = name.trim().to_string();
    if normalized.is_empty() {
        return Err(Error::invalid_input(
            "shared-memory semaphore name is empty",
        ));
    }
    if !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    CString::new(normalized)
        .map_err(|_| Error::invalid_input("shared-memory semaphore name contains NUL"))
}

#[cfg(windows)]
struct SharedMemoryEvents {
    data_available: NamedEventHandle,
    space_available: NamedEventHandle,
}

#[cfg(unix)]
struct SharedMemoryEvents {
    data_available: NamedSemaphore,
    space_available: NamedSemaphore,
    data_is_signaled: std::sync::atomic::AtomicBool,
    space_is_signaled: std::sync::atomic::AtomicBool,
}

#[cfg(unix)]
impl SharedMemoryEvents {
    fn open(data_event: Option<&str>, space_event: Option<&str>) -> Result<Option<Self>> {
        let Some(data_event) = data_event else {
            return Ok(None);
        };
        let Some(space_event) = space_event else {
            return Ok(None);
        };
        Ok(Some(Self {
            data_available: open_named_semaphore(data_event)?,
            space_available: open_named_semaphore(space_event)?,
            data_is_signaled: std::sync::atomic::AtomicBool::new(false),
            space_is_signaled: std::sync::atomic::AtomicBool::new(false),
        }))
    }

    fn sync_after_ring_change(&self, occupied_bytes: usize, free_bytes: usize) -> Result<()> {
        sync_named_semaphore_state(
            &self.data_available,
            &self.data_is_signaled,
            occupied_bytes > 0,
        )?;
        sync_named_semaphore_state(
            &self.space_available,
            &self.space_is_signaled,
            free_bytes > 0,
        )?;
        Ok(())
    }

    fn wait_for_data(&self, timeout_ms: Option<u32>) -> Result<bool> {
        wait_for_named_semaphore(&self.data_available, timeout_ms)
    }

    fn wait_for_space(&self, timeout_ms: Option<u32>) -> Result<bool> {
        wait_for_named_semaphore(&self.space_available, timeout_ms)
    }
}

#[cfg(windows)]
impl SharedMemoryEvents {
    fn open(data_event: Option<&str>, space_event: Option<&str>) -> Result<Option<Self>> {
        let Some(data_event) = data_event else {
            return Ok(None);
        };
        let Some(space_event) = space_event else {
            return Ok(None);
        };
        let access = EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE;
        let data_available = unsafe {
            OpenEventW(access, false, &HSTRING::from(data_event))
                .map_err(|error| Error::operation("sidecar.shm.open-event", error.to_string()))?
        };
        let space_available = unsafe {
            OpenEventW(access, false, &HSTRING::from(space_event))
                .map_err(|error| Error::operation("sidecar.shm.open-event", error.to_string()))?
        };
        Ok(Some(Self {
            data_available: NamedEventHandle {
                handle: data_available,
            },
            space_available: NamedEventHandle {
                handle: space_available,
            },
        }))
    }

    fn sync_after_ring_change(&self, occupied_bytes: usize, free_bytes: usize) -> Result<()> {
        unsafe {
            if occupied_bytes > 0 {
                SetEvent(self.data_available.handle).map_err(|error| {
                    Error::operation("sidecar.shm.set-event", error.to_string())
                })?;
            } else {
                ResetEvent(self.data_available.handle).map_err(|error| {
                    Error::operation("sidecar.shm.reset-event", error.to_string())
                })?;
            }

            if free_bytes > 0 {
                SetEvent(self.space_available.handle).map_err(|error| {
                    Error::operation("sidecar.shm.set-event", error.to_string())
                })?;
            } else {
                ResetEvent(self.space_available.handle).map_err(|error| {
                    Error::operation("sidecar.shm.reset-event", error.to_string())
                })?;
            }
        }
        Ok(())
    }

    fn wait_for_data(&self, timeout_ms: Option<u32>) -> Result<bool> {
        wait_for_named_event(self.data_available.handle, timeout_ms)
    }

    fn wait_for_space(&self, timeout_ms: Option<u32>) -> Result<bool> {
        wait_for_named_event(self.space_available.handle, timeout_ms)
    }
}

#[cfg(windows)]
fn wait_for_named_event(handle: HANDLE, timeout_ms: Option<u32>) -> Result<bool> {
    let timeout = timeout_ms.unwrap_or(u32::MAX);
    let result = unsafe { WaitForSingleObject(handle, timeout) };
    if result == windows::Win32::Foundation::WAIT_OBJECT_0 {
        Ok(true)
    } else if result == WAIT_TIMEOUT {
        Ok(false)
    } else {
        Err(Error::operation(
            "sidecar.shm.wait-event",
            format!("unexpected wait result: {result:?}"),
        ))
    }
}

#[cfg(unix)]
fn sync_named_semaphore_state(
    semaphore: &NamedSemaphore,
    is_signaled: &std::sync::atomic::AtomicBool,
    available: bool,
) -> Result<()> {
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
fn post_named_semaphore(semaphore: &NamedSemaphore) -> Result<()> {
    if unsafe { libc::sem_post(semaphore.handle) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    Err(Error::operation(
        "sidecar.shm.post-semaphore",
        error.to_string(),
    ))
}

#[cfg(unix)]
fn drain_named_semaphore(semaphore: &NamedSemaphore) -> Result<()> {
    loop {
        let result = unsafe { libc::sem_trywait(semaphore.handle) };
        if result == 0 {
            continue;
        }

        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(code) if code == libc::EAGAIN => return Ok(()),
            Some(code) if code == libc::EINTR => continue,
            _ => {
                return Err(Error::operation(
                    "sidecar.shm.drain-semaphore",
                    error.to_string(),
                ));
            },
        }
    }
}

#[cfg(unix)]
fn wait_for_named_semaphore(semaphore: &NamedSemaphore, timeout_ms: Option<u32>) -> Result<bool> {
    match timeout_ms {
        None => loop {
            let result = unsafe { libc::sem_wait(semaphore.handle) };
            if result == 0 {
                return Ok(true);
            }
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(code) if code == libc::EINTR) {
                continue;
            }
            return Err(Error::operation(
                "sidecar.shm.wait-semaphore",
                error.to_string(),
            ));
        },
        Some(timeout_ms) => wait_for_named_semaphore_timeout(semaphore, timeout_ms),
    }
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn wait_for_named_semaphore_timeout(semaphore: &NamedSemaphore, timeout_ms: u32) -> Result<bool> {
    let mut deadline = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut deadline) } != 0 {
        let error = std::io::Error::last_os_error();
        return Err(Error::operation(
            "sidecar.shm.wait-semaphore",
            error.to_string(),
        ));
    }

    deadline.tv_sec += (timeout_ms / 1_000) as libc::time_t;
    deadline.tv_nsec += ((timeout_ms % 1_000) as libc::c_long) * 1_000_000;
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
            _ => {
                return Err(Error::operation(
                    "sidecar.shm.wait-semaphore",
                    error.to_string(),
                ));
            },
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn wait_for_named_semaphore_timeout(semaphore: &NamedSemaphore, timeout_ms: u32) -> Result<bool> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
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
            _ => {
                return Err(Error::operation(
                    "sidecar.shm.wait-semaphore",
                    error.to_string(),
                ));
            },
        }
    }
}

fn unique_ring_path(base_dir: &Path, direction: &str) -> PathBuf {
    let pid = std::process::id();
    let epoch_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    for attempt in 0..1024_u32 {
        let path = base_dir.join(format!("ring-{pid}-{epoch_ns}-{direction}-{attempt}.shm"));
        if !path.exists() {
            return path;
        }
    }
    base_dir.join(format!("ring-{pid}-{epoch_ns}-{direction}.shm"))
}

fn first_non_empty_env(env_map: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| env_map.get(*key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn ensure_env_entry(env: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some(entry) = env
        .iter_mut()
        .find(|(existing_key, _)| existing_key.eq_ignore_ascii_case(key))
    {
        entry.0 = key.to_string();
        entry.1 = value.to_string();
        return;
    }
    env.push((key.to_string(), value.to_string()));
}
