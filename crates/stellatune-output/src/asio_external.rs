use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use stellatune_asio_proto::{
    AudioSpec, PROTOCOL_VERSION, ProtoError, Request, Response, SharedRingFile, read_frame,
    shm::SharedRingMapped, write_frame,
};

use crate::{AudioBackend, AudioDevice, OutputError, OutputSpec, SampleConsumer};

const HOST_ENV: &str = "STELLATUNE_ASIO_HOST";

fn host_exe_name() -> &'static str {
    if cfg!(windows) {
        "stellatune-asio-host.exe"
    } else {
        "stellatune-asio-host"
    }
}

fn find_host_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var(HOST_ENV) {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(mut exe) = std::env::current_exe() {
        exe.pop();
        let candidate = exe.join(host_exe_name());
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(host_exe_name());
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

struct HostClient {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl HostClient {
    fn spawn() -> Result<Self, OutputError> {
        let exe = find_host_exe().ok_or_else(|| OutputError::ConfigMismatch {
            message: format!(
                "ASIO host not found (set {HOST_ENV} or place {} next to the app executable)",
                host_exe_name()
            ),
        })?;
        let mut cmd = Command::new(exe);
        cmd.arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| OutputError::ConfigMismatch {
            message: format!("failed to spawn ASIO host: {e}"),
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| OutputError::ConfigMismatch {
                message: "failed to open ASIO host stdin".to_string(),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| OutputError::ConfigMismatch {
                message: "failed to open ASIO host stdout".to_string(),
            })?;
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    fn hello(&mut self) -> Result<(), OutputError> {
        write_frame(
            &mut self.stdin,
            &Request::Hello {
                version: PROTOCOL_VERSION,
            },
        )
        .map_err(proto_to_output_err)?;
        let resp: Response = read_frame(&mut self.stdout).map_err(proto_to_output_err)?;
        match resp {
            Response::HelloOk { .. } => Ok(()),
            Response::Err { message } => Err(OutputError::ConfigMismatch { message }),
            other => Err(OutputError::ConfigMismatch {
                message: format!("unexpected ASIO host response: {other:?}"),
            }),
        }
    }

    fn request(&mut self, req: Request) -> Result<Response, OutputError> {
        write_frame(&mut self.stdin, &req).map_err(proto_to_output_err)?;
        let resp: Response = read_frame(&mut self.stdout).map_err(proto_to_output_err)?;
        Ok(resp)
    }
}

fn proto_to_output_err(e: ProtoError) -> OutputError {
    OutputError::ConfigMismatch {
        message: format!("ASIO host protocol error: {e}"),
    }
}

pub fn list_asio_devices_via_host() -> Vec<AudioDevice> {
    let Ok(mut c) = HostClient::spawn() else {
        return Vec::new();
    };
    if c.hello().is_err() {
        return Vec::new();
    }
    let Ok(resp) = c.request(Request::ListDevices) else {
        return Vec::new();
    };
    match resp {
        Response::Devices { devices } => devices
            .into_iter()
            .map(|d| AudioDevice {
                backend: AudioBackend::Asio,
                id: d.id,
                name: d.name,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn resolve_device_id(c: &mut HostClient, device_id: Option<String>) -> Result<String, OutputError> {
    let resp = c.request(Request::ListDevices)?;
    let Response::Devices { devices } = resp else {
        return Err(OutputError::ConfigMismatch {
            message: "ASIO host did not return devices".to_string(),
        });
    };
    let mut devices = devices;
    if devices.is_empty() {
        return Err(OutputError::NoDevice);
    }
    if let Some(sel) = device_id {
        if let Some(d) = devices.into_iter().find(|d| d.id == sel) {
            return Ok(d.id);
        }
        return Err(OutputError::NoDevice);
    }
    // Choose a deterministic "default" that matches the UI ordering.
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(devices.into_iter().next().expect("non-empty").id)
}

pub fn output_spec_for_asio_device(device_id: Option<String>) -> Result<OutputSpec, OutputError> {
    let mut c = HostClient::spawn()?;
    c.hello()?;

    let device_id = resolve_device_id(&mut c, device_id)?;

    let resp = c.request(Request::GetDeviceCaps { device_id })?;
    let Response::DeviceCaps { caps } = resp else {
        return Err(OutputError::ConfigMismatch {
            message: "ASIO host did not return device caps".to_string(),
        });
    };
    Ok(OutputSpec {
        sample_rate: caps.default_spec.sample_rate,
        channels: caps.default_spec.channels,
    })
}

pub fn supports_asio_spec(
    device_id: Option<String>,
    spec: OutputSpec,
) -> Result<bool, OutputError> {
    let mut c = HostClient::spawn()?;
    c.hello()?;

    let device_id = resolve_device_id(&mut c, device_id)?;
    let resp = c.request(Request::GetDeviceCaps { device_id })?;
    let Response::DeviceCaps { caps } = resp else {
        return Err(OutputError::ConfigMismatch {
            message: "ASIO host did not return device caps".to_string(),
        });
    };
    Ok(caps.supported_sample_rates.contains(&spec.sample_rate)
        && caps.supported_channels.contains(&spec.channels))
}

pub struct AsioExternalHandle {
    shutdown: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    child: Option<Child>,
    shm_path: Option<PathBuf>,
    spec: OutputSpec,
}

impl AsioExternalHandle {
    pub fn spec(&self) -> OutputSpec {
        self.spec
    }
}

impl Drop for AsioExternalHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(p) = self.shm_path.take() {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn make_temp_shm_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    p.push(format!("stellatune-asio-ring-{pid}-{ns}.bin"));
    p
}

pub fn start_asio_external<C: SampleConsumer, F>(
    device_id: Option<String>,
    mut consumer: C,
    expected_spec: OutputSpec,
    _on_error: F,
) -> Result<AsioExternalHandle, OutputError>
where
    F: Fn(String) + Send + Sync + 'static,
{
    let mut c = HostClient::spawn()?;
    c.hello()?;

    let device_id = resolve_device_id(&mut c, device_id)?;

    // Shared memory ring buffer (~400ms).
    let capacity_frames = ((expected_spec.sample_rate as u64) * 400 / 1000).max(256) as usize;
    let capacity_samples = capacity_frames * expected_spec.channels as usize;
    let shm_path = make_temp_shm_path();
    let ring = SharedRingMapped::create(&shm_path, capacity_samples, expected_spec.channels)
        .map_err(|e| OutputError::ConfigMismatch {
            message: format!("failed to create shared ring: {e}"),
        })?;

    let open = Request::Open {
        device_id,
        spec: AudioSpec {
            sample_rate: expected_spec.sample_rate,
            channels: expected_spec.channels,
        },
        buffer_size_frames: None,
        shared_ring: Some(SharedRingFile {
            path: shm_path.to_string_lossy().to_string(),
            capacity_samples: capacity_samples as u32,
        }),
    };
    match c.request(open)? {
        Response::Ok => {}
        Response::Err { message } => {
            return Err(OutputError::ConfigMismatch { message });
        }
        other => {
            return Err(OutputError::ConfigMismatch {
                message: format!("unexpected ASIO host response: {other:?}"),
            });
        }
    }
    match c.request(Request::Start)? {
        Response::Ok => {}
        Response::Err { message } => return Err(OutputError::ConfigMismatch { message }),
        other => {
            return Err(OutputError::ConfigMismatch {
                message: format!("unexpected ASIO host response: {other:?}"),
            });
        }
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = Arc::clone(&shutdown);
    let mut stdin = c.stdin;
    let mut stdout = c.stdout;
    let child = c.child;

    let thread = thread::Builder::new()
        .name("stellatune-asio-external".to_string())
        .spawn(move || {
            // Batch size: ~10ms of audio.
            let frames_per_chunk = (expected_spec.sample_rate / 100).max(64) as usize;
            let samples_per_chunk = frames_per_chunk * expected_spec.channels as usize;
            let mut buf = vec![0f32; samples_per_chunk];
            let ring = ring;

            while !thread_shutdown.load(Ordering::Relaxed) {
                let mut provided = 0usize;
                for s in &mut buf {
                    if let Some(v) = consumer.pop_sample() {
                        *s = v;
                        provided += 1;
                    } else {
                        *s = 0.0;
                    }
                }
                consumer.on_output(buf.len(), provided);

                let mut offset = 0usize;
                while offset < buf.len() && !thread_shutdown.load(Ordering::Relaxed) {
                    let n = ring.write_samples(&buf[offset..]);
                    if n == 0 {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    offset += n;
                }

                // Avoid busy looping if the consumer is silent / host buffering.
                thread::sleep(Duration::from_millis(1));
            }

            // Best-effort stop/close.
            let _ = write_frame(&mut stdin, &Request::Stop);
            let _ = read_frame::<_, Response>(&mut stdout);
            let _ = write_frame(&mut stdin, &Request::Close);
            let _ = read_frame::<_, Response>(&mut stdout);
        })
        .map_err(|e| OutputError::ConfigMismatch {
            message: format!("failed to spawn ASIO external thread: {e}"),
        })?;

    Ok(AsioExternalHandle {
        shutdown,
        thread: Some(thread),
        child: Some(child),
        shm_path: Some(shm_path),
        spec: expected_spec,
    })
}
