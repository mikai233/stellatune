use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCaps {
    pub default_spec: AudioSpec,
    pub supported_sample_rates: Vec<u32>,
    pub supported_channels: Vec<u16>,
    pub supported_formats: Vec<SampleFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedRingFile {
    /// Path to a memory-mapped file used as the shared ring buffer.
    pub path: String,
    /// Capacity in **samples** (interleaved, i.e. frames * channels).
    pub capacity_samples: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleFormat {
    F32,
    I16,
    I32,
    U16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Hello {
        version: u32,
    },
    ListDevices,
    GetDeviceCaps {
        device_id: String,
    },
    Open {
        device_id: String,
        spec: AudioSpec,
        buffer_size_frames: Option<u32>,
        shared_ring: Option<SharedRingFile>,
    },
    Start,
    Stop,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    HelloOk { version: u32 },
    Devices { devices: Vec<DeviceInfo> },
    DeviceCaps { caps: DeviceCaps },
    Ok,
    Err { message: String },
}

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("postcard: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("unexpected response: {0:?}")]
    UnexpectedResponse(Response),
}

pub fn write_frame<W: std::io::Write, T: Serialize>(mut w: W, msg: &T) -> Result<(), ProtoError> {
    let payload = postcard::to_stdvec(msg)?;
    let len: u32 = payload
        .len()
        .try_into()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"))?;
    w.write_all(&len.to_le_bytes())?;
    w.write_all(&payload)?;
    w.flush()?;
    Ok(())
}

pub fn read_frame<R: std::io::Read, T: for<'de> Deserialize<'de>>(
    mut r: R,
) -> Result<T, ProtoError> {
    let mut len_bytes = [0u8; 4];
    r.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    // Basic sanity limit: 64 MiB.
    if len > 64 * 1024 * 1024 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large").into());
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok(postcard::from_bytes(&payload)?)
}

#[cfg(feature = "shm")]
pub mod shm {
    use std::fs::OpenOptions;
    use std::io;
    use std::mem;
    use std::path::Path;
    use std::ptr;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const SHM_MAGIC: u32 = 0x5354524e; // "STRN"
    pub const SHM_VERSION: u32 = 1;
    const MAX_CAPACITY_SAMPLES: usize = 64 * 1024 * 1024; // sanity limit

    #[repr(C)]
    struct SharedRingHeader {
        magic: u32,
        version: u32,
        capacity_samples: u32,
        channels: u16,
        _reserved: u16,
        write_pos: AtomicU64,
        read_pos: AtomicU64,
    }

    fn header_size() -> usize {
        mem::size_of::<SharedRingHeader>()
    }

    fn total_size_bytes(capacity_samples: usize) -> io::Result<u64> {
        if capacity_samples == 0 || capacity_samples > MAX_CAPACITY_SAMPLES {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad capacity"));
        }
        let bytes = header_size()
            .checked_add(
                capacity_samples
                    .checked_mul(mem::size_of::<f32>())
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "capacity overflow")
                    })?,
            )
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "capacity overflow"))?;
        Ok(bytes as u64)
    }

    pub struct SharedRingMapped {
        map: memmap2::MmapMut,
        capacity_samples: usize,
        channels: u16,
    }

    impl SharedRingMapped {
        pub fn create(path: &Path, capacity_samples: usize, channels: u16) -> io::Result<Self> {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(path)?;
            file.set_len(total_size_bytes(capacity_samples)?)?;

            let mut map = unsafe { memmap2::MmapOptions::new().map_mut(&file)? };
            unsafe {
                let header = map.as_mut_ptr() as *mut SharedRingHeader;
                ptr::write(
                    header,
                    SharedRingHeader {
                        magic: SHM_MAGIC,
                        version: SHM_VERSION,
                        capacity_samples: capacity_samples as u32,
                        channels,
                        _reserved: 0,
                        write_pos: AtomicU64::new(0),
                        read_pos: AtomicU64::new(0),
                    },
                );
                // Zero the data region for deterministic startup.
                ptr::write_bytes(
                    map.as_mut_ptr().add(header_size()),
                    0,
                    map.len() - header_size(),
                );
            }

            Ok(Self {
                map,
                capacity_samples,
                channels,
            })
        }

        pub fn open(path: &Path) -> io::Result<Self> {
            let file = OpenOptions::new().read(true).write(true).open(path)?;
            let map = unsafe { memmap2::MmapOptions::new().map_mut(&file)? };
            if map.len() < header_size() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "shm too small"));
            }
            let header = unsafe { &*(map.as_ptr() as *const SharedRingHeader) };
            if header.magic != SHM_MAGIC || header.version != SHM_VERSION {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "shm header mismatch",
                ));
            }
            let capacity_samples = header.capacity_samples as usize;
            let channels = header.channels;
            let expected_len = total_size_bytes(capacity_samples)? as usize;
            if map.len() != expected_len {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "shm size mismatch",
                ));
            }
            Ok(Self {
                map,
                capacity_samples,
                channels,
            })
        }

        pub fn capacity_samples(&self) -> usize {
            self.capacity_samples
        }

        pub fn channels(&self) -> u16 {
            self.channels
        }

        pub fn reset(&self) {
            let header = unsafe { &*(self.map.as_ptr() as *const SharedRingHeader) };
            header.write_pos.store(0, Ordering::Release);
            header.read_pos.store(0, Ordering::Release);
        }

        pub fn available_to_read(&self) -> usize {
            let header = unsafe { &*(self.map.as_ptr() as *const SharedRingHeader) };
            let w = header.write_pos.load(Ordering::Acquire);
            let r = header.read_pos.load(Ordering::Acquire);
            w.saturating_sub(r).min(self.capacity_samples as u64) as usize
        }

        pub fn available_to_write(&self) -> usize {
            self.capacity_samples
                .saturating_sub(self.available_to_read())
        }

        pub fn write_samples(&self, input: &[f32]) -> usize {
            let header = unsafe { &*(self.map.as_ptr() as *const SharedRingHeader) };
            let r = header.read_pos.load(Ordering::Acquire);
            let w = header.write_pos.load(Ordering::Relaxed);
            let used = w.saturating_sub(r).min(self.capacity_samples as u64) as usize;
            let available = self.capacity_samples.saturating_sub(used);
            let n = available.min(input.len());
            if n == 0 {
                return 0;
            }

            let start = (w as usize) % self.capacity_samples;
            let first = n.min(self.capacity_samples - start);
            unsafe {
                let base = self.map.as_ptr() as *mut u8;
                let data = base.add(header_size()) as *mut f32;
                ptr::copy_nonoverlapping(input.as_ptr(), data.add(start), first);
                if first < n {
                    ptr::copy_nonoverlapping(input.as_ptr().add(first), data, n - first);
                }
            }

            header.write_pos.store(w + n as u64, Ordering::Release);
            n
        }

        pub fn read_samples(&self, out: &mut [f32]) -> usize {
            let header = unsafe { &*(self.map.as_ptr() as *const SharedRingHeader) };
            let w = header.write_pos.load(Ordering::Acquire);
            let r = header.read_pos.load(Ordering::Relaxed);
            let available = w.saturating_sub(r).min(self.capacity_samples as u64) as usize;
            let n = available.min(out.len());
            if n == 0 {
                return 0;
            }

            let start = (r as usize) % self.capacity_samples;
            let first = n.min(self.capacity_samples - start);
            unsafe {
                let base = self.map.as_ptr() as *mut u8;
                let data = base.add(header_size()) as *const f32;
                ptr::copy_nonoverlapping(data.add(start), out.as_mut_ptr(), first);
                if first < n {
                    ptr::copy_nonoverlapping(data, out.as_mut_ptr().add(first), n - first);
                }
            }

            header.read_pos.store(r + n as u64, Ordering::Release);
            n
        }
    }
}
