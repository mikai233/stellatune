use super::{AsioSinkFactory, connection::Connection, select_device};
use std::{
    sync::{Arc, atomic::Ordering},
    thread,
    time::{Duration, Instant},
};
use stellatune_asio_proto::{AudioSpec, Request, Response, shared_ring::SharedByteRingMapped};
use stellatune_audio_core::{
    buffering::{BufferingConfig, frames_for_ms},
    error::SinkError,
    format::{AudioBlock, PcmFormat},
    sink::{SinkClockSnapshot, SinkStage, SinkWriteResult, SinkWriteState},
};
use tempfile::NamedTempFile;

pub(crate) struct AsioSink {
    route: Arc<AsioSinkFactory>,
    connection: Option<Arc<Connection>>,
    mapping: Option<SharedByteRingMapped>,
    mapping_file: Option<NamedTempFile>,
    buffering: BufferingConfig,
    capacity_frames: usize,
    accepted: u64,
    base: u64,
    epoch: u64,
    format: Option<PcmFormat>,
}
impl AsioSink {
    pub fn new(route: Arc<AsioSinkFactory>) -> Self {
        Self {
            route,
            connection: None,
            mapping: None,
            mapping_file: None,
            buffering: BufferingConfig::default(),
            capacity_frames: 0,
            accepted: 0,
            base: 0,
            epoch: 0,
            format: None,
        }
    }
    fn connection(&self) -> Result<&Connection, SinkError> {
        self.connection
            .as_deref()
            .ok_or_else(|| failed("ASIO output is not open".into()))
    }
}
fn failed(message: String) -> SinkError {
    SinkError::Failed { message }
}
impl SinkStage for AsioSink {
    fn configure_buffering(&mut self, config: BufferingConfig) {
        self.buffering = config;
    }
    fn open(&mut self, format: PcmFormat) -> Result<(), SinkError> {
        self.close();
        if !self.route.supports_format(format) {
            return Err(SinkError::Unsupported);
        }
        self.capacity_frames = frames_for_ms(format, self.buffering.device_ms)
            .max(self.route.config.buffer_size_frames.unwrap_or(0) as usize);
        let bytes = self.capacity_frames * usize::from(format.channel_layout.channel_count()) * 4;
        let (file, mapping) = SharedByteRingMapped::create(bytes + 4096).map_err(failed)?;
        let connection = self.route.connect(file.path()).map_err(failed)?;
        let device = select_device(&connection, &self.route.device_id).map_err(failed)?;
        let prepared_switch_id = match connection
            .request(Request::PrepareDeviceSwitch {
                selection_session_id: device.selection_session_id.clone(),
                device_id: device.id.clone(),
            })
            .map_err(failed)?
        {
            Response::PreparedDeviceSwitch {
                prepared_switch_id, ..
            } => prepared_switch_id,
            other => {
                return Err(failed(format!(
                    "unexpected ASIO prepare response: {other:?}"
                )));
            },
        };
        let opened = connection
            .request(Request::Open {
                prepared_switch_id,
                selection_session_id: device.selection_session_id,
                device_id: device.id,
                spec: AudioSpec {
                    sample_rate: format.sample_rate,
                    channels: format.channel_layout.channel_count(),
                },
                buffer_size_frames: self.route.config.buffer_size_frames,
                queue_capacity_ms: Some(self.buffering.device_ms),
            })
            .map_err(failed)?;
        let Response::Opened { buffer_size_frames } = opened else {
            return Err(failed(format!("unexpected ASIO open response: {opened:?}")));
        };
        // The software time budget cannot be smaller than a hardware callback.
        // The driver may choose a larger default than an explicitly requested one.
        self.capacity_frames = self.capacity_frames.max(buffer_size_frames as usize * 2);
        self.connection = Some(connection);
        self.mapping = Some(mapping);
        self.mapping_file = Some(file);
        self.accepted = 0;
        self.base = 0;
        self.epoch = self.epoch.wrapping_add(1);
        self.format = Some(format);
        Ok(())
    }
    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        self.connection()?.check().map_err(failed)?;
        if Some(block.format) != self.format {
            return Err(SinkError::Unsupported);
        }
        block.validate().map_err(|e| failed(e.to_string()))?;
        let available = self
            .capacity_frames
            .saturating_sub(self.clock_snapshot().buffered_frames as usize);
        let channels = usize::from(block.format.channel_layout.channel_count());
        let samples = block.frames().min(available) * channels;
        let consumed_frames = self
            .mapping
            .as_mut()
            .expect("opened mapping")
            .write_samples(&block.samples[..samples], channels);
        self.accepted += consumed_frames as u64;
        Ok(SinkWriteResult {
            consumed_frames,
            state: if consumed_frames < block.frames() {
                SinkWriteState::WouldBlock
            } else {
                SinkWriteState::Ready
            },
        })
    }
    fn pause(&mut self) -> Result<(), SinkError> {
        self.connection()?.ok(Request::Pause).map_err(failed)
    }
    fn resume(&mut self) -> Result<(), SinkError> {
        self.connection()?.ok(Request::Start).map_err(failed)
    }
    fn discard(&mut self) -> Result<(), SinkError> {
        let connection = self.connection()?;
        connection.ok(Request::Reset).map_err(failed)?;
        // Ordered after Reset; no old status reply can arrive after this baseline.
        connection.request(Request::QueryStatus).map_err(failed)?;
        self.base = connection.consumed.load(Ordering::Acquire);
        self.accepted = 0;
        self.epoch = self.epoch.wrapping_add(1);
        Ok(())
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        let consumed = self
            .connection
            .as_ref()
            .map_or(0, |c| c.consumed.load(Ordering::Acquire))
            .saturating_sub(self.base)
            .min(self.accepted);
        SinkClockSnapshot {
            consumed_frames: consumed,
            buffered_frames: self.accepted.saturating_sub(consumed),
            epoch: self.epoch,
        }
    }
    fn drain(&mut self) -> Result<(), SinkError> {
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.clock_snapshot().buffered_frames != 0 {
            self.connection()?.check().map_err(failed)?;
            if Instant::now() >= deadline {
                return Err(failed("ASIO drain timed out".into()));
            }
            thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }
    fn close(&mut self) {
        self.format = None;
        if let Some(connection) = self.connection.take() {
            connection.close();
        }
        self.mapping = None;
        self.mapping_file = None;
    }
}
impl Drop for AsioSink {
    fn drop(&mut self) {
        self.close();
    }
}
