use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};

use stellatune_asio_proto::{
    DeviceCaps, DeviceInfo, PROTOCOL_VERSION, Request, Response, read_frame, write_frame,
};
use stellatune_plugin_sdk::{SdkError, SdkResult, resolve_runtime_path, sidecar_command};

use crate::config::AsioOutputConfig;

struct AsioHostClient {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Drop for AsioHostClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl AsioHostClient {
    fn spawn(config: &AsioOutputConfig) -> SdkResult<Self> {
        let mut cmd = build_sidecar_command(config)?;
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        cmd.args(&config.sidecar_args);

        let mut child = cmd.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| SdkError::msg("failed to capture ASIO sidecar stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SdkError::msg("failed to capture ASIO sidecar stdout"))?;

        let mut client = Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
        };

        match client.request(Request::Hello {
            version: PROTOCOL_VERSION,
        })? {
            Response::HelloOk { version } if version == PROTOCOL_VERSION => Ok(client),
            Response::HelloOk { version } => Err(SdkError::msg(format!(
                "ASIO sidecar protocol mismatch: expected {}, got {}",
                PROTOCOL_VERSION, version
            ))),
            other => Err(SdkError::msg(format!(
                "unexpected hello response: {other:?}"
            ))),
        }
    }

    fn request(&mut self, req: Request) -> SdkResult<Response> {
        write_frame(&mut self.stdin, &req).map_err(|e| SdkError::Io(e.to_string()))?;
        let resp: Response =
            read_frame(&mut self.stdout).map_err(|e| SdkError::Io(e.to_string()))?;
        if let Response::Err { message } = resp {
            return Err(SdkError::msg(message));
        }
        Ok(resp)
    }

    fn request_ok(&mut self, req: Request) -> SdkResult<()> {
        match self.request(req)? {
            Response::Ok => Ok(()),
            other => Err(SdkError::msg(format!(
                "unexpected response (expected Ok): {other:?}"
            ))),
        }
    }

    fn list_devices(&mut self) -> SdkResult<Vec<DeviceInfo>> {
        match self.request(Request::ListDevices)? {
            Response::Devices { devices } => Ok(devices),
            other => Err(SdkError::msg(format!(
                "unexpected response to ListDevices: {other:?}"
            ))),
        }
    }

    fn get_device_caps(&mut self, device_id: &str) -> SdkResult<DeviceCaps> {
        match self.request(Request::GetDeviceCaps {
            device_id: device_id.to_string(),
        })? {
            Response::DeviceCaps { caps } => Ok(caps),
            other => Err(SdkError::msg(format!(
                "unexpected response to GetDeviceCaps: {other:?}"
            ))),
        }
    }
}

#[derive(Default)]
struct SharedSidecarClientState {
    signature: Option<String>,
    client: Option<AsioHostClient>,
}

static SHARED_SIDECAR_CLIENT: OnceLock<Mutex<SharedSidecarClientState>> = OnceLock::new();

fn shared_sidecar_client_state() -> &'static Mutex<SharedSidecarClientState> {
    SHARED_SIDECAR_CLIENT.get_or_init(|| Mutex::new(SharedSidecarClientState::default()))
}

fn sidecar_client_signature(config: &AsioOutputConfig) -> String {
    let path = config.sidecar_path.as_deref().unwrap_or_default();
    let args = config.sidecar_args.join("\u{1f}");
    format!("path={path}\u{1e}args={args}")
}

pub(crate) fn ensure_shared_sidecar_client(config: &AsioOutputConfig) -> SdkResult<()> {
    with_shared_sidecar_client(config, |_| Ok(()))
}

fn with_shared_sidecar_client<T>(
    config: &AsioOutputConfig,
    mut f: impl FnMut(&mut AsioHostClient) -> SdkResult<T>,
) -> SdkResult<T> {
    let mut guard = shared_sidecar_client_state()
        .lock()
        .map_err(|_| SdkError::msg("ASIO shared sidecar mutex poisoned"))?;
    let signature = sidecar_client_signature(config);
    if guard.signature.as_deref() != Some(signature.as_str()) {
        guard.client = None;
        guard.signature = Some(signature);
    }
    if guard.client.is_none() {
        guard.client = Some(AsioHostClient::spawn(config)?);
    }
    let first_result = {
        let client = guard
            .client
            .as_mut()
            .ok_or_else(|| SdkError::msg("failed to initialize shared ASIO sidecar client"))?;
        f(client)
    };
    match first_result {
        Ok(v) => Ok(v),
        Err(e) if is_retryable_pipe_error(&e) => {
            // Sidecar might have exited unexpectedly; recreate once and retry.
            guard.client = Some(AsioHostClient::spawn(config)?);
            let client = guard.client.as_mut().ok_or_else(|| {
                SdkError::msg("failed to reinitialize shared ASIO sidecar client")
            })?;
            f(client)
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn sidecar_request_ok(config: &AsioOutputConfig, req: Request) -> SdkResult<()> {
    with_shared_sidecar_client(config, |client| client.request_ok(req.clone()))
}

pub(crate) fn sidecar_list_devices(config: &AsioOutputConfig) -> SdkResult<Vec<DeviceInfo>> {
    with_shared_sidecar_client(config, |client| client.list_devices())
}

pub(crate) fn sidecar_get_device_caps(
    config: &AsioOutputConfig,
    device_id: &str,
) -> SdkResult<DeviceCaps> {
    with_shared_sidecar_client(config, |client| client.get_device_caps(device_id))
}

fn is_retryable_pipe_error(err: &SdkError) -> bool {
    let SdkError::Io(msg) = err else {
        return false;
    };
    msg.contains("os error 232")
        || msg.contains("broken pipe")
        || msg.contains("管道正在被关闭")
        || msg.contains("管道已结束")
}

pub(crate) fn ensure_windows() -> SdkResult<()> {
    if cfg!(windows) {
        Ok(())
    } else {
        Err(SdkError::msg(
            "ASIO output sink is only supported on Windows",
        ))
    }
}

fn default_sidecar_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["stellatune-asio-host.exe", "bin/stellatune-asio-host.exe"]
    } else {
        &["stellatune-asio-host", "bin/stellatune-asio-host"]
    }
}

fn build_sidecar_command(config: &AsioOutputConfig) -> SdkResult<Command> {
    if let Some(raw) = config.sidecar_path.as_deref() {
        let path = raw.trim();
        if path.is_empty() {
            return Err(SdkError::invalid_arg("sidecar_path is empty"));
        }
        if Path::new(path).is_absolute() {
            let mut cmd = Command::new(path);
            if let Some(root) = resolve_runtime_path(".") {
                cmd.current_dir(root);
            }
            return Ok(cmd);
        }
        return sidecar_command(path).map_err(SdkError::from);
    }

    for candidate in default_sidecar_candidates() {
        if let Some(path) = resolve_runtime_path(candidate)
            && path.exists()
        {
            return sidecar_command(candidate).map_err(SdkError::from);
        }
    }

    Err(SdkError::msg(format!(
        "ASIO sidecar not found under runtime root; tried: {}",
        default_sidecar_candidates().join(", ")
    )))
}
