use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, Builder, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) use stellatune_asio_proto::shared_ring::SharedByteRingMapped;

use crate::platform::data_channel::{DataIngressThreadPlatformState, ReaderPlatformState};
use crate::stream::StreamIngress;

const DATA_FRAME_MAX_BYTES: usize = 16 * 1024 * 1024;
const DATA_POLL_INTERVAL: Duration = Duration::from_millis(1);

pub(crate) struct DataIngressPump {
    current_ingress: Arc<Mutex<IngressSlot>>,
    reset_requested: Arc<AtomicU64>,
    reset_completed: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct IngressSlot {
    generation: u64,
    ingress: Option<StreamIngress>,
}

impl DataIngressPump {
    pub(crate) fn from_env() -> Result<Option<Self>, String> {
        let Some(mut reader) = SharedMemoryDataReader::open_from_env()? else {
            return Ok(None);
        };

        let current_ingress = Arc::new(Mutex::new(IngressSlot::default()));
        let reset_requested = Arc::new(AtomicU64::new(0));
        let reset_completed = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let ingress_ref = Arc::clone(&current_ingress);
        let reset_ref = Arc::clone(&reset_requested);
        let reset_completed_ref = Arc::clone(&reset_completed);
        let stop_ref = Arc::clone(&stop);
        let join = Builder::new()
            .name("stellatune-asio-data".to_string())
            .spawn(move || {
                run_data_ingress(
                    &mut reader,
                    ingress_ref,
                    reset_ref,
                    reset_completed_ref,
                    stop_ref,
                )
            })
            .map_err(|error| format!("spawn asio data ingress thread: {error}"))?;

        Ok(Some(Self {
            current_ingress,
            reset_requested,
            reset_completed,
            stop,
            join: Some(join),
        }))
    }

    pub(crate) fn set_ingress(&self, ingress: Option<StreamIngress>) {
        if let Ok(mut slot) = self.current_ingress.lock() {
            slot.generation = slot.generation.wrapping_add(1);
            slot.ingress = ingress;
        }
    }

    pub(crate) fn request_reset_and_wait(&self, timeout: Duration) -> Result<(), String> {
        let reset_id = self.reset_requested.fetch_add(1, Ordering::AcqRel) + 1;
        let deadline = Instant::now() + timeout;
        while self.reset_completed.load(Ordering::Acquire) < reset_id {
            if self.stop.load(Ordering::Acquire) {
                return Err("data ingress pump stopped while waiting for reset".to_string());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "timed out waiting for data ingress reset after {}ms",
                    timeout.as_millis()
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
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
    current_ingress: Arc<Mutex<IngressSlot>>,
    reset_requested: Arc<AtomicU64>,
    reset_completed: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
) {
    let mut platform_state = DataIngressThreadPlatformState::new();
    platform_state.on_thread_start();

    let mut len_bytes = [0_u8; 4];
    let mut payload = Vec::<u8>::new();
    let mut last_completed_reset = 0_u64;

    while !stop.load(Ordering::Acquire) {
        let requested_reset = reset_requested.load(Ordering::Acquire);
        if requested_reset > last_completed_reset {
            let _ = reader.discard_pending_data();
            payload.clear();
            last_completed_reset = requested_reset;
            reset_completed.store(last_completed_reset, Ordering::Release);
        }

        let got_len = match reader.read_exact(&mut len_bytes, &stop, Duration::from_millis(100)) {
            Ok(got_frame) => got_frame,
            Err(error) => {
                tracing::warn!(
                    "asio host data ingress stopped while reading frame length: {error}"
                );
                break;
            },
        };
        if !got_len {
            continue;
        }

        let frame_len = u32::from_le_bytes(len_bytes) as usize;
        if frame_len > DATA_FRAME_MAX_BYTES {
            tracing::warn!("asio host data ingress rejected oversized frame: {frame_len} bytes");
            let _ = reader.discard_pending_data();
            continue;
        }

        payload.resize(frame_len, 0);
        if frame_len > 0 {
            match reader.read_exact(payload.as_mut_slice(), &stop, Duration::from_millis(250)) {
                Ok(true) => {},
                Ok(false) => continue,
                Err(error) => {
                    tracing::warn!("asio host data ingress stopped while reading payload: {error}");
                    break;
                },
            }
        }

        let mut payload_offset = 0usize;
        let mut payload_generation = None::<u64>;
        while payload_offset < payload.len() && !stop.load(Ordering::Acquire) {
            let requested_reset = reset_requested.load(Ordering::Acquire);
            if requested_reset > last_completed_reset {
                let _ = reader.discard_pending_data();
                payload.clear();
                last_completed_reset = requested_reset;
                reset_completed.store(last_completed_reset, Ordering::Release);
                break;
            }
            let (generation, ingress) = current_ingress
                .lock()
                .ok()
                .map(|slot| (slot.generation, slot.ingress.clone()))
                .unwrap_or((0, None));
            let Some(ingress) = ingress else {
                tracing::warn!(
                    "asio host data ingress dropped frame while no active ingress: remaining_bytes={}",
                    payload.len().saturating_sub(payload_offset)
                );
                let _ = reader.discard_pending_data();
                break;
            };
            if let Some(expected_generation) = payload_generation {
                if generation != expected_generation {
                    tracing::warn!(
                        "asio host data ingress dropped frame after ingress switch: remaining_bytes={} old_generation={} new_generation={}",
                        payload.len().saturating_sub(payload_offset),
                        expected_generation,
                        generation
                    );
                    let _ = reader.discard_pending_data();
                    break;
                }
            } else {
                payload_generation = Some(generation);
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
                    tracing::warn!("asio host data ingress write failed: {error}");
                    let _ = reader.discard_pending_data();
                    break;
                },
            }
        }
    }
}

struct SharedMemoryDataReader {
    ring: SharedByteRingMapped,
    platform: ReaderPlatformState,
}

impl SharedMemoryDataReader {
    fn open_from_env() -> Result<Option<Self>, String> {
        if let Some(path) = std::env::var_os("STELLATUNE_ASIO_PCM_MAPPING") {
            let config = SharedMemoryEndpoint {
                host_to_sidecar_path: path.to_string_lossy().into_owned(),
                host_to_sidecar_data_event: None,
                host_to_sidecar_space_event: None,
            };
            return Ok(Some(Self {
                ring: SharedByteRingMapped::open(Path::new(&path))?,
                platform: ReaderPlatformState::open(&config)?,
            }));
        }
        let Some(endpoint) = resolve_data_endpoint() else {
            return Ok(None);
        };

        let config = parse_shared_memory_endpoint(endpoint.as_str())?;
        let ring = SharedByteRingMapped::open(Path::new(config.host_to_sidecar_path.as_str()))?;
        Ok(Some(Self {
            ring,
            platform: ReaderPlatformState::open(&config)?,
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
                self.platform.sync_after_ring_change(&self.ring)?;
                continue;
            }

            if Instant::now() >= deadline {
                if offset == 0 {
                    return Ok(false);
                }
                return Err("timed out waiting for shared-memory data frame".to_string());
            }

            if let Some(got_data) = self.platform.wait_for_data_until(deadline)? {
                if got_data {
                    continue;
                }
                return Ok(false);
            }

            thread::sleep(DATA_POLL_INTERVAL);
        }

        Ok(true)
    }

    fn discard_pending_data(&mut self) -> Result<(), String> {
        self.ring.discard_all();
        self.platform.sync_after_ring_change(&self.ring)
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

pub(crate) struct SharedMemoryEndpoint {
    pub(crate) host_to_sidecar_path: String,
    pub(crate) host_to_sidecar_data_event: Option<String>,
    pub(crate) host_to_sidecar_space_event: Option<String>,
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
            },
            "tx_data_event" => {
                host_to_sidecar_data_event = Some(value.to_string());
            },
            "tx_space_event" => {
                host_to_sidecar_space_event = Some(value.to_string());
            },
            _ => {},
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
