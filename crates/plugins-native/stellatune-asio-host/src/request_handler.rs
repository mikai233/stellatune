use std::io::Write;
use std::thread;
use std::time::Duration;

use stellatune_asio_proto::{PROTOCOL_VERSION, ProtoError, Request, Response, write_frame};

use crate::device::{
    OPEN_RECONFIGURE_SETTLE_MS, get_device_caps, list_devices, validate_selection_session,
};
use crate::state::RuntimeState;
use crate::stream::StreamState;

pub(crate) fn dispatch_request<W: Write>(
    request: Request,
    state: &mut RuntimeState,
    writer: &mut W,
) -> Result<bool, ProtoError> {
    match request {
        Request::Hello { version } => {
            handle_hello(version, writer)?;
        },
        Request::ListDevices => {
            handle_list_devices(state, writer)?;
        },
        Request::GetDeviceCaps {
            selection_session_id,
            device_id,
        } => {
            handle_get_device_caps(state, writer, &selection_session_id, &device_id)?;
        },
        Request::Open {
            selection_session_id,
            device_id,
            spec,
            buffer_size_frames,
            queue_capacity_ms,
        } => {
            handle_open(
                state,
                writer,
                selection_session_id,
                device_id,
                spec,
                buffer_size_frames,
                queue_capacity_ms,
            )?;
        },
        Request::Start => {
            handle_start(state, writer)?;
        },
        Request::Stop => {
            handle_stop(state, writer)?;
        },
        Request::Reset => {
            handle_reset(state, writer)?;
        },
        Request::Close => {
            handle_close(state, writer)?;
            return Ok(false);
        },
        Request::WriteSamples { interleaved_f32le } => {
            handle_write_samples(state, writer, &interleaved_f32le)?;
        },
        Request::QueryStatus => {
            handle_query_status(state, writer)?;
        },
    }

    Ok(true)
}

fn handle_hello<W: Write>(version: u32, writer: &mut W) -> Result<(), ProtoError> {
    if version != PROTOCOL_VERSION {
        write_frame(
            writer,
            &Response::Err {
                message: format!(
                    "protocol version mismatch: client={version}, host={}",
                    PROTOCOL_VERSION
                ),
            },
        )
    } else {
        write_frame(writer, &Response::HelloOk { version })
    }
}

fn handle_list_devices<W: Write>(
    state: &mut RuntimeState,
    writer: &mut W,
) -> Result<(), ProtoError> {
    match list_devices(state) {
        Ok(devices) => {
            let preview = devices
                .iter()
                .take(6)
                .map(|device| {
                    format!(
                        "{} ({}) session={}",
                        device.id, device.name, device.selection_session_id
                    )
                })
                .collect::<Vec<_>>()
                .join(" || ");
            eprintln!(
                "asio host request ListDevices ok: count={} active_device={:?} stream_active={} preview=[{}]",
                devices.len(),
                state.active_device_id,
                state.stream.is_some(),
                preview
            );
            write_frame(writer, &Response::Devices { devices })
        },
        Err(error) => {
            eprintln!("asio host request ListDevices err: {error}");
            write_frame(
                writer,
                &Response::Err {
                    message: format!("ListDevices failed: {error}"),
                },
            )
        },
    }
}

fn handle_get_device_caps<W: Write>(
    state: &mut RuntimeState,
    writer: &mut W,
    selection_session_id: &str,
    device_id: &str,
) -> Result<(), ProtoError> {
    match get_device_caps(state, selection_session_id, device_id) {
        Ok(caps) => {
            eprintln!(
                "asio host request GetDeviceCaps ok: device={} session={} default={}Hz/{}ch rates={} chans={} fmts={}",
                device_id,
                selection_session_id,
                caps.default_spec.sample_rate,
                caps.default_spec.channels,
                caps.supported_sample_rates.len(),
                caps.supported_channels.len(),
                caps.supported_formats.len()
            );
            write_frame(writer, &Response::DeviceCaps { caps })
        },
        Err(error) => {
            eprintln!(
                "asio host request GetDeviceCaps err: device={} session={} err={}",
                device_id, selection_session_id, error
            );
            write_frame(
                writer,
                &Response::Err {
                    message: format!("GetDeviceCaps failed for device `{device_id}`: {error}"),
                },
            )
        },
    }
}

fn handle_open<W: Write>(
    state: &mut RuntimeState,
    writer: &mut W,
    selection_session_id: String,
    device_id: String,
    spec: stellatune_asio_proto::AudioSpec,
    buffer_size_frames: Option<u32>,
    queue_capacity_ms: Option<u32>,
) -> Result<(), ProtoError> {
    eprintln!(
        "asio host request Open begin: device={} session={} spec={}Hz/{}ch buffer_size_frames={:?} queue_capacity_ms={:?}",
        device_id,
        selection_session_id,
        spec.sample_rate,
        spec.channels,
        buffer_size_frames,
        queue_capacity_ms
    );

    let requested_sample_rate = spec.sample_rate;
    let requested_channels = spec.channels;

    match validate_selection_session(state, &selection_session_id, &device_id) {
        Ok(()) => {
            // ASIO backends generally allow one active stream per device.
            // Drop current stream before opening the next one to avoid
            // transient double-open races during rapid track switches.
            if state.stream.take().is_some() {
                eprintln!(
                    "asio host request Open dropping previous stream: active_device={:?}",
                    state.active_device_id
                );
                state.active_device_id = None;
                thread::sleep(Duration::from_millis(OPEN_RECONFIGURE_SETTLE_MS));
            }
            match StreamState::open(&device_id, spec, buffer_size_frames, queue_capacity_ms) {
                Ok(next_state) => {
                    eprintln!(
                        "asio host request Open ok: device={} session={} spec={}Hz/{}ch",
                        device_id, selection_session_id, requested_sample_rate, requested_channels
                    );
                    state.stream = Some(next_state);
                    state.active_device_id = Some(device_id.clone());
                    write_frame(writer, &Response::Ok)
                },
                Err(error) => {
                    eprintln!(
                        "asio host request Open err: device={} session={} err={}",
                        device_id, selection_session_id, error
                    );
                    write_frame(
                        writer,
                        &Response::Err {
                            message: format!(
                                "Open failed for device `{device_id}` ({}/{}ch): {error}",
                                requested_sample_rate, requested_channels
                            ),
                        },
                    )
                },
            }
        },
        Err(error) => {
            eprintln!(
                "asio host request Open rejected: device={} session={} err={}",
                device_id, selection_session_id, error
            );
            write_frame(
                writer,
                &Response::Err {
                    message: format!("Open rejected for device `{device_id}`: {error}"),
                },
            )
        },
    }
}

fn handle_start<W: Write>(state: &RuntimeState, writer: &mut W) -> Result<(), ProtoError> {
    if let Some(stream) = state.stream.as_ref() {
        match stream.start() {
            Ok(()) => write_frame(writer, &Response::Ok),
            Err(error) => write_frame(
                writer,
                &Response::Err {
                    message: format!("Start failed: {error}"),
                },
            ),
        }
    } else {
        write_frame(
            writer,
            &Response::Err {
                message: "not opened".to_string(),
            },
        )
    }
}

fn handle_stop<W: Write>(state: &mut RuntimeState, writer: &mut W) -> Result<(), ProtoError> {
    let _ = state.stream.take();
    state.active_device_id = None;
    eprintln!("asio host request Stop: stream_active=false");
    write_frame(writer, &Response::Ok)
}

fn handle_reset<W: Write>(state: &mut RuntimeState, writer: &mut W) -> Result<(), ProtoError> {
    if let Some(stream) = state.stream.as_ref() {
        stream.reset();
        eprintln!("asio host request Reset: queue_cleared=true");
        write_frame(writer, &Response::Ok)
    } else {
        write_frame(
            writer,
            &Response::Err {
                message: "not opened".to_string(),
            },
        )
    }
}

fn handle_write_samples<W: Write>(
    state: &RuntimeState,
    writer: &mut W,
    interleaved_f32le: &[u8],
) -> Result<(), ProtoError> {
    if let Some(stream) = state.stream.as_ref() {
        match stream.write_interleaved_f32le(interleaved_f32le) {
            Ok(frames) => write_frame(writer, &Response::WrittenFrames { frames }),
            Err(error) => write_frame(
                writer,
                &Response::Err {
                    message: format!("WriteSamples failed: {error}"),
                },
            ),
        }
    } else {
        write_frame(
            writer,
            &Response::Err {
                message: "not opened".to_string(),
            },
        )
    }
}

fn handle_query_status<W: Write>(state: &RuntimeState, writer: &mut W) -> Result<(), ProtoError> {
    if let Some(stream) = state.stream.as_ref() {
        write_frame(
            writer,
            &Response::Status {
                queued_samples: stream.queued_samples(),
                running: stream.running(),
            },
        )
    } else {
        write_frame(
            writer,
            &Response::Status {
                queued_samples: 0,
                running: false,
            },
        )
    }
}

fn handle_close<W: Write>(state: &mut RuntimeState, writer: &mut W) -> Result<(), ProtoError> {
    let _ = state.stream.take();
    state.active_device_id = None;
    eprintln!("asio host request Close: stream_active=false");
    write_frame(writer, &Response::Ok)
}
