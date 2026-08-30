use std::io::{BufReader, BufWriter, Write};
use std::mem;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use memmap2::{MmapMut, MmapOptions};
use serde::{Deserialize, Serialize};
use stellatune_asio_proto::{
    AudioSpec, PROTOCOL_VERSION, Request, Response, read_frame, write_frame,
};
use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_core::pipeline::stages::{Stage, StageFlow};
use tempfile::NamedTempFile;

const SHM_MAGIC: u32 = 0x5354_4D52;
const SHM_VERSION: u32 = 1;
const DEFAULT_RING_BYTES: usize = 256 * 1024;
const WRITE_TIMEOUT: Duration = Duration::from_millis(250);

/// SDK-neutral configuration for an independently supplied external sink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSinkConfig {
    pub executable: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub selection_session_id: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub buffer_size_frames: Option<u32>,
    #[serde(default = "default_ring_bytes")]
    pub ring_capacity_bytes: usize,
}

fn default_ring_bytes() -> usize {
    DEFAULT_RING_BYTES
}

/// Native proxy stage. JSON/Actor mailboxes never carry its PCM payload.
pub struct ExternalSinkProxy {
    config: ExternalSinkConfig,
    process: Option<ExternalSinkProcess>,
    prepared_spec: Option<StreamSpec>,
    started: bool,
}

impl ExternalSinkProxy {
    pub fn new(config: ExternalSinkConfig) -> Self {
        Self {
            config,
            process: None,
            prepared_spec: None,
            started: false,
        }
    }

    fn process_mut(&mut self) -> Result<&mut ExternalSinkProcess, PipelineError> {
        self.process
            .as_mut()
            .ok_or_else(|| PipelineError::StageFailure("external sink is not running".into()))
    }
}

impl Stage for ExternalSinkProxy {}

impl SinkStage for ExternalSinkProxy {
    fn prepare(
        &mut self,
        spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.stop_process();
        let mut process = ExternalSinkProcess::launch(&self.config)?;
        let (selection_session_id, device_id) = match (
            self.config.selection_session_id.as_deref(),
            self.config.device_id.as_deref(),
        ) {
            (Some(session), Some(device)) => (session.to_string(), device.to_string()),
            _ => match process.request(Request::ListDevices)? {
                Response::Devices { devices } => {
                    let device = devices.into_iter().next().ok_or_else(|| {
                        PipelineError::StageFailure("external sink reported no devices".into())
                    })?;
                    (device.selection_session_id, device.id)
                },
                response => return Err(unexpected(response)),
            },
        };
        let prepared_switch_id = match process.request(Request::PrepareDeviceSwitch {
            selection_session_id: selection_session_id.clone(),
            device_id: device_id.clone(),
        })? {
            Response::PreparedDeviceSwitch {
                prepared_switch_id, ..
            } => prepared_switch_id,
            response => return Err(unexpected(response)),
        };
        expect_ok(process.request(Request::Open {
            prepared_switch_id,
            selection_session_id,
            device_id,
            spec: AudioSpec {
                sample_rate: spec.sample_rate,
                channels: spec.channels,
            },
            buffer_size_frames: self.config.buffer_size_frames,
            queue_capacity_ms: None,
        })?)?;
        self.process = Some(process);
        self.prepared_spec = Some(spec);
        self.started = false;
        Ok(())
    }

    fn write(
        &mut self,
        block: &AudioBlock,
        _ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError> {
        let spec = self.prepared_spec.ok_or(PipelineError::NotPrepared)?;
        if block.channels != spec.channels {
            return Err(PipelineError::StageFailure(format!(
                "external sink channel mismatch: expected {}, got {}",
                spec.channels, block.channels
            )));
        }
        let mut bytes = Vec::with_capacity(block.samples.len() * mem::size_of::<f32>());
        for sample in &block.samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let started = self.started;
        let process = self.process_mut()?;
        process.ring.write_frame(&bytes, WRITE_TIMEOUT)?;
        if !started {
            expect_ok(process.request(Request::Start)?)?;
            self.started = true;
        }
        Ok(StageFlow::Continue)
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        let Some(process) = self.process.as_mut() else {
            return Ok(());
        };
        let deadline = Instant::now() + Duration::from_millis(400);
        while Instant::now() < deadline {
            match process.request(Request::QueryStatus)? {
                Response::Status {
                    queued_samples: 0, ..
                } => return Ok(()),
                Response::Status { .. } => thread::sleep(Duration::from_millis(2)),
                response => return Err(unexpected(response)),
            }
        }
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.stop_process();
    }
}

impl ExternalSinkProxy {
    fn stop_process(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.request(Request::Stop);
            let _ = process.request(Request::Close);
        }
        self.prepared_spec = None;
        self.started = false;
    }
}

impl Drop for ExternalSinkProxy {
    fn drop(&mut self) {
        self.stop_process();
    }
}

struct ExternalSinkProcess {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
    ring: SharedPcmRing,
}

impl ExternalSinkProcess {
    fn launch(config: &ExternalSinkConfig) -> Result<Self, PipelineError> {
        let ring = SharedPcmRing::create(config.ring_capacity_bytes)?;
        let endpoint = format!("tx={}", ring.path().display());
        let mut child = Command::new(&config.executable)
            .args(&config.args)
            .env(
                "STELLATUNE_SIDECAR_DATA_SAMPLES_SHARED_MEMORY_RING",
                endpoint,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| {
                PipelineError::StageFailure(format!("launch external sink: {error}"))
            })?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| PipelineError::StageFailure("external sink stdin unavailable".into()))?;
        let output = child.stdout.take().ok_or_else(|| {
            PipelineError::StageFailure("external sink stdout unavailable".into())
        })?;
        let mut process = Self {
            child,
            input: BufWriter::new(input),
            output: BufReader::new(output),
            ring,
        };
        match process.request(Request::Hello {
            version: PROTOCOL_VERSION,
        })? {
            Response::HelloOk { version } if version == PROTOCOL_VERSION => Ok(process),
            response => Err(unexpected(response)),
        }
    }

    fn request(&mut self, request: Request) -> Result<Response, PipelineError> {
        write_frame(&mut self.input, &request).map_err(|error| {
            PipelineError::StageFailure(format!("external sink request: {error}"))
        })?;
        read_frame(&mut self.output).map_err(|error| {
            PipelineError::StageFailure(format!("external sink response: {error}"))
        })
    }
}

impl Drop for ExternalSinkProcess {
    fn drop(&mut self) {
        drop(self.input.flush());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[repr(C)]
struct SharedRingHeader {
    magic: u32,
    version: u32,
    capacity_bytes: u32,
    reserved: u32,
    write_pos: AtomicU64,
    read_pos: AtomicU64,
}

struct SharedPcmRing {
    file: NamedTempFile,
    map: MmapMut,
    capacity: usize,
}

impl SharedPcmRing {
    fn create(requested_capacity: usize) -> Result<Self, PipelineError> {
        let capacity = requested_capacity.clamp(4 * 1024, 64 * 1024 * 1024);
        let file = NamedTempFile::new().map_err(io_error)?;
        file.as_file()
            .set_len((mem::size_of::<SharedRingHeader>() + capacity) as u64)
            .map_err(io_error)?;
        let mut map = unsafe {
            MmapOptions::new()
                .map_mut(file.as_file())
                .map_err(io_error)?
        };
        let header = SharedRingHeader {
            magic: SHM_MAGIC,
            version: SHM_VERSION,
            capacity_bytes: capacity as u32,
            reserved: 0,
            write_pos: AtomicU64::new(0),
            read_pos: AtomicU64::new(0),
        };
        unsafe { ptr::write(map.as_mut_ptr().cast::<SharedRingHeader>(), header) };
        map.flush().map_err(io_error)?;
        Ok(Self {
            file,
            map,
            capacity,
        })
    }

    fn path(&self) -> &std::path::Path {
        self.file.path()
    }

    fn write_frame(&mut self, payload: &[u8], timeout: Duration) -> Result<(), PipelineError> {
        let length: u32 = payload
            .len()
            .try_into()
            .map_err(|_| PipelineError::StageFailure("external PCM frame is too large".into()))?;
        self.write_all(&length.to_le_bytes(), timeout)?;
        self.write_all(payload, timeout)
    }

    fn write_all(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), PipelineError> {
        let deadline = Instant::now() + timeout;
        let mut offset = 0;
        while offset < bytes.len() {
            let written = self.write_some(&bytes[offset..]);
            offset += written;
            if written == 0 {
                if Instant::now() >= deadline {
                    return Err(PipelineError::StageFailure(
                        "external PCM shared-memory ring write timed out".into(),
                    ));
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
        Ok(())
    }

    fn write_some(&mut self, bytes: &[u8]) -> usize {
        let header = unsafe { &*(self.map.as_ptr().cast::<SharedRingHeader>()) };
        let write_pos = header.write_pos.load(Ordering::Relaxed);
        let read_pos = header.read_pos.load(Ordering::Acquire);
        let occupied = write_pos.saturating_sub(read_pos).min(self.capacity as u64) as usize;
        let count = bytes.len().min(self.capacity.saturating_sub(occupied));
        if count == 0 {
            return 0;
        }
        let start = write_pos as usize % self.capacity;
        let first = count.min(self.capacity - start);
        unsafe {
            let data = self
                .map
                .as_mut_ptr()
                .add(mem::size_of::<SharedRingHeader>());
            ptr::copy_nonoverlapping(bytes.as_ptr(), data.add(start), first);
            if first < count {
                ptr::copy_nonoverlapping(bytes.as_ptr().add(first), data, count - first);
            }
        }
        header
            .write_pos
            .store(write_pos + count as u64, Ordering::Release);
        count
    }
}

fn expect_ok(response: Response) -> Result<(), PipelineError> {
    match response {
        Response::Ok => Ok(()),
        Response::Err { message } => Err(PipelineError::StageFailure(message)),
        response => Err(unexpected(response)),
    }
}

fn unexpected(response: Response) -> PipelineError {
    PipelineError::StageFailure(format!("unexpected external sink response: {response:?}"))
}

fn io_error(error: std::io::Error) -> PipelineError {
    PipelineError::StageFailure(format!("external sink shared-memory I/O: {error}"))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use super::{SharedPcmRing, SharedRingHeader};

    #[test]
    fn shared_ring_carries_length_prefixed_pcm_without_json() {
        let mut ring = SharedPcmRing::create(4096).unwrap();
        ring.write_frame(&[1, 2, 3, 4], Duration::from_millis(10))
            .unwrap();
        let header = unsafe { &*(ring.map.as_ptr().cast::<SharedRingHeader>()) };
        assert_eq!(header.write_pos.load(Ordering::Acquire), 8);
        let offset = std::mem::size_of::<SharedRingHeader>();
        assert_eq!(&ring.map[offset..offset + 8], &[4, 0, 0, 0, 1, 2, 3, 4]);
    }
}
