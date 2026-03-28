use std::io::Write;
use std::time::Instant;

use stellatune_asio_proto::{PROTOCOL_VERSION, ProtoError, Request, Response, write_frame};

use crate::device::{
    get_device_caps, list_devices, prepare_device_switch, validate_selection_session,
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
        Request::PrepareDeviceSwitch {
            selection_session_id,
            device_id,
        } => {
            handle_prepare_device_switch(state, writer, &selection_session_id, &device_id)?;
        },
        Request::Open {
            prepared_switch_id,
            selection_session_id,
            device_id,
            spec,
            buffer_size_frames,
            queue_capacity_ms,
        } => {
            handle_open(
                state,
                writer,
                OpenRequest {
                    prepared_switch_id,
                    selection_session_id,
                    device_id,
                    spec,
                    buffer_size_frames,
                    queue_capacity_ms,
                },
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
            tracing::debug!(
                "asio host request ListDevices ok: count={} active_device={:?} stream_active={} preview=[{}]",
                devices.len(),
                state.active_device_id,
                state.stream.is_some(),
                preview
            );
            write_frame(writer, &Response::Devices { devices })
        },
        Err(error) => {
            tracing::warn!("asio host request ListDevices err: {error}");
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
            tracing::debug!(
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
            tracing::warn!(
                "asio host request GetDeviceCaps err: device={} session={} err={}",
                device_id,
                selection_session_id,
                error
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

fn handle_prepare_device_switch<W: Write>(
    state: &mut RuntimeState,
    writer: &mut W,
    selection_session_id: &str,
    device_id: &str,
) -> Result<(), ProtoError> {
    match prepare_device_switch(state, selection_session_id, device_id) {
        Ok((prepared_switch_id, caps)) => {
            tracing::debug!(
                "asio host request PrepareDeviceSwitch ok: device={} session={} prepared_switch_id={} default={}Hz/{}ch",
                device_id,
                selection_session_id,
                prepared_switch_id,
                caps.default_spec.sample_rate,
                caps.default_spec.channels
            );
            write_frame(
                writer,
                &Response::PreparedDeviceSwitch {
                    prepared_switch_id,
                    caps,
                },
            )
        },
        Err(error) => {
            tracing::warn!(
                "asio host request PrepareDeviceSwitch err: device={} session={} err={}",
                device_id,
                selection_session_id,
                error
            );
            write_frame(
                writer,
                &Response::Err {
                    message: format!(
                        "PrepareDeviceSwitch failed for device `{device_id}`: {error}"
                    ),
                },
            )
        },
    }
}

fn handle_open<W: Write>(
    state: &mut RuntimeState,
    writer: &mut W,
    request: OpenRequest,
) -> Result<(), ProtoError> {
    let OpenRequest {
        prepared_switch_id,
        selection_session_id,
        device_id,
        spec,
        buffer_size_frames,
        queue_capacity_ms,
    } = request;
    let request_started_at = Instant::now();
    tracing::info!(
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
            if let Err(error) =
                state.consume_prepared_switch(prepared_switch_id, &selection_session_id, &device_id)
            {
                tracing::warn!(
                    "asio host request Open rejected: device={} session={} prepared_switch_id={} err={}",
                    device_id,
                    selection_session_id,
                    prepared_switch_id,
                    error
                );
                return write_frame(
                    writer,
                    &Response::Err {
                        message: format!("Open rejected for device `{device_id}`: {error}"),
                    },
                );
            }
            if state.stream.is_some() {
                tracing::warn!(
                    "asio host request Open rejected: device={} session={} prepared_switch_id={} err=active stream still present after prepare-device-switch",
                    device_id,
                    selection_session_id,
                    prepared_switch_id
                );
                return write_frame(
                    writer,
                    &Response::Err {
                        message: format!(
                            "Open rejected for device `{device_id}`: active stream still present after prepare-device-switch"
                        ),
                    },
                );
            }
            match StreamState::open(&device_id, spec, buffer_size_frames, queue_capacity_ms) {
                Ok(next_state) => {
                    let next_ingress = next_state.ingress();
                    tracing::info!(
                        "asio host request Open ok: device={} session={} spec={}Hz/{}ch",
                        device_id,
                        selection_session_id,
                        requested_sample_rate,
                        requested_channels
                    );
                    if let Some(data_ingress) = state.data_ingress.as_ref() {
                        data_ingress.set_ingress(Some(next_ingress));
                    }
                    state.stream = Some(next_state);
                    state.active_device_id = Some(device_id.clone());
                    tracing::debug!(
                        "asio host request Open end: device={} elapsed_ms={}",
                        device_id,
                        request_started_at.elapsed().as_millis()
                    );
                    write_frame(writer, &Response::Ok)
                },
                Err(error) => {
                    tracing::warn!(
                        "asio host request Open err: device={} session={} err={} elapsed_ms={}",
                        device_id,
                        selection_session_id,
                        error,
                        request_started_at.elapsed().as_millis()
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
            tracing::warn!(
                "asio host request Open rejected: device={} session={} err={}",
                device_id,
                selection_session_id,
                error
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

struct OpenRequest {
    prepared_switch_id: u64,
    selection_session_id: String,
    device_id: String,
    spec: stellatune_asio_proto::AudioSpec,
    buffer_size_frames: Option<u32>,
    queue_capacity_ms: Option<u32>,
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
    let started_at = Instant::now();
    let had_stream = state.stream.is_some();
    tracing::debug!(
        "asio host request Stop begin: had_stream={} active_device={:?}",
        had_stream,
        state.active_device_id
    );
    match state.clear_active_stream() {
        Ok(_) => {
            tracing::debug!(
                "asio host request Stop end: had_stream={} elapsed_ms={}",
                had_stream,
                started_at.elapsed().as_millis()
            );
            write_frame(writer, &Response::Ok)
        },
        Err(error) => write_frame(
            writer,
            &Response::Err {
                message: format!("Stop failed: {error}"),
            },
        ),
    }
}

fn handle_reset<W: Write>(state: &mut RuntimeState, writer: &mut W) -> Result<(), ProtoError> {
    if let Some(stream) = state.stream.as_ref() {
        stream.reset();
        tracing::debug!("asio host request Reset: queue_cleared=true");
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
    match state.clear_active_stream() {
        Ok(_) => {
            tracing::debug!("asio host request Close: stream_active=false");
            write_frame(writer, &Response::Ok)
        },
        Err(error) => write_frame(
            writer,
            &Response::Err {
                message: format!("Close failed: {error}"),
            },
        ),
    }
}
