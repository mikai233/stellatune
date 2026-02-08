use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use stellatune_asio_proto::shm::SharedRingMapped;
use stellatune_asio_proto::{
    AudioSpec, DeviceCaps, DeviceInfo, PROTOCOL_VERSION, Request, Response, SharedRingFile,
    read_frame, write_frame,
};
use stellatune_plugin_sdk::{
    OutputSink, OutputSinkDescriptor, ST_OUTPUT_NEGOTIATE_CHANGED_CH,
    ST_OUTPUT_NEGOTIATE_CHANGED_SR, ST_OUTPUT_NEGOTIATE_EXACT, SdkError, SdkResult, StAudioSpec,
    StLogLevel, StOutputSinkNegotiatedSpecV1, compose_get_interface, export_output_sinks_interface,
    export_plugin, host_log, resolve_runtime_path, sidecar_command,
};

const CONFIG_SCHEMA_JSON: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "sidecar_path": { "type": "string" },
    "sidecar_args": {
      "type": "array",
      "items": { "type": "string" },
      "default": []
    },
    "buffer_size_frames": { "type": ["integer", "null"], "minimum": 16 },
    "ring_capacity_ms": { "type": "integer", "minimum": 20, "default": 250 },
    "preferred_chunk_frames": { "type": "integer", "minimum": 0, "default": 256 },
    "flush_timeout_ms": { "type": "integer", "minimum": 1, "default": 400 }
  }
}"#;

const FLUSH_POLL_INTERVAL_MS: u64 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AsioOutputConfig {
    pub sidecar_path: Option<String>,
    pub sidecar_args: Vec<String>,
    pub buffer_size_frames: Option<u32>,
    pub ring_capacity_ms: u32,
    pub preferred_chunk_frames: u32,
    pub flush_timeout_ms: u64,
}

impl Default for AsioOutputConfig {
    fn default() -> Self {
        Self {
            sidecar_path: None,
            sidecar_args: Vec::new(),
            buffer_size_frames: None,
            ring_capacity_ms: 250,
            preferred_chunk_frames: 256,
            flush_timeout_ms: 400,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AsioOutputTarget {
    pub id: String,
    pub name: Option<String>,
}

pub struct AsioOutputSink {
    client: AsioHostClient,
    ring: SharedRingMapped,
    channels: u16,
    flush_timeout_ms: u64,
    ring_path: PathBuf,
}

impl Drop for AsioOutputSink {
    fn drop(&mut self) {
        let _ = self.client.request_ok(Request::Stop);
        let _ = self.client.request_ok(Request::Close);
        let _ = std::fs::remove_file(&self.ring_path);
    }
}

impl OutputSink for AsioOutputSink {
    fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> SdkResult<u32> {
        if channels == 0 {
            return Err(SdkError::invalid_arg("channels must be > 0"));
        }
        if channels != self.channels {
            return Err(SdkError::invalid_arg(format!(
                "channel mismatch: got {channels}, expected {}",
                self.channels
            )));
        }
        let channels_usize = channels as usize;
        if !samples.len().is_multiple_of(channels_usize) {
            return Err(SdkError::invalid_arg("samples not aligned to channels"));
        }

        let accepted_samples = self.ring.write_samples(samples);
        Ok((accepted_samples / channels_usize) as u32)
    }

    fn flush(&mut self) -> SdkResult<()> {
        let timeout = Duration::from_millis(self.flush_timeout_ms.max(1));
        let start = Instant::now();
        while self.ring.available_to_read() > 0 {
            if start.elapsed() >= timeout {
                host_log(
                    StLogLevel::Warn,
                    &format!(
                        "asio sink flush timeout after {}ms (pending_samples={})",
                        self.flush_timeout_ms,
                        self.ring.available_to_read()
                    ),
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(FLUSH_POLL_INTERVAL_MS));
        }
        Ok(())
    }
}

impl OutputSinkDescriptor for AsioOutputSink {
    type Config = AsioOutputConfig;
    type Target = AsioOutputTarget;

    const TYPE_ID: &'static str = "asio";
    const DISPLAY_NAME: &'static str = "ASIO (Sidecar)";
    const CONFIG_SCHEMA_JSON: &'static str = CONFIG_SCHEMA_JSON;

    fn default_config() -> Self::Config {
        AsioOutputConfig::default()
    }

    fn list_targets(config: &Self::Config) -> SdkResult<Vec<Self::Target>> {
        ensure_windows()?;
        let mut client = AsioHostClient::spawn(config)?;
        let devices = client.list_devices()?;
        let _ = client.request_ok(Request::Close);

        Ok(devices
            .into_iter()
            .map(|d| AsioOutputTarget {
                id: d.id,
                name: Some(d.name),
            })
            .collect())
    }

    fn negotiate_spec(
        desired_spec: StAudioSpec,
        config: &Self::Config,
        target: &Self::Target,
    ) -> SdkResult<StOutputSinkNegotiatedSpecV1> {
        ensure_windows()?;
        let mut client = AsioHostClient::spawn(config)?;
        let caps = client.get_device_caps(&target.id)?;
        let _ = client.request_ok(Request::Close);

        let desired_sr = desired_spec.sample_rate.max(1);
        let desired_ch = desired_spec.channels.max(1);

        let sample_rate = choose_sample_rate(desired_sr, &caps);
        let channels = choose_channels(desired_ch, &caps);

        let mut flags = 0u32;
        if sample_rate != desired_sr {
            flags |= ST_OUTPUT_NEGOTIATE_CHANGED_SR;
        }
        if channels != desired_ch {
            flags |= ST_OUTPUT_NEGOTIATE_CHANGED_CH;
        }
        if flags == 0 {
            flags |= ST_OUTPUT_NEGOTIATE_EXACT;
        }

        Ok(StOutputSinkNegotiatedSpecV1 {
            spec: StAudioSpec {
                sample_rate,
                channels,
                reserved: 0,
            },
            preferred_chunk_frames: config.preferred_chunk_frames,
            flags,
            reserved: 0,
        })
    }

    fn open(spec: StAudioSpec, config: &Self::Config, target: &Self::Target) -> SdkResult<Self> {
        ensure_windows()?;

        let spec = AudioSpec {
            sample_rate: spec.sample_rate.max(1),
            channels: spec.channels.max(1),
        };

        let (ring, ring_desc, ring_path) = create_ring(config.ring_capacity_ms, &spec)?;

        let mut client = match AsioHostClient::spawn(config) {
            Ok(client) => client,
            Err(e) => {
                let _ = std::fs::remove_file(&ring_path);
                return Err(e);
            }
        };

        let open_result = client.request_ok(Request::Open {
            device_id: target.id.clone(),
            spec: spec.clone(),
            buffer_size_frames: config.buffer_size_frames,
            shared_ring: Some(ring_desc),
        });
        if let Err(e) = open_result {
            let _ = client.request_ok(Request::Close);
            let _ = std::fs::remove_file(&ring_path);
            return Err(e);
        }

        if let Err(e) = client.request_ok(Request::Start) {
            let _ = client.request_ok(Request::Close);
            let _ = std::fs::remove_file(&ring_path);
            return Err(e);
        }

        Ok(Self {
            client,
            ring,
            channels: spec.channels,
            flush_timeout_ms: config.flush_timeout_ms.max(1),
            ring_path,
        })
    }
}

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

fn ensure_windows() -> SdkResult<()> {
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

fn choose_sample_rate(desired: u32, caps: &DeviceCaps) -> u32 {
    choose_nearest_u32(
        desired.max(1),
        &caps.supported_sample_rates,
        caps.default_spec.sample_rate.max(1),
    )
}

fn choose_channels(desired: u16, caps: &DeviceCaps) -> u16 {
    choose_nearest_u16(
        desired.max(1),
        &caps.supported_channels,
        caps.default_spec.channels.max(1),
    )
}

fn choose_nearest_u32(desired: u32, supported: &[u32], fallback: u32) -> u32 {
    if supported.is_empty() {
        return fallback.max(1);
    }
    if supported.contains(&desired) {
        return desired;
    }

    let mut best = supported[0].max(1);
    let mut best_diff = desired.abs_diff(best);
    for &candidate in supported.iter().skip(1) {
        let candidate = candidate.max(1);
        let diff = desired.abs_diff(candidate);
        if diff < best_diff {
            best = candidate;
            best_diff = diff;
        }
    }
    best
}

fn choose_nearest_u16(desired: u16, supported: &[u16], fallback: u16) -> u16 {
    if supported.is_empty() {
        return fallback.max(1);
    }
    if supported.contains(&desired) {
        return desired;
    }

    let desired_u32 = desired as u32;
    let mut best = supported[0].max(1);
    let mut best_diff = desired_u32.abs_diff(best as u32);
    for &candidate in supported.iter().skip(1) {
        let candidate = candidate.max(1);
        let diff = desired_u32.abs_diff(candidate as u32);
        if diff < best_diff {
            best = candidate;
            best_diff = diff;
        }
    }
    best
}

fn create_ring(
    ring_capacity_ms: u32,
    spec: &AudioSpec,
) -> SdkResult<(SharedRingMapped, SharedRingFile, PathBuf)> {
    let ring_capacity_ms = ring_capacity_ms.max(20);

    let capacity_samples_u64 = (spec.sample_rate as u64)
        .saturating_mul(spec.channels as u64)
        .saturating_mul(ring_capacity_ms as u64)
        / 1000;
    let min_samples = (spec.channels as u64).saturating_mul(512);
    let capacity_samples = capacity_samples_u64.max(min_samples).min(u32::MAX as u64) as u32;

    let base_dir = resolve_runtime_path(".asio")
        .unwrap_or_else(|| std::env::temp_dir().join("stellatune-asio"));
    std::fs::create_dir_all(&base_dir)?;

    let pid = std::process::id();
    let mut created = None;
    for attempt in 0..16u32 {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0);
        let path = base_dir.join(format!("ring-{pid}-{ts}-{attempt}.shm"));

        match SharedRingMapped::create(&path, capacity_samples as usize, spec.channels) {
            Ok(map) => {
                created = Some((path, map));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(SdkError::Io(format!("failed to create shared ring: {e}"))),
        }
    }

    let Some((path, ring)) = created else {
        return Err(SdkError::msg(
            "failed to create unique ASIO shared ring file",
        ));
    };

    let desc = SharedRingFile {
        path: path.to_string_lossy().to_string(),
        capacity_samples,
    };

    Ok((ring, desc, path))
}

export_output_sinks_interface! {
    sinks: [
        asio => AsioOutputSink,
    ],
}

compose_get_interface! {
    fn __st_get_interface;
    __st_output_sinks_get_interface,
}

export_plugin! {
    id: "dev.stellatune.output.asio",
    name: "ASIO Output Sink",
    version: (0, 1, 0),
    decoders: [],
    dsps: [],
    get_interface: __st_get_interface,
}
