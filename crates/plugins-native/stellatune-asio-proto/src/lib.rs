use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod shared_ring;

pub const PROTOCOL_VERSION: u32 = 10;

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

impl DeviceCaps {
    /// A driver may report zero for its current rate before a stream starts.
    /// Select only from enumerated supported rates; never treat that sentinel as PCM.
    pub fn resolve_sample_rate(&self, requested: Option<u32>) -> Result<u32, String> {
        let supported = |rate: u32| rate != 0 && self.supported_sample_rates.contains(&rate);
        if let Some(rate) = requested {
            return if supported(rate) {
                Ok(rate)
            } else {
                Err(format!("ASIO device does not support {rate} Hz"))
            };
        }
        [self.default_spec.sample_rate, 48_000, 44_100]
            .into_iter()
            .find(|&rate| supported(rate))
            .or_else(|| {
                self.supported_sample_rates
                    .iter()
                    .copied()
                    .filter(|&rate| rate != 0)
                    .min()
            })
            .ok_or_else(|| "ASIO device reported no supported non-zero sample rate".into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleFormat {
    F32,
    I16,
    I32,
    U16,
    I24,
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
    PrepareDeviceSwitch {
        selection_session_id: String,
        device_id: String,
    },
    Open {
        prepared_switch_id: u64,
        selection_session_id: String,
        device_id: String,
        spec: AudioSpec,
        buffer_size_frames: Option<u32>,
        queue_capacity_ms: Option<u32>,
    },
    Start,
    Pause,
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
    PreparedDeviceSwitch {
        prepared_switch_id: u64,
        caps: DeviceCaps,
    },
    Opened {
        buffer_size_frames: u32,
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
        consumed_frames: u64,
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

#[cfg(test)]
mod tests {
    use super::{AudioSpec, DeviceCaps};

    fn caps(default_rate: u32, rates: &[u32]) -> DeviceCaps {
        DeviceCaps {
            default_spec: AudioSpec {
                sample_rate: default_rate,
                channels: 2,
            },
            supported_sample_rates: rates.to_vec(),
            supported_channels: vec![2],
            supported_formats: vec![],
        }
    }

    #[test]
    fn unknown_default_uses_only_reported_supported_rates() {
        assert_eq!(
            caps(0, &[0, 44100, 48000])
                .resolve_sample_rate(None)
                .unwrap(),
            48000
        );
        assert_eq!(
            caps(0, &[0, 96000, 44100])
                .resolve_sample_rate(None)
                .unwrap(),
            44100
        );
        assert_eq!(
            caps(0, &[96000, 32000, 0])
                .resolve_sample_rate(None)
                .unwrap(),
            32000
        );
    }

    #[test]
    fn supported_default_and_explicit_rate_take_precedence() {
        let caps = caps(192000, &[44100, 48000, 192000]);
        assert_eq!(caps.resolve_sample_rate(None).unwrap(), 192000);
        assert_eq!(caps.resolve_sample_rate(Some(44100)).unwrap(), 44100);
        assert!(caps.resolve_sample_rate(Some(96000)).is_err());
    }

    #[test]
    fn zero_is_never_a_valid_requested_or_fallback_rate() {
        assert!(caps(0, &[0]).resolve_sample_rate(None).is_err());
        assert!(caps(48000, &[]).resolve_sample_rate(None).is_err());
        assert!(caps(0, &[0, 48000]).resolve_sample_rate(Some(0)).is_err());
        assert_eq!(
            caps(12345, &[44100]).resolve_sample_rate(None).unwrap(),
            44100
        );
    }
}
