use std::collections::HashMap;
use std::io::{BufReader, BufWriter, ErrorKind};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use stellatune_asio_proto::{
    DeviceCaps, DeviceInfo, PROTOCOL_VERSION, Request, Response, read_frame, write_frame,
};
use stellatune_plugin_sdk::{
    SdkError, SdkResult, StLogLevel, host_log, resolve_runtime_path, sidecar_command,
};

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
        write_frame(&mut self.stdin, &req)
            .map_err(|e| SdkError::Io(format!("ASIO sidecar write_frame failed: {e}")))?;
        let resp: Response = match read_frame(&mut self.stdout) {
            Ok(resp) => resp,
            Err(stellatune_asio_proto::ProtoError::Io(io))
                if io.kind() == ErrorKind::UnexpectedEof =>
            {
                let status_hint = match self.child.try_wait() {
                    Ok(Some(status)) => format!(" sidecar exit status: {status}."),
                    Ok(None) => " sidecar exit status is not observable yet; process may still be running or may be terminating."
                        .to_string(),
                    Err(error) => format!(" sidecar status query failed: {error}."),
                };
                return Err(SdkError::Io(format!(
                    "ASIO sidecar closed the pipe unexpectedly while reading response.{} \
                     Verify `sidecar_path` points to `stellatune-asio-host` and that the sidecar was built with `--features asio`.",
                    status_hint
                )));
            },
            Err(stellatune_asio_proto::ProtoError::Postcard(err)) => {
                return Err(SdkError::msg(format!(
                    "ASIO sidecar protocol decode failed: {err}. \
                     Sidecar stdout may be non-protocol output; check sidecar executable/path."
                )));
            },
            Err(other) => {
                return Err(SdkError::Io(format!(
                    "ASIO sidecar read_frame failed: {other}"
                )));
            },
        };
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

    fn get_device_caps(
        &mut self,
        selection_session_id: &str,
        device_id: &str,
    ) -> SdkResult<DeviceCaps> {
        match self.request(Request::GetDeviceCaps {
            selection_session_id: selection_session_id.to_string(),
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
struct SidecarEntry {
    lease_count: usize,
    client: Option<AsioHostClient>,
}

#[derive(Default)]
struct SidecarManagerState {
    entries: HashMap<String, SidecarEntry>,
}

static SHARED_SIDECAR_MANAGER: OnceLock<Mutex<SidecarManagerState>> = OnceLock::new();
const SIDECAR_SIGNATURE_FIELD_SEP: &str = "\u{1e}";
const SIDECAR_SIGNATURE_ARG_SEP: &str = "\u{1f}";

struct SidecarMetrics {
    asio_sidecar_spawns_total: AtomicU64,
    asio_sidecar_running: AtomicU64,
}

impl SidecarMetrics {
    fn new() -> Self {
        Self {
            asio_sidecar_spawns_total: AtomicU64::new(0),
            asio_sidecar_running: AtomicU64::new(0),
        }
    }

    fn set_running(&self, running: usize) -> u64 {
        let running = running as u64;
        self.asio_sidecar_running.store(running, Ordering::Relaxed);
        running
    }
}

fn sidecar_metrics() -> &'static SidecarMetrics {
    static METRICS: OnceLock<SidecarMetrics> = OnceLock::new();
    METRICS.get_or_init(SidecarMetrics::new)
}

fn sidecar_running_entries(state: &SidecarManagerState) -> usize {
    state
        .entries
        .values()
        .filter(|entry| entry.client.is_some())
        .count()
}

fn sidecar_manager_state() -> &'static Mutex<SidecarManagerState> {
    SHARED_SIDECAR_MANAGER.get_or_init(|| Mutex::new(SidecarManagerState::default()))
}

pub(crate) struct SidecarLease {
    signature: String,
    released: bool,
}

impl SidecarLease {
    pub(crate) fn release(&mut self) -> SdkResult<()> {
        if self.released {
            return Ok(());
        }
        release_sidecar_lease_impl(&self.signature)?;
        self.released = true;
        Ok(())
    }
}

impl Drop for SidecarLease {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

fn sidecar_client_signature(config: &AsioOutputConfig) -> String {
    let path = config.sidecar_path.as_deref().unwrap_or_default();
    let args = config.sidecar_args.join(SIDECAR_SIGNATURE_ARG_SEP);
    format!("path={path}{SIDECAR_SIGNATURE_FIELD_SEP}args={args}")
}

fn sidecar_signature_for_log(signature: &str) -> String {
    let mut path = "";
    let mut args: Vec<&str> = Vec::new();

    for part in signature.split('\u{1e}') {
        if let Some(value) = part.strip_prefix("path=") {
            path = value;
            continue;
        }
        if let Some(value) = part.strip_prefix("args=") {
            if !value.is_empty() {
                args = value.split('\u{1f}').collect();
            }
            continue;
        }
    }

    let args_text = args
        .iter()
        .map(|arg| format!("{arg:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("path={path:?} args=[{args_text}]")
}

pub(crate) fn acquire_sidecar_lease(config: &AsioOutputConfig) -> SdkResult<SidecarLease> {
    let signature = sidecar_client_signature(config);
    let signature_log = sidecar_signature_for_log(&signature);
    {
        let mut guard = sidecar_manager_state()
            .lock()
            .map_err(|_| SdkError::msg("ASIO sidecar manager mutex poisoned"))?;
        let (lease_count, spawned) = {
            let entry = guard.entries.entry(signature.clone()).or_default();
            let mut spawned = false;
            if entry.client.is_none() {
                entry.client = Some(AsioHostClient::spawn(config)?);
                spawned = true;
            }
            entry.lease_count = entry.lease_count.saturating_add(1);
            (entry.lease_count, spawned)
        };
        let metrics = sidecar_metrics();
        let spawns_total = if spawned {
            metrics
                .asio_sidecar_spawns_total
                .fetch_add(1, Ordering::Relaxed)
                + 1
        } else {
            metrics.asio_sidecar_spawns_total.load(Ordering::Relaxed)
        };
        let running = metrics.set_running(sidecar_running_entries(&guard));
        host_log(
            StLogLevel::Debug,
            &format!(
                "asio sidecar lease acquired: signature={} leases={} asio_sidecar_spawns_total={} asio_sidecar_running={}",
                signature_log, lease_count, spawns_total, running
            ),
        );
    }
    Ok(SidecarLease {
        signature,
        released: false,
    })
}

fn release_sidecar_lease_impl(signature: &str) -> SdkResult<()> {
    let signature_log = sidecar_signature_for_log(signature);
    let mut guard = sidecar_manager_state()
        .lock()
        .map_err(|_| SdkError::msg("ASIO sidecar manager mutex poisoned"))?;
    let lease_count = {
        let Some(entry) = guard.entries.get_mut(signature) else {
            return Ok(());
        };

        if entry.lease_count == 0 {
            return Ok(());
        }

        entry.lease_count -= 1;
        entry.lease_count
    };
    let running_before_drop = sidecar_metrics().set_running(sidecar_running_entries(&guard));
    host_log(
        StLogLevel::Debug,
        &format!(
            "asio sidecar lease released: signature={} leases={} asio_sidecar_running={}",
            signature_log, lease_count, running_before_drop
        ),
    );

    if lease_count > 0 {
        return Ok(());
    }

    if let Some(entry) = guard.entries.get_mut(signature)
        && let Some(client) = entry.client.as_mut()
    {
        let _ = client.request_ok(Request::Stop);
    }
    let running_after_stop = sidecar_metrics().set_running(sidecar_running_entries(&guard));
    let spawns_total = sidecar_metrics()
        .asio_sidecar_spawns_total
        .load(Ordering::Relaxed);
    host_log(
        StLogLevel::Debug,
        &format!(
            "asio sidecar lease reached zero (client kept resident): signature={} asio_sidecar_spawns_total={} asio_sidecar_running={}",
            signature_log, spawns_total, running_after_stop
        ),
    );
    Ok(())
}

fn with_sidecar_client<T>(
    config: &AsioOutputConfig,
    mut f: impl FnMut(&mut AsioHostClient) -> SdkResult<T>,
) -> SdkResult<T> {
    let signature = sidecar_client_signature(config);
    let signature_log = sidecar_signature_for_log(&signature);
    let mut guard = sidecar_manager_state()
        .lock()
        .map_err(|_| SdkError::msg("ASIO sidecar manager mutex poisoned"))?;
    let mut initialized = false;
    let mut lease_count: usize;
    {
        let entry = guard.entries.entry(signature.clone()).or_default();
        if entry.client.is_none() {
            entry.client = Some(AsioHostClient::spawn(config)?);
            initialized = true;
        }
        lease_count = entry.lease_count;
    }
    if initialized {
        let metrics = sidecar_metrics();
        let spawns_total = metrics
            .asio_sidecar_spawns_total
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        let running = metrics.set_running(sidecar_running_entries(&guard));
        host_log(
            StLogLevel::Debug,
            &format!(
                "asio sidecar client initialized: signature={} leases={} asio_sidecar_spawns_total={} asio_sidecar_running={}",
                signature_log, lease_count, spawns_total, running
            ),
        );
    }

    let first_result = {
        let entry = guard
            .entries
            .get_mut(&signature)
            .ok_or_else(|| SdkError::msg("missing ASIO sidecar entry"))?;
        let client = entry
            .client
            .as_mut()
            .ok_or_else(|| SdkError::msg("failed to initialize ASIO sidecar client"))?;
        f(client)
    };
    match first_result {
        Ok(v) => Ok(v),
        Err(e) if is_retryable_pipe_error(&e) => {
            {
                let entry = guard
                    .entries
                    .get_mut(&signature)
                    .ok_or_else(|| SdkError::msg("missing ASIO sidecar entry"))?;
                entry.client = Some(AsioHostClient::spawn(config)?);
                lease_count = entry.lease_count;
            }
            let metrics = sidecar_metrics();
            let spawns_total = metrics
                .asio_sidecar_spawns_total
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            let running = metrics.set_running(sidecar_running_entries(&guard));
            host_log(
                StLogLevel::Warn,
                &format!(
                    "asio sidecar client reinitialized after pipe error: signature={} leases={} asio_sidecar_spawns_total={} asio_sidecar_running={}",
                    signature_log, lease_count, spawns_total, running
                ),
            );
            let entry = guard
                .entries
                .get_mut(&signature)
                .ok_or_else(|| SdkError::msg("missing ASIO sidecar entry"))?;
            let client = entry
                .client
                .as_mut()
                .ok_or_else(|| SdkError::msg("failed to reinitialize ASIO sidecar client"))?;
            f(client)
        },
        Err(e) => Err(e),
    }
}

pub(crate) fn sidecar_request_ok(config: &AsioOutputConfig, req: Request) -> SdkResult<()> {
    with_sidecar_client(config, |client| client.request_ok(req.clone()))
}

pub(crate) fn prewarm_sidecar(config: &AsioOutputConfig) -> SdkResult<()> {
    with_sidecar_client(config, |_| Ok(()))
}

pub(crate) fn sidecar_list_devices(config: &AsioOutputConfig) -> SdkResult<Vec<DeviceInfo>> {
    with_sidecar_client(config, |client| client.list_devices())
}

pub(crate) fn sidecar_get_device_caps(
    config: &AsioOutputConfig,
    selection_session_id: &str,
    device_id: &str,
) -> SdkResult<DeviceCaps> {
    with_sidecar_client(config, |client| {
        client.get_device_caps(selection_session_id, device_id)
    })
}

pub(crate) fn shutdown_all_sidecars() -> SdkResult<()> {
    let mut guard = sidecar_manager_state()
        .lock()
        .map_err(|_| SdkError::msg("ASIO sidecar manager mutex poisoned"))?;
    let entries = std::mem::take(&mut guard.entries);
    let entries_total = entries.len();
    let spawns_total = sidecar_metrics()
        .asio_sidecar_spawns_total
        .load(Ordering::Relaxed);
    sidecar_metrics().set_running(0);
    drop(guard);

    for (signature, mut entry) in entries {
        let signature_log = sidecar_signature_for_log(&signature);
        if let Some(client) = entry.client.as_mut() {
            let _ = client.request_ok(Request::Stop);
            let _ = client.request_ok(Request::Close);
        }
        host_log(
            StLogLevel::Debug,
            &format!("asio sidecar shutdown entry: signature={signature_log}"),
        );
    }

    host_log(
        StLogLevel::Debug,
        &format!(
            "asio sidecar shutdown complete: entries={} asio_sidecar_spawns_total={} asio_sidecar_running=0",
            entries_total, spawns_total
        ),
    );
    Ok(())
}

fn is_retryable_pipe_error(err: &SdkError) -> bool {
    let SdkError::Io(msg) = err else {
        return false;
    };
    let normalized = msg.to_ascii_lowercase();
    normalized.contains("os error 232")
        || normalized.contains("os error 109")
        || normalized.contains("broken pipe")
        || normalized.contains("failed to fill whole buffer")
        || normalized.contains("unexpected eof")
        || normalized.contains("connection reset")
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

            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                if !cfg!(debug_assertions) {
                    cmd.creation_flags(CREATE_NO_WINDOW);
                }
            }

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
