//! Single-producer/single-consumer PCM transport shared by the core and ASIO host.
//! The temporary mapping is allocated once per stream and is never explicitly
//! flushed. Control RPC never carries PCM when this transport is active.
use std::fs::OpenOptions;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};

use memmap2::{MmapMut, MmapOptions};
use tempfile::NamedTempFile;

const MAGIC: u32 = 0x53544D52;
const VERSION: u32 = 1;
const MIN_CAPACITY: usize = 4096;
const MAX_CAPACITY: usize = 64 * 1024 * 1024;

#[repr(C)]
struct Header {
    magic: u32,
    version: u32,
    capacity_bytes: u32,
    reserved: u32,
    write_pos: AtomicU64,
    read_pos: AtomicU64,
}

pub struct SharedByteRingMapped {
    map: MmapMut,
    capacity: usize,
}

impl SharedByteRingMapped {
    pub fn create(capacity: usize) -> Result<(NamedTempFile, Self), String> {
        let capacity = capacity.clamp(MIN_CAPACITY, MAX_CAPACITY);
        let file = NamedTempFile::new().map_err(|e| e.to_string())?;
        file.as_file()
            .set_len((size_of::<Header>() + capacity) as u64)
            .map_err(|e| e.to_string())?;
        let mut map =
            unsafe { MmapOptions::new().map_mut(file.as_file()) }.map_err(|e| e.to_string())?;
        // Mapping starts page-aligned and remains alive while either endpoint uses it.
        unsafe {
            ptr::write(
                map.as_mut_ptr().cast::<Header>(),
                Header {
                    magic: MAGIC,
                    version: VERSION,
                    capacity_bytes: capacity as u32,
                    reserved: 0,
                    write_pos: AtomicU64::new(0),
                    read_pos: AtomicU64::new(0),
                },
            );
        }
        Ok((file, Self { map, capacity }))
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("open PCM mapping: {e}"))?;
        let map = unsafe { MmapOptions::new().map_mut(&file) }.map_err(|e| e.to_string())?;
        if map.len() < size_of::<Header>() {
            return Err("PCM mapping is truncated".into());
        }
        let header = unsafe { &*map.as_ptr().cast::<Header>() };
        let capacity = header.capacity_bytes as usize;
        if header.magic != MAGIC
            || header.version != VERSION
            || !(MIN_CAPACITY..=MAX_CAPACITY).contains(&capacity)
            || map.len() != size_of::<Header>() + capacity
        {
            return Err("invalid PCM mapping header".into());
        }
        Ok(Self { map, capacity })
    }

    fn header(&self) -> &Header {
        unsafe { &*self.map.as_ptr().cast::<Header>() }
    }
    pub fn occupied_len(&self) -> usize {
        let h = self.header();
        h.write_pos
            .load(Ordering::Acquire)
            .saturating_sub(h.read_pos.load(Ordering::Acquire))
            .min(self.capacity as u64) as usize
    }
    pub fn free_len(&self) -> usize {
        self.capacity - self.occupied_len()
    }

    /// Producer only. Publish complete, frame-aligned messages atomically.
    /// Returns accepted audio frames; zero means backpressure.
    pub fn write_samples(&mut self, samples: &[f32], channels: usize) -> usize {
        if channels == 0 {
            return 0;
        }
        let frames = (self.free_len().saturating_sub(4) / (channels * 4))
            .min(samples.len() / channels)
            .min(4096);
        if frames == 0 {
            return 0;
        }
        let bytes = frames * channels * 4;
        let mut payload = Vec::with_capacity(bytes + 4);
        payload.extend_from_slice(&(bytes as u32).to_le_bytes());
        for sample in &samples[..frames * channels] {
            payload.extend_from_slice(&sample.to_le_bytes());
        }
        let h = self.header();
        let pos = h.write_pos.load(Ordering::Relaxed);
        let start = pos as usize % self.capacity;
        let first = payload.len().min(self.capacity - start);
        // SPSC ownership: the consumer cannot access unpublished bytes; release
        // publication pairs with its acquire load before accessing the payload.
        unsafe {
            let data = self.map.as_ptr().add(size_of::<Header>()).cast_mut();
            ptr::copy_nonoverlapping(payload.as_ptr(), data.add(start), first);
            ptr::copy_nonoverlapping(payload.as_ptr().add(first), data, payload.len() - first);
        }
        h.write_pos
            .store(pos + payload.len() as u64, Ordering::Release);
        frames
    }

    /// Consumer only. May read a prefix of a framed message.
    pub fn read_bytes(&mut self, out: &mut [u8]) -> usize {
        let h = self.header();
        let pos = h.read_pos.load(Ordering::Relaxed);
        let count = self.occupied_len().min(out.len());
        let start = pos as usize % self.capacity;
        let first = count.min(self.capacity - start);
        unsafe {
            let data = self.map.as_ptr().add(size_of::<Header>());
            ptr::copy_nonoverlapping(data.add(start), out.as_mut_ptr(), first);
            ptr::copy_nonoverlapping(data, out.as_mut_ptr().add(first), count - first);
        }
        h.read_pos.store(pos + count as u64, Ordering::Release);
        count
    }
    /// Consumer only, after the producer has stopped publishing for reset.
    pub fn discard_all(&self) {
        let h = self.header();
        h.read_pos
            .store(h.write_pos.load(Ordering::Acquire), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn separate_mappings_wrap_and_preserve_whole_pcm_frames() {
        let (file, mut writer) = SharedByteRingMapped::create(4096).unwrap();
        let mut reader = SharedByteRingMapped::open(file.path()).unwrap();
        for _ in 0..20 {
            let samples = vec![0.25; 900];
            assert_eq!(writer.write_samples(&samples, 2), 450);
            let remainder = writer.write_samples(&samples, 2);
            assert!(remainder < 450);
            assert_eq!(writer.write_samples(&samples, 2), 0);
            let mut header = [0; 4];
            assert_eq!(reader.read_bytes(&mut header), 4);
            assert_eq!(u32::from_le_bytes(header), 3600);
            let mut bytes = vec![0; 3600];
            assert_eq!(reader.read_bytes(&mut bytes), 3600);
            assert!(
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|b| *b == 0.25_f32.to_le_bytes())
            );
            reader.discard_all();
            assert_eq!(writer.occupied_len(), 0);
        }
    }
}
