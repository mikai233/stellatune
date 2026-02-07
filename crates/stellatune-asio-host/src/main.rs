use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use stellatune_asio_proto::{
    read_frame, shm::SharedRingMapped, write_frame, AudioSpec, DeviceCaps, DeviceInfo, Request,
    Response, SampleFormat, PROTOCOL_VERSION,
};

#[cfg(windows)]
use windows::Win32::System::Threading::{
    AvSetMmThreadCharacteristicsW, AvSetMmThreadPriority, GetCurrentProcess, SetPriorityClass,
    AVRT_PRIORITY_HIGH, HIGH_PRIORITY_CLASS,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut r = stdin.lock();
    let mut w = stdout.lock();

    #[cfg(windows)]
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS);
    }

    let mut state: Option<StreamState> = None;

    loop {
        let req: Request = match read_frame(&mut r) {
            Ok(v) => v,
            Err(e) => {
                // EOF / broken pipe => exit.
                if matches!(e, stellatune_asio_proto::ProtoError::Io(ref io) if io.kind() == std::io::ErrorKind::UnexpectedEof)
                {
                    break;
                }
                let _ = write_frame(
                    &mut w,
                    &Response::Err {
                        message: e.to_string(),
                    },
                );
                continue;
            }
        };

        match req {
            Request::Hello { version } => {
                if version != PROTOCOL_VERSION {
                    write_frame(
                        &mut w,
                        &Response::Err {
                            message: format!(
                                "protocol version mismatch: client={version}, host={}",
                                PROTOCOL_VERSION
                            ),
                        },
                    )?;
                } else {
                    write_frame(&mut w, &Response::HelloOk { version })?;
                }
            }
            Request::ListDevices => {
                let devices = list_devices()?;
                write_frame(&mut w, &Response::Devices { devices })?;
            }
            Request::GetDeviceCaps { device_id } => {
                let caps = get_device_caps(&device_id)?;
                write_frame(&mut w, &Response::DeviceCaps { caps })?;
            }
            Request::Open {
                device_id,
                spec,
                buffer_size_frames,
                shared_ring,
            } => {
                state = Some(StreamState::open(
                    &device_id,
                    spec,
                    buffer_size_frames,
                    shared_ring,
                )?);
                write_frame(&mut w, &Response::Ok)?;
            }
            Request::Start => {
                if let Some(s) = state.as_ref() {
                    s.start()?;
                    write_frame(&mut w, &Response::Ok)?;
                } else {
                    write_frame(
                        &mut w,
                        &Response::Err {
                            message: "not opened".to_string(),
                        },
                    )?;
                }
            }
            Request::Stop => {
                let _ = state.take();
                write_frame(&mut w, &Response::Ok)?;
            }
            Request::Close => {
                let _ = state.take();
                write_frame(&mut w, &Response::Ok)?;
                break;
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
struct MmcssGuard(#[allow(dead_code)] windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for MmcssGuard {}

#[cfg(windows)]
fn enable_mmcss_pro_audio() -> Option<MmcssGuard> {
    let mut task_index = 0u32;
    let task = windows::core::HSTRING::from("Pro Audio");
    let handle = unsafe { AvSetMmThreadCharacteristicsW(&task, &mut task_index) }.ok()?;
    let _ = unsafe { AvSetMmThreadPriority(handle, AVRT_PRIORITY_HIGH) };
    Some(MmcssGuard(handle))
}

fn device_id_string(dev: &cpal::Device) -> String {
    dev.id()
        .ok()
        .map(|id| id.to_string())
        .or_else(|| dev.description().ok().map(|d| d.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

fn asio_host() -> Result<cpal::Host, String> {
    #[cfg(all(windows, feature = "asio"))]
    {
        return cpal::host_from_id(cpal::HostId::Asio).map_err(|e| e.to_string());
    }
    #[cfg(not(all(windows, feature = "asio")))]
    {
        Err("ASIO support not built (enable `stellatune-asio-host` feature `asio`)".to_string())
    }
}

fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let host = asio_host()?;
    let mut out = Vec::new();
    let devs = host.output_devices().map_err(|e| e.to_string())?;
    for dev in devs {
        let id = device_id_string(&dev);
        let name = dev
            .description()
            .ok()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "Unknown ASIO Device".to_string());
        out.push(DeviceInfo { id, name });
    }
    Ok(out)
}

fn get_device_caps(device_id: &str) -> Result<DeviceCaps, String> {
    let host = asio_host()?;
    let devs = host.output_devices().map_err(|e| e.to_string())?;
    let dev = devs
        .into_iter()
        .find(|d| device_id_string(d) == device_id)
        .ok_or_else(|| format!("device not found: {device_id}"))?;

    let default_cfg = dev.default_output_config().map_err(|e| e.to_string())?;
    let default_spec = AudioSpec {
        sample_rate: default_cfg.sample_rate(),
        channels: default_cfg.channels(),
    };

    let mut rates = Vec::new();
    let mut chans = Vec::new();
    let mut fmts = Vec::new();

    if let Ok(configs) = dev.supported_output_configs() {
        for cfg in configs {
            let min = cfg.min_sample_rate();
            let max = cfg.max_sample_rate();
            // Enumerate common rates within range (small list, but useful for "match track rate").
            for r in [
                8000u32, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000,
            ] {
                if r >= min && r <= max {
                    rates.push(r);
                }
            }
            rates.push(min);
            rates.push(max);
            rates.push(default_spec.sample_rate);
            chans.push(cfg.channels());
            fmts.push(match cfg.sample_format() {
                cpal::SampleFormat::F32 => SampleFormat::F32,
                cpal::SampleFormat::I16 => SampleFormat::I16,
                cpal::SampleFormat::I32 => SampleFormat::I32,
                cpal::SampleFormat::U16 => SampleFormat::U16,
                _ => continue,
            });
        }
    }

    rates.sort_unstable();
    rates.dedup();
    chans.sort_unstable();
    chans.dedup();
    fmts.sort_unstable_by_key(|f| *f as u8);
    fmts.dedup();

    Ok(DeviceCaps {
        default_spec,
        supported_sample_rates: rates,
        supported_channels: chans,
        supported_formats: fmts,
    })
}

struct StreamState {
    running: Arc<AtomicBool>,
    _stream: cpal::Stream,
}

impl StreamState {
    fn open(
        device_id: &str,
        spec: AudioSpec,
        buffer_size_frames: Option<u32>,
        shared_ring: Option<stellatune_asio_proto::SharedRingFile>,
    ) -> Result<Self, String> {
        let host = asio_host()?;
        let devs = host.output_devices().map_err(|e| e.to_string())?;
        let dev = devs
            .into_iter()
            .find(|d| device_id_string(d) == device_id)
            .ok_or_else(|| format!("device not found: {device_id}"))?;

        let shared_ring = shared_ring.ok_or_else(|| "shared ring not provided".to_string())?;
        let ring_capacity_samples = shared_ring.capacity_samples;
        let ring_path = shared_ring.path;
        let ring = SharedRingMapped::open(std::path::Path::new(&ring_path))
            .map_err(|e| format!("failed to open shared ring: {e}"))?;
        if ring.capacity_samples() != ring_capacity_samples as usize {
            return Err("shared ring capacity mismatch".to_string());
        }
        if ring.channels() != spec.channels {
            return Err("shared ring channel mismatch".to_string());
        }
        ring.reset();

        let running = Arc::new(AtomicBool::new(false));

        let cfg = cpal::StreamConfig {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            buffer_size: match buffer_size_frames {
                Some(n) => cpal::BufferSize::Fixed(n),
                None => cpal::BufferSize::Default,
            },
        };

        // Prefer f32; if unavailable, fall back to i16/i32/u16.
        let supported = dev.supported_output_configs().map_err(|e| e.to_string())?;
        let mut chosen_format = None;
        for cand in [
            cpal::SampleFormat::F32,
            cpal::SampleFormat::I16,
            cpal::SampleFormat::I32,
            cpal::SampleFormat::U16,
        ] {
            if supported.clone().any(|c| c.sample_format() == cand) {
                chosen_format = Some(cand);
                break;
            }
        }
        let chosen_format = chosen_format.unwrap_or(cpal::SampleFormat::F32);

        let err_fn = |e| eprintln!("cpal stream error: {e}");

        let stream = match chosen_format {
            cpal::SampleFormat::F32 => {
                let ring = SharedRingMapped::open(std::path::Path::new(&ring_path))
                    .map_err(|e| format!("failed to open shared ring: {e}"))?;
                let running_cb = Arc::clone(&running);
                #[cfg(windows)]
                let mut mmcss_guard: Option<MmcssGuard> = None;
                dev.build_output_stream(
                    &cfg,
                    move |out: &mut [f32], _| {
                        #[cfg(windows)]
                        if mmcss_guard.is_none() {
                            mmcss_guard = enable_mmcss_pro_audio();
                        }
                        fill_shm_f32(out, &ring, &running_cb)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            }
            cpal::SampleFormat::I16 => {
                let mut tmp = vec![0f32; 0];
                let ring = SharedRingMapped::open(std::path::Path::new(&ring_path))
                    .map_err(|e| format!("failed to open shared ring: {e}"))?;
                let running_cb = Arc::clone(&running);
                #[cfg(windows)]
                let mut mmcss_guard: Option<MmcssGuard> = None;
                dev.build_output_stream(
                    &cfg,
                    move |out: &mut [i16], _| {
                        #[cfg(windows)]
                        if mmcss_guard.is_none() {
                            mmcss_guard = enable_mmcss_pro_audio();
                        }
                        fill_shm_i16(out, &ring, &running_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            }
            cpal::SampleFormat::I32 => {
                let mut tmp = vec![0f32; 0];
                let ring = SharedRingMapped::open(std::path::Path::new(&ring_path))
                    .map_err(|e| format!("failed to open shared ring: {e}"))?;
                let running_cb = Arc::clone(&running);
                #[cfg(windows)]
                let mut mmcss_guard: Option<MmcssGuard> = None;
                dev.build_output_stream(
                    &cfg,
                    move |out: &mut [i32], _| {
                        #[cfg(windows)]
                        if mmcss_guard.is_none() {
                            mmcss_guard = enable_mmcss_pro_audio();
                        }
                        fill_shm_i32(out, &ring, &running_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            }
            cpal::SampleFormat::U16 => {
                let mut tmp = vec![0f32; 0];
                let ring = SharedRingMapped::open(std::path::Path::new(&ring_path))
                    .map_err(|e| format!("failed to open shared ring: {e}"))?;
                let running_cb = Arc::clone(&running);
                #[cfg(windows)]
                let mut mmcss_guard: Option<MmcssGuard> = None;
                dev.build_output_stream(
                    &cfg,
                    move |out: &mut [u16], _| {
                        #[cfg(windows)]
                        if mmcss_guard.is_none() {
                            mmcss_guard = enable_mmcss_pro_audio();
                        }
                        fill_shm_u16(out, &ring, &running_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            }
            other => return Err(format!("unsupported sample format: {other:?}")),
        };

        Ok(Self {
            running,
            _stream: stream,
        })
    }

    fn start(&self) -> Result<(), String> {
        self.running.store(true, Ordering::Release);
        self._stream.play().map_err(|e| e.to_string())
    }
}

fn fill_shm_f32(out: &mut [f32], ring: &SharedRingMapped, running: &Arc<AtomicBool>) {
    if !running.load(Ordering::Acquire) {
        out.fill(0.0);
        return;
    }
    let n = ring.read_samples(out);
    if n < out.len() {
        out[n..].fill(0.0);
    }
}

fn ensure_tmp(tmp: &mut Vec<f32>, len: usize) {
    if tmp.len() < len {
        tmp.resize(len, 0.0);
    }
}

fn fill_shm_i16(
    out: &mut [i16],
    ring: &SharedRingMapped,
    running: &Arc<AtomicBool>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::Acquire) {
        out.fill(0);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = ring.read_samples(&mut tmp[..out.len()]);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        *dst = (v * i16::MAX as f32) as i16;
    }
}

fn fill_shm_i32(
    out: &mut [i32],
    ring: &SharedRingMapped,
    running: &Arc<AtomicBool>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::Acquire) {
        out.fill(0);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = ring.read_samples(&mut tmp[..out.len()]);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        *dst = (v * i32::MAX as f32) as i32;
    }
}

fn fill_shm_u16(
    out: &mut [u16],
    ring: &SharedRingMapped,
    running: &Arc<AtomicBool>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::Acquire) {
        out.fill(0);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = ring.read_samples(&mut tmp[..out.len()]);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        let normalized = (v + 1.0) * 0.5;
        *dst = (normalized * u16::MAX as f32) as u16;
    }
}
