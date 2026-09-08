//! Native ASIO output adapter. The installed host owns the ASIO driver;
//! this crate owns bounded PCM transport and implements the core sink contract.
mod connection;
mod sink;

use connection::Connection;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};
use stellatune_asio_proto::{DeviceCaps, DeviceInfo, Request, Response};
use stellatune_audio_core::{
    error::FactoryError,
    format::{ChannelLayout, PcmFormat},
    sink::{OutputCompatibilityKey, SinkFactory, SinkStage},
    stage::StageId,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsioConfig {
    #[serde(default)]
    pub buffer_size_frames: Option<u32>,
    /// None uses the driver's default sample rate.
    #[serde(default)]
    pub sample_rate: Option<u32>,
}
impl AsioConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .buffer_size_frames
            .is_some_and(|n| n == 0 || n > 65_536)
        {
            return Err("ASIO buffer_size_frames must be between 1 and 65536".into());
        }
        if self
            .sample_rate
            .is_some_and(|n| !(8000..=768000).contains(&n))
        {
            return Err("ASIO sample_rate must be between 8000 and 768000".into());
        }
        Ok(())
    }
}

pub fn list_devices(executable: &Path) -> Result<Vec<DeviceInfo>, String> {
    let connection = Connection::spawn(executable, None)?;
    devices(&connection)
}
fn devices(connection: &Connection) -> Result<Vec<DeviceInfo>, String> {
    match connection.request(Request::ListDevices)? {
        Response::Devices { devices } => Ok(devices),
        other => Err(format!("unexpected ASIO device response: {other:?}")),
    }
}
fn select_device(connection: &Connection, device_id: &str) -> Result<DeviceInfo, String> {
    devices(connection)?
        .into_iter()
        .find(|device| device.id == device_id)
        .ok_or_else(|| format!("ASIO device is unavailable: {device_id}"))
}

#[derive(Clone)]
pub struct AsioSinkFactory {
    id: StageId,
    executable: PathBuf,
    device_id: String,
    config: AsioConfig,
    format: PcmFormat,
    supported_sample_rates: Vec<u32>,
    match_track_sample_rate: bool,
    revision: u64,
    valid: Arc<AtomicBool>,
    // Opening a replacement releases the previous driver instance first.
    connection: Arc<Mutex<Weak<Connection>>>,
}
impl AsioSinkFactory {
    pub fn discover(
        executable: PathBuf,
        device_id: String,
        config: AsioConfig,
        revision: u64,
    ) -> Result<Arc<Self>, String> {
        config.validate()?;
        let connection = Connection::spawn(&executable, None)?;
        let device = select_device(&connection, &device_id)?;
        let caps = match connection.request(Request::GetDeviceCaps {
            selection_session_id: device.selection_session_id,
            device_id: device.id,
        })? {
            Response::DeviceCaps { caps } => caps,
            other => return Err(format!("unexpected ASIO capabilities response: {other:?}")),
        };
        Self::from_caps(executable, device_id, config, caps, revision)
    }
    fn from_caps(
        executable: PathBuf,
        device_id: String,
        config: AsioConfig,
        caps: DeviceCaps,
        revision: u64,
    ) -> Result<Arc<Self>, String> {
        config.validate()?;
        let sample_rate = config.sample_rate.unwrap_or(caps.default_spec.sample_rate);
        if !caps.supported_sample_rates.contains(&sample_rate) {
            return Err(format!("ASIO device does not support {sample_rate} Hz"));
        }
        // Device channel counts carry no speaker positions. Music output is
        // explicitly mono/stereo; never guess a multichannel speaker layout.
        let channel_layout = if caps.supported_channels.contains(&2) {
            ChannelLayout::STEREO
        } else if caps.supported_channels.contains(&1) {
            ChannelLayout::MONO
        } else {
            return Err("ASIO device has no mono/stereo output configuration".into());
        };
        Ok(Arc::new(Self {
            id: StageId::new("native.asio").expect("stage id"),
            executable,
            device_id,
            config,
            format: PcmFormat {
                sample_rate,
                channel_layout,
            },
            supported_sample_rates: caps.supported_sample_rates,
            match_track_sample_rate: false,
            revision,
            valid: Arc::new(AtomicBool::new(true)),
            connection: Arc::new(Mutex::new(Weak::new())),
        }))
    }
    pub fn format(&self) -> PcmFormat {
        self.format
    }
    /// Explicit plugin sample_rate takes precedence over the application option.
    /// Unsupported track rates fall back to the negotiated driver default.
    pub fn with_match_track_sample_rate(&self, enabled: bool) -> Self {
        let mut factory = self.clone();
        factory.match_track_sample_rate = enabled;
        factory
    }
    fn supports_format(&self, format: PcmFormat) -> bool {
        format.channel_layout == self.format.channel_layout
            && self.supported_sample_rates.contains(&format.sample_rate)
    }
    /// Called before uninstall/update. Also invalidates any already prepared sink.
    pub fn revoke(&self) {
        self.valid.store(false, Ordering::Release);
        self.release_device();
    }
    pub fn release_device(&self) {
        let mut slot = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(connection) = slot.upgrade() {
            connection.close();
        }
        *slot = Weak::new();
    }
    fn connect(&self, mapping: &Path) -> Result<Arc<Connection>, String> {
        let mut slot = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !self.valid.load(Ordering::Acquire) {
            return Err("ASIO route is no longer active".into());
        }
        if let Some(previous) = slot.upgrade() {
            previous.close();
        }
        let connection = Connection::spawn(&self.executable, Some(mapping))?;
        *slot = Arc::downgrade(&connection);
        Ok(connection)
    }
}
// Clones share revocation and the single active driver connection.
impl SinkFactory for AsioSinkFactory {
    fn id(&self) -> &StageId {
        &self.id
    }
    fn preferred_format(&self, input: PcmFormat) -> Result<PcmFormat, FactoryError> {
        let sample_rate = if self.match_track_sample_rate
            && self.config.sample_rate.is_none()
            && self.supported_sample_rates.contains(&input.sample_rate)
        {
            input.sample_rate
        } else {
            self.format.sample_rate
        };
        Ok(PcmFormat {
            sample_rate,
            ..self.format
        })
    }
    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        Ok(OutputCompatibilityKey {
            backend_id: "native.asio".into(),
            device_id: Some(self.device_id.clone()),
            sample_rate: format.sample_rate,
            channel_layout: format.channel_layout,
            route_revision: self.revision,
        })
    }
    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        if !self.valid.load(Ordering::Acquire) {
            return Err(FactoryError::CreateFailed {
                message: "ASIO route was removed".into(),
            });
        }
        Ok(Box::new(sink::AsioSink::new(Arc::new(self.clone()))))
    }
}
