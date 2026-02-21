use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Session token for this exact device snapshot.
    ///
    /// Target IDs are only valid when this session token matches.
    pub selection_session_id: String,
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
        selection_session_id: String,
        device_id: String,
    },
    Open {
        selection_session_id: String,
        device_id: String,
        spec: AudioSpec,
        buffer_size_frames: Option<u32>,
        queue_capacity_ms: Option<u32>,
    },
    Start,
    Stop,
    /// Reset runtime buffering state while keeping device/session opened.
    Reset,
    Close,
    /// Write PCM samples as interleaved f32le bytes (fallback for non-SHM mode).
    WriteSamples {
        interleaved_f32le: Vec<u8>,
    },
    /// Query the current output sink status.
    QueryStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    HelloOk {
        version: u32,
    },
    Devices {
        devices: Vec<DeviceInfo>,
    },
    DeviceCaps {
        caps: DeviceCaps,
    },
    Ok,
    Err {
        message: String,
    },
    /// Response to `WriteSamples`: number of frames accepted.
    WrittenFrames {
        frames: u32,
    },
    /// Response to `QueryStatus`.
    Status {
        queued_samples: u32,
        running: bool,
    },
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
