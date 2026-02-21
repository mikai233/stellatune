use stellatune_asio_proto::{
    AudioSpec, DeviceCaps, DeviceInfo, PROTOCOL_VERSION, Request, Response,
};
use stellatune_wasm_plugin_sdk::error::{SdkError, SdkResult};
use stellatune_wasm_plugin_sdk::__private::parking_lot::{Mutex, MutexGuard};
use stellatune_wasm_plugin_sdk::__private::stellatune_wasm_guest_bindings_output_sink::stellatune::plugin::sidecar;
use std::sync::OnceLock;

use crate::config::AsioOutputConfig;

const DEFAULT_SIDECAR_EXE: &str = "stellatune-asio-host";

/// Communication channel wrapper using WASM sidecar guest bindings.
///
/// - Control channel (stdio): postcard-framed `Request`/`Response`
pub(crate) struct AsioSidecarClient {
    control_channel: sidecar::Channel,
    _process: sidecar::Process,
}

impl AsioSidecarClient {
    pub fn launch(config: &AsioOutputConfig) -> SdkResult<Self> {
        let exe = config
            .sidecar_path
            .as_deref()
            .unwrap_or(DEFAULT_SIDECAR_EXE);

        let spec = sidecar::LaunchSpec {
            executable: exe.to_string(),
            args: config.sidecar_args.clone(),
            preferred_control: vec![sidecar::TransportOption {
                kind: sidecar::TransportKind::Stdio,
                priority: 10,
                max_frame_bytes: None,
            }],
            preferred_data: Vec::new(),
            env: Vec::new(),
        };

        let process =
            sidecar::launch(&spec).map_err(|e| SdkError::io(format!("launch sidecar: {e:?}")))?;

        let control_channel = process
            .open_control()
            .map_err(|e| SdkError::io(format!("open control channel: {e:?}")))?;

        let mut client = Self {
            control_channel,
            _process: process,
        };

        // Handshake
        match client.control_request(Request::Hello {
            version: PROTOCOL_VERSION,
        })? {
            Response::HelloOk { version } if version == PROTOCOL_VERSION => {},
            Response::HelloOk { version } => {
                return Err(SdkError::internal(format!(
                    "ASIO sidecar protocol version mismatch: local={PROTOCOL_VERSION}, remote={version}"
                )));
            },
            other => {
                return Err(SdkError::internal(format!(
                    "unexpected hello response: {other:?}"
                )));
            },
        }

        Ok(client)
    }

    fn control_request(&mut self, req: Request) -> SdkResult<Response> {
        self.send_request(&self.control_channel, &req)?;
        self.read_response(&self.control_channel)
    }

    fn send_request(&self, channel: &sidecar::Channel, req: &Request) -> SdkResult<()> {
        let mut buf = Vec::<u8>::new();
        stellatune_asio_proto::write_frame(&mut buf, req)
            .map_err(|e| SdkError::io(format!("write_frame: {e}")))?;
        channel
            .write(&buf)
            .map_err(|e| SdkError::io(format!("channel write: {e:?}")))?;
        Ok(())
    }

    fn read_response(&self, channel: &sidecar::Channel) -> SdkResult<Response> {
        // Read length prefix first (4 bytes)
        let len_bytes = channel
            .read(4, Some(5000))
            .map_err(|e| SdkError::io(format!("channel read length: {e:?}")))?;
        if len_bytes.len() < 4 {
            return Err(SdkError::io("short read for frame length"));
        }
        let len =
            u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
        if len > 64 * 1024 * 1024 {
            return Err(SdkError::io("frame too large"));
        }

        // Read payload
        let mut payload = Vec::<u8>::new();
        while payload.len() < len {
            let remaining = (len - payload.len()) as u32;
            let chunk = channel
                .read(remaining, Some(5000))
                .map_err(|e| SdkError::io(format!("channel read payload: {e:?}")))?;
            if chunk.is_empty() {
                return Err(SdkError::io("unexpected EOF reading sidecar response"));
            }
            payload.extend_from_slice(&chunk);
        }

        let response: Response = postcard::from_bytes(&payload)
            .map_err(|e| SdkError::io(format!("postcard decode: {e}")))?;
        Ok(response)
    }

    fn expect_ok(&mut self, req: Request) -> SdkResult<()> {
        match self.control_request(req)? {
            Response::Ok => Ok(()),
            Response::Err { message } => Err(SdkError::internal(message)),
            other => Err(SdkError::internal(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn list_devices(&mut self) -> SdkResult<Vec<DeviceInfo>> {
        match self.control_request(Request::ListDevices)? {
            Response::Devices { devices } => Ok(devices),
            Response::Err { message } => Err(SdkError::internal(message)),
            other => Err(SdkError::internal(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn get_device_caps(
        &mut self,
        selection_session_id: String,
        device_id: String,
    ) -> SdkResult<DeviceCaps> {
        match self.control_request(Request::GetDeviceCaps {
            selection_session_id,
            device_id,
        })? {
            Response::DeviceCaps { caps } => Ok(caps),
            Response::Err { message } => Err(SdkError::internal(message)),
            other => Err(SdkError::internal(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn open(
        &mut self,
        selection_session_id: String,
        device_id: String,
        spec: AudioSpec,
        buffer_size_frames: Option<u32>,
        queue_capacity_ms: Option<u32>,
    ) -> SdkResult<()> {
        self.expect_ok(Request::Open {
            selection_session_id,
            device_id,
            spec,
            buffer_size_frames,
            queue_capacity_ms,
        })
    }

    pub fn start(&mut self) -> SdkResult<()> {
        self.expect_ok(Request::Start)
    }

    pub fn stop(&mut self) -> SdkResult<()> {
        self.expect_ok(Request::Stop)
    }

    pub fn close(&mut self) -> SdkResult<()> {
        self.expect_ok(Request::Close)
    }

    pub fn reset(&mut self) -> SdkResult<()> {
        self.expect_ok(Request::Reset)
    }

    pub fn write_samples(&mut self, data: &[u8]) -> SdkResult<u32> {
        match self.control_request(Request::WriteSamples {
            interleaved_f32le: data.to_vec(),
        })? {
            Response::WrittenFrames { frames } => Ok(frames),
            Response::Err { message } => Err(SdkError::internal(message)),
            other => Err(SdkError::internal(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    pub fn query_status(&mut self) -> SdkResult<(u32, bool)> {
        match self.control_request(Request::QueryStatus)? {
            Response::Status {
                queued_samples,
                running,
            } => Ok((queued_samples, running)),
            Response::Err { message } => Err(SdkError::internal(message)),
            other => Err(SdkError::internal(format!(
                "unexpected response: {other:?}"
            ))),
        }
    }
}

#[derive(Default)]
struct SidecarManager {
    signature: Option<String>,
    client: Option<AsioSidecarClient>,
}

impl SidecarManager {
    fn shutdown_current(&mut self) {
        if let Some(client) = self.client.as_mut() {
            let _ = client.stop();
            let _ = client.close();
        }
        self.client = None;
        self.signature = None;
    }

    fn ensure_for(&mut self, config: &AsioOutputConfig) -> SdkResult<&mut AsioSidecarClient> {
        let signature = config_signature(config);
        let needs_restart = self
            .signature
            .as_deref()
            .map(|current| current != signature)
            .unwrap_or(true)
            || self.client.is_none();

        if needs_restart {
            self.shutdown_current();
            self.client = Some(AsioSidecarClient::launch(config)?);
            self.signature = Some(signature.to_string());
        }

        self.client
            .as_mut()
            .ok_or_else(|| SdkError::internal("sidecar manager missing live client"))
    }
}

fn config_signature(config: &AsioOutputConfig) -> String {
    let path = config
        .sidecar_path
        .as_deref()
        .unwrap_or(DEFAULT_SIDECAR_EXE);
    let args = config.sidecar_args.join("\u{1f}");
    format!("{path}\u{1e}{args}")
}

fn sidecar_manager() -> &'static Mutex<SidecarManager> {
    static MANAGER: OnceLock<Mutex<SidecarManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(SidecarManager::default()))
}

fn lock_manager() -> MutexGuard<'static, SidecarManager> {
    sidecar_manager().lock()
}

pub(crate) fn lifecycle_on_enable() -> SdkResult<()> {
    // Prewarm with default config; session-level config can trigger restart.
    with_sidecar(&AsioOutputConfig::default(), |_| Ok(()))
}

pub(crate) fn lifecycle_on_disable() -> SdkResult<()> {
    let mut manager = lock_manager();
    manager.shutdown_current();
    Ok(())
}

pub(crate) fn with_sidecar<T>(
    config: &AsioOutputConfig,
    f: impl FnOnce(&mut AsioSidecarClient) -> SdkResult<T>,
) -> SdkResult<T> {
    let mut manager = lock_manager();
    let client = manager.ensure_for(config)?;
    f(client)
}
