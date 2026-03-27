use std::fs::OpenOptions;
use std::mem;
use std::path::Path;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, Builder, JoinHandle};
use std::time::{Duration, Instant};

use memmap2::{MmapMut, MmapOptions};

use crate::stream::StreamIngress;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    AVRT_PRIORITY_HIGH, AvSetMmThreadCharacteristicsW, AvSetMmThreadPriority, EVENT_MODIFY_STATE,
    OpenEventW, ResetEvent, SYNCHRONIZATION_SYNCHRONIZE, SetEvent, WaitForSingleObject,
};
#[cfg(windows)]
use windows::core::HSTRING;

const DATA_FRAME_MAX_BYTES: usize = 16 * 1024 * 1024;
const DATA_POLL_INTERVAL: Duration = Duration::from_millis(1);

const SHM_MAGIC: u32 = 0x53544D52; // "STMR"
const SHM_VERSION: u32 = 1;
const SHM_MIN_CAPACITY: usize = 4 * 1024;
const SHM_MAX_CAPACITY: usize = 64 * 1024 * 1024;

pub(crate) struct DataIngressPump {
    current_ingress: Arc<Mutex<Option<StreamIngress>>>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl DataIngressPump {
    pub(crate) fn from_env() -> Result<Option<Self>, String> {
        let Some(mut reader) = SharedMemoryDataReader::open_from_env()? else {
            return Ok(None);
        };

        let current_ingress = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let ingress_ref = Arc::clone(&current_ingress);
        let stop_ref = Arc::clone(&stop);
        let join = Builder::new()
            .name("stellatune-asio-data".to_string())
            .spawn(move || run_data_ingress(&mut reader, ingress_ref, stop_ref))
            .map_err(|error| format!("spawn asio data ingress thread: {error}"))?;

        Ok(Some(Self {
            current_ingress,
            stop,
            join: Some(join),
        }))
    }

    pub(crate) fn set_ingress(&self, ingress: Option<StreamIngress>) {
        if let Ok(mut slot) = self.current_ingress.lock() {
            *slot = ingress;
        }
    }
}

impl Drop for DataIngressPump {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_data_ingress(
    reader: &mut SharedMemoryDataReader,
    current_ingress: Arc<Mutex<Option<StreamIngress>>>,
    stop: Arc<AtomicBool>,
) {
    #[cfg(windows)]
    let mut mmcss_state = MmcssState::default();
    #[cfg(windows)]
    mmcss_state.ensure_pro_audio();

    let mut len_bytes = [0_u8; 4];
    let mut payload = Vec::<u8>::new();

    while !stop.load(Ordering::Acquire) {
        let got_len = match reader.read_exact(&mut len_bytes, &stop, Duration::from_millis(100)) {
            Ok(got_frame) => got_frame,
            Err(error) => {
                eprintln!("asio host data ingress stopped while reading frame length: {error}");
                break;
            },
        };
        if !got_len {
            continue;
        }

        let frame_len = u32::from_le_bytes(len_bytes) as usize;
        if frame_len > DATA_FRAME_MAX_BYTES {
            eprintln!("asio host data ingress rejected oversized frame: {frame_len} bytes");
            break;
        }

        payload.resize(frame_len, 0);
        if frame_len > 0 {
            match reader.read_exact(payload.as_mut_slice(), &stop, Duration::from_millis(250)) {
                Ok(true) => {},
                Ok(false) => continue,
                Err(error) => {
                    eprintln!("asio host data ingress stopped while reading payload: {error}");
                    break;
                },
            }
        }

        let mut payload_offset = 0usize;
        while payload_offset < payload.len() && !stop.load(Ordering::Acquire) {
            let ingress = current_ingress
                .lock()
                .ok()
                .and_then(|slot| slot.as_ref().cloned());
            let Some(ingress) = ingress else {
                thread::sleep(DATA_POLL_INTERVAL);
                continue;
            };

            let bytes_per_frame = ingress.bytes_per_frame();
            match ingress.write_interleaved_f32le(&payload[payload_offset..]) {
                Ok(frames_written) => {
                    let accepted_bytes = frames_written as usize * bytes_per_frame;
                    if accepted_bytes == 0 {
                        thread::sleep(DATA_POLL_INTERVAL);
                        continue;
                    }

                    payload_offset = payload_offset.saturating_add(accepted_bytes);
                },
                Err(error) => {
                    eprintln!("asio host data ingress write failed: {error}");
                    break;
                },
            }
        }
    }
}

struct SharedMemoryDataReader {
    ring: SharedByteRingMapped,
    #[cfg(windows)]
    events: Option<SharedMemoryEvents>,
}

impl SharedMemoryDataReader {
    fn open_from_env() -> Result<Option<Self>, String> {
        let Some(endpoint) = resolve_data_endpoint() else {
            return Ok(None);
        };

        let config = parse_shared_memory_endpoint(endpoint.as_str())?;
        let ring = SharedByteRingMapped::open(Path::new(config.host_to_sidecar_path.as_str()))?;
        Ok(Some(Self {
            ring,
            #[cfg(windows)]
            events: SharedMemoryEvents::open(
                config.host_to_sidecar_data_event.as_deref(),
                config.host_to_sidecar_space_event.as_deref(),
            )?,
        }))
    }

    fn read_exact(
        &mut self,
        out: &mut [u8],
        stop: &AtomicBool,
        timeout: Duration,
    ) -> Result<bool, String> {
        if out.is_empty() {
            return Ok(true);
        }

        let deadline = Instant::now() + timeout;
        let mut offset = 0usize;

        while offset < out.len() {
            if stop.load(Ordering::Acquire) {
                return Ok(false);
            }

            let read = self.ring.read_bytes(&mut out[offset..]);
            if read > 0 {
                offset += read;
                #[cfg(windows)]
                if let Some(events) = self.events.as_ref() {
                    events.sync_after_ring_change(self.ring.occupied_len(), self.ring.free_len())?;
                }
                continue;
            }

            if Instant::now() >= deadline {
                if offset == 0 {
                    return Ok(false);
                }
                return Err("timed out waiting for shared-memory data frame".to_string());
            }

            #[cfg(windows)]
            if let Some(events) = self.events.as_ref() {
                let wait_ms = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(u32::MAX as u128) as u32;
                if events.wait_for_data(Some(wait_ms))? {
                    continue;
                }
                return Ok(false);
            }

            thread::sleep(DATA_POLL_INTERVAL);
        }

        Ok(true)
    }
}

fn resolve_data_endpoint() -> Option<String> {
    let keys = [
        "STELLATUNE_SIDECAR_DATA_SAMPLES_SHARED_MEMORY_RING",
        "STELLATUNE_SIDECAR_DATA_SAMPLES_SHM",
        "STELLATUNE_SIDECAR_DATA_SHARED_MEMORY_RING",
        "STELLATUNE_SIDECAR_DATA_SHM",
    ];

    keys.into_iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

struct SharedMemoryEndpoint {
    host_to_sidecar_path: String,
    host_to_sidecar_data_event: Option<String>,
    host_to_sidecar_space_event: Option<String>,
}

fn parse_shared_memory_endpoint(endpoint: &str) -> Result<SharedMemoryEndpoint, String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Err("shared-memory endpoint is empty".to_string());
    }

    let mut host_to_sidecar_path = None::<String>;
    let mut host_to_sidecar_data_event = None::<String>;
    let mut host_to_sidecar_space_event = None::<String>;
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

        match key.as_str() {
            "tx" => {
                host_to_sidecar_path = Some(value.to_string());
            }
            "tx_data_event" => {
                host_to_sidecar_data_event = Some(value.to_string());
            }
            "tx_space_event" => {
                host_to_sidecar_space_event = Some(value.to_string());
            }
            _ => {}
        }
    }

    let host_to_sidecar_path =
        host_to_sidecar_path.ok_or_else(|| "shared-memory endpoint missing `tx`".to_string())?;
    Ok(SharedMemoryEndpoint {
        host_to_sidecar_path,
        host_to_sidecar_data_event,
        host_to_sidecar_space_event,
    })
}

#[repr(C)]
struct SharedByteRingHeader {
    magic: u32,
    version: u32,
    capacity_bytes: u32,
    _reserved: u32,
    write_pos: AtomicU64,
    read_pos: AtomicU64,
}

struct SharedByteRingMapped {
    map: MmapMut,
    capacity_bytes: usize,
}

impl SharedByteRingMapped {
    fn header_size() -> usize {
        mem::size_of::<SharedByteRingHeader>()
    }

    fn open(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|error| format!("open shared-memory ring {}: {error}", path.display()))?;
        let map = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(|error| format!("map shared-memory ring {}: {error}", path.display()))?
        };
        if map.len() < Self::header_size() {
            return Err(format!("shared-memory ring too small: {}", path.display()));
        }

        let header = unsafe { &*(map.as_ptr() as *const SharedByteRingHeader) };
        if header.magic != SHM_MAGIC || header.version != SHM_VERSION {
            return Err(format!(
                "invalid shared-memory ring header: {}",
                path.display()
            ));
        }
        let capacity_bytes = header.capacity_bytes as usize;
        if !(SHM_MIN_CAPACITY..=SHM_MAX_CAPACITY).contains(&capacity_bytes) {
            return Err(format!(
                "invalid shared-memory ring capacity {} for {}",
                capacity_bytes,
                path.display()
            ));
        }
        let expected = Self::header_size()
            .checked_add(capacity_bytes)
            .ok_or_else(|| "shared-memory ring capacity overflow".to_string())?;
        if expected != map.len() {
            return Err(format!(
                "shared-memory ring size mismatch for {}: expect {}, got {}",
                path.display(),
                expected,
                map.len()
            ));
        }

        Ok(Self {
            map,
            capacity_bytes,
        })
    }

    fn header(&self) -> &SharedByteRingHeader {
        unsafe { &*(self.map.as_ptr() as *const SharedByteRingHeader) }
    }

    fn read_bytes(&mut self, out: &mut [u8]) -> usize {
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

    fn occupied_len(&self) -> usize {
        let header = self.header();
        let write_pos = header.write_pos.load(Ordering::Acquire);
        let read_pos = header.read_pos.load(Ordering::Relaxed);
        write_pos
            .saturating_sub(read_pos)
            .min(self.capacity_bytes as u64) as usize
    }

    fn free_len(&self) -> usize {
        self.capacity_bytes.saturating_sub(self.occupied_len())
    }
}

#[cfg(windows)]
struct NamedEventHandle(HANDLE);

#[cfg(windows)]
unsafe impl Send for NamedEventHandle {}

#[cfg(windows)]
impl Drop for NamedEventHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
struct SharedMemoryEvents {
    data_available: NamedEventHandle,
    space_available: NamedEventHandle,
}

#[cfg(windows)]
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

    fn sync_after_ring_change(&self, occupied_bytes: usize, free_bytes: usize) -> Result<(), String> {
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

    fn wait_for_data(&self, timeout_ms: Option<u32>) -> Result<bool, String> {
        let timeout = timeout_ms.unwrap_or(u32::MAX);
        let result = unsafe { WaitForSingleObject(self.data_available.0, timeout) };
        if result == WAIT_OBJECT_0 {
            Ok(true)
        } else if result == WAIT_TIMEOUT {
            Ok(false)
        } else {
            Err(format!("unexpected wait result: {result:?}"))
        }
    }
}

#[cfg(windows)]
struct MmcssGuard(#[allow(dead_code)] HANDLE);

#[cfg(windows)]
unsafe impl Send for MmcssGuard {}

#[cfg(windows)]
#[derive(Default)]
struct MmcssState {
    attempted: bool,
    guard: Option<MmcssGuard>,
}

#[cfg(windows)]
impl MmcssState {
    fn ensure_pro_audio(&mut self) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        self.guard = enable_mmcss_pro_audio();
        if self.guard.is_some() {
            eprintln!("asio host data ingress mmcss: Pro Audio enabled");
        } else {
            eprintln!("asio host data ingress mmcss: failed to enable Pro Audio");
        }
    }
}

#[cfg(windows)]
fn enable_mmcss_pro_audio() -> Option<MmcssGuard> {
    let mut task_index = 0u32;
    let task = HSTRING::from("Pro Audio");
    let handle = unsafe { AvSetMmThreadCharacteristicsW(&task, &mut task_index) }.ok()?;
    let _ = unsafe { AvSetMmThreadPriority(handle, AVRT_PRIORITY_HIGH) };
    Some(MmcssGuard(handle))
}
