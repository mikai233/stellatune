use std::sync::OnceLock;
use stellatune_asio_proto::{
    AudioSpec, DeviceCaps, DeviceInfo, PROTOCOL_VERSION, Request, Response,
};
use stellatune_plugin_sdk::__private::parking_lot::{Mutex, MutexGuard};
use stellatune_plugin_sdk::__private::stellatune_world_output_sink::stellatune::plugin::sidecar;
use stellatune_plugin_sdk::error::{SdkError, SdkResult};

use crate::config::AsioOutputConfig;

const DEFAULT_SIDECAR_EXE: &str = "stellatune-asio-host";
const CONTROL_LOCK_NAME: &str = "asio-control";

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
            scope: sidecar::LaunchScope::PackageShared,
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
        let lock = sidecar::lock(CONTROL_LOCK_NAME, Some(5_000))
            .map_err(|e| SdkError::io(format!("acquire control lock: {e:?}")))?;
        let result = (|| {
            self.send_request(&self.control_channel, &req)?;
            self.read_response(&self.control_channel)
        })();
        lock.unlock();
        result
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
    lifecycle_leases: usize,
    signature: Option<String>,
    client: Option<AsioSidecarClient>,
}

impl SidecarManager {
    fn drop_current_client(&mut self) {
        // Do not send protocol-level stop/close here.
        // Lifecycle owns process leases; stream control belongs to session APIs.
        self.client = None;
        self.signature = None;
    }

    fn acquire_lifecycle(&mut self, config: &AsioOutputConfig) -> SdkResult<()> {
        if self.lifecycle_leases > 0 {
            return Ok(());
        }
        self.lifecycle_leases = 1;
        if let Err(error) = self.ensure_for(config) {
            self.lifecycle_leases = 0;
            self.drop_current_client();
            return Err(error);
        }
        Ok(())
    }

    fn ensure_for(&mut self, config: &AsioOutputConfig) -> SdkResult<&mut AsioSidecarClient> {
        if self.lifecycle_leases == 0 {
            return Err(SdkError::internal(
                "sidecar access outside plugin lifecycle is not allowed",
            ));
        }

        let signature = config_signature(config);
        let needs_restart = self
            .signature
            .as_deref()
            .map(|current| current != signature)
            .unwrap_or(true)
            || self.client.is_none();

        if needs_restart {
            self.drop_current_client();
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
    // Acquire lifecycle lease and prewarm with default config.
    // Session-level config can trigger a signature-based restart.
    let mut manager = lock_manager();
    manager.acquire_lifecycle(&AsioOutputConfig::default())
}

pub(crate) fn with_sidecar<T>(
    config: &AsioOutputConfig,
    f: impl FnOnce(&mut AsioSidecarClient) -> SdkResult<T>,
) -> SdkResult<T> {
    let mut manager = lock_manager();
    let client = manager.ensure_for(config)?;
    f(client)
}
