//! Separate-process protocol/PCM test double; never included in plugin packages.
use std::{
    io::{stdin, stdout},
    path::Path,
    time::{Duration, Instant},
};
use stellatune_asio_proto::{
    AudioSpec, DeviceCaps, DeviceInfo, Request, Response, SampleFormat, read_frame,
    shared_ring::SharedByteRingMapped, write_frame,
};

fn caps() -> DeviceCaps {
    DeviceCaps {
        default_spec: AudioSpec {
            sample_rate: 48000,
            channels: 2,
        },
        supported_sample_rates: vec![8000, 48000, 192000],
        supported_channels: vec![2],
        supported_formats: vec![SampleFormat::F32],
    }
}
fn main() {
    let mut input = stdin().lock();
    let mut output = stdout().lock();
    let mut mapping = std::env::var_os("STELLATUNE_ASIO_PCM_MAPPING")
        .map(|path| SharedByteRingMapped::open(Path::new(&path)).unwrap());
    let mut running = false;
    let mut queued = 0u64;
    let mut consumed = 0u64;
    let mut channels = 2usize;
    let mut rate = 48000u32;
    let mut mode = String::new();
    let mut last = Instant::now();
    while let Ok(request) = read_frame::<_, Request>(&mut input) {
        if let Some(ring) = &mut mapping {
            while ring.occupied_len() >= 4 {
                let mut len = [0; 4];
                ring.read_bytes(&mut len);
                let mut data = vec![0; u32::from_le_bytes(len) as usize];
                assert_eq!(ring.read_bytes(&mut data), data.len());
                assert_eq!(data.len() % (channels * 4), 0);
                // Audio pattern checks verify bytes, interleaving and wraparound.
                for frame in data.chunks_exact(channels * 4) {
                    assert_eq!(&frame[..4], &0.25f32.to_le_bytes());
                    assert_eq!(&frame[4..8], &(-0.25f32).to_le_bytes());
                }
                queued += (data.len() / (channels * 4)) as u64;
            }
        }
        let now = Instant::now();
        if running {
            let frames = ((now - last).as_secs_f64() * f64::from(rate)) as u64;
            let n = frames.min(queued);
            queued -= n;
            consumed += n;
        }
        last = now;
        let closing = matches!(request, Request::Close);
        let response = match request {
            Request::Hello { version } => Response::HelloOk { version },
            Request::ListDevices => Response::Devices {
                devices: ["test", "crash", "hang", "fail-open"]
                    .into_iter()
                    .map(|id| DeviceInfo {
                        id: id.into(),
                        name: id.into(),
                        selection_session_id: "fresh-session".into(),
                    })
                    .collect(),
            },
            Request::GetDeviceCaps { .. } => Response::DeviceCaps { caps: caps() },
            Request::PrepareDeviceSwitch {
                selection_session_id,
                ..
            } => {
                assert_eq!(selection_session_id, "fresh-session");
                Response::PreparedDeviceSwitch {
                    prepared_switch_id: 1,
                    caps: caps(),
                }
            },
            Request::Open {
                spec,
                device_id,
                prepared_switch_id,
                ..
            } => {
                assert_eq!(prepared_switch_id, 1);
                rate = spec.sample_rate;
                channels = usize::from(spec.channels);
                mode = device_id;
                if mode == "fail-open" {
                    Response::Err {
                        message: "controlled open failure".into(),
                    }
                } else {
                    Response::Opened {
                        buffer_size_frames: 16,
                    }
                }
            },
            Request::Start => {
                running = true;
                Response::Ok
            },
            Request::Pause => {
                running = false;
                Response::Ok
            },
            Request::Reset => {
                if let Some(ring) = &mapping {
                    ring.discard_all();
                }
                queued = 0;
                Response::Ok
            },
            Request::QueryStatus => {
                if mode == "crash" {
                    std::process::exit(17);
                }
                if mode == "hang" {
                    std::thread::sleep(Duration::from_secs(60));
                }
                Response::Status {
                    consumed_frames: consumed,
                    queued_samples: (queued * channels as u64) as u32,
                    running,
                }
            },
            Request::Close | Request::Stop => Response::Ok,
            Request::WriteSamples { .. } => panic!("PCM must not travel over control RPC"),
        };
        if write_frame(&mut output, &response).is_err() || closing {
            break;
        }
    }
}
