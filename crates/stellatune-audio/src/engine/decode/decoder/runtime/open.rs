use crossbeam_channel::Receiver;
use tracing::debug;

use crate::types::TrackDecodeInfo;
use stellatune_plugin_api::{ST_DECODER_INFO_FLAG_HAS_DURATION, StIoVTable};
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_controller::WorkerApplyPendingOutcome;
use stellatune_plugins::runtime::worker_endpoint::DecoderWorkerController;

use crate::engine::control::{
    source_close_stream_via_runtime_blocking, source_open_stream_via_runtime_blocking,
};

use super::super::gapless::GaplessTrimSpec;
use super::super::io::DecoderIoOwner;
use super::{PluginOpenDecoder, SourceStreamLocator};

fn create_decoder_controller(
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
) -> Result<(DecoderWorkerController, Receiver<WorkerControlMessage>), String> {
    let endpoint = stellatune_runtime::block_on(
        stellatune_plugins::runtime::handle::shared_runtime_service()
            .bind_decoder_worker_endpoint(plugin_id, type_id),
    )
    .map_err(|e| e.to_string())?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok((controller, control_rx))
        }
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            Err("decoder worker controller did not create instance".to_string())
        }
    }
}

fn build_plugin_track_info(
    dec: &mut stellatune_plugins::capabilities::decoder::DecoderInstance,
    plugin_id: &str,
    decoder_type_id: &str,
    fallback_metadata: Option<serde_json::Value>,
) -> Result<(TrackDecodeInfo, GaplessTrimSpec), String> {
    let info = dec.get_info().map_err(|e| e.to_string())?;
    if info.spec.sample_rate == 0 {
        return Err("plugin decoder returned sample_rate=0".to_string());
    }
    let duration_ms = if info.flags & ST_DECODER_INFO_FLAG_HAS_DURATION != 0 {
        Some(info.duration_ms)
    } else {
        None
    };
    let metadata = match dec.get_metadata_json() {
        Ok(Some(raw)) => match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(v) => Some(v),
            Err(e) => {
                debug!(
                    plugin_id,
                    decoder_type_id, "decoder metadata json invalid: {e}"
                );
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            debug!(
                plugin_id,
                decoder_type_id, "decoder metadata unavailable: {e}"
            );
            None
        }
    }
    .or(fallback_metadata);

    let mut out = TrackDecodeInfo {
        sample_rate: info.spec.sample_rate,
        channels: info.spec.channels,
        duration_ms,
        metadata_json: None,
        decoder_plugin_id: Some(plugin_id.to_string()),
        decoder_type_id: Some(decoder_type_id.to_string()),
    };
    out.set_metadata(metadata.as_ref())
        .map_err(|e| format!("failed to serialize decoder metadata: {e}"))?;
    let gapless = GaplessTrimSpec {
        head_frames: info.encoder_delay_frames,
        tail_frames: info.encoder_padding_frames,
    };
    Ok((out, gapless))
}

pub(super) fn try_open_decoder_for_local_path(
    path: &str,
    ext_hint: &str,
) -> Result<Option<PluginOpenDecoder>, String> {
    let candidates = super::select_decoder_candidates(ext_hint, None, None).unwrap_or_default();
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut last_err: Option<String> = None;
    for candidate in candidates {
        let (mut controller, control_rx) = match create_decoder_controller(
            &candidate.plugin_id,
            &candidate.type_id,
            &candidate.default_config_json,
        ) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(format!(
                    "create_decoder_instance failed for {}::{}: {e:#}",
                    candidate.plugin_id, candidate.type_id
                ));
                continue;
            }
        };

        let mut io_owner = match DecoderIoOwner::local(path) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };

        let Some(dec) = controller.instance_mut() else {
            last_err = Some("decoder controller missing instance".to_string());
            continue;
        };

        match dec.open_with_io(
            path,
            ext_hint,
            io_owner.io_vtable_ptr(),
            io_owner.io_handle_ptr(),
        ) {
            Ok(()) => {
                let (info, gapless) =
                    build_plugin_track_info(dec, &candidate.plugin_id, &candidate.type_id, None)?;
                return Ok(Some((controller, info, gapless, io_owner, control_rx)));
            }
            Err(e) => {
                last_err = Some(format!(
                    "decoder open_with_io failed for {}::{}: {e:#}",
                    candidate.plugin_id, candidate.type_id
                ));
            }
        }
    }

    match last_err {
        Some(e) => Err(format!(
            "failed to open any v2 decoder for local track `{path}`: {e}"
        )),
        None => Ok(None),
    }
}

pub(super) fn try_open_decoder_for_source_stream(
    source: &SourceStreamLocator,
    path_hint: &str,
    ext_hint: &str,
) -> Result<Option<PluginOpenDecoder>, String> {
    let candidates = super::select_decoder_candidates(
        ext_hint,
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    )
    .unwrap_or_default();
    if candidates.is_empty() {
        return Ok(None);
    }

    let config_json = serde_json::to_string(&source.config)
        .map_err(|e| format!("invalid source config json: {e}"))?;
    let track_json = serde_json::to_string(&source.track)
        .map_err(|e| format!("invalid source track json: {e}"))?;

    let mut last_err: Option<String> = None;
    for candidate in candidates {
        let (mut controller, control_rx) = match create_decoder_controller(
            &candidate.plugin_id,
            &candidate.type_id,
            &candidate.default_config_json,
        ) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(format!(
                    "create_decoder_instance failed for {}::{}: {e:#}",
                    candidate.plugin_id, candidate.type_id
                ));
                continue;
            }
        };

        let Some(dec) = controller.instance_mut() else {
            last_err = Some("decoder controller missing instance".to_string());
            continue;
        };

        let lease = match source_open_stream_via_runtime_blocking(
            &source.plugin_id,
            &source.type_id,
            config_json.clone(),
            track_json.clone(),
        ) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(format!("source open_stream failed: {e:#}"));
                continue;
            }
        };
        let source_metadata =
            lease.source_metadata_json.and_then(|raw| {
                match serde_json::from_str::<serde_json::Value>(&raw) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        debug!(
                            plugin_id = source.plugin_id,
                            type_id = source.type_id,
                            "source metadata json invalid: {e}"
                        );
                        None
                    }
                }
            });

        match dec.open_with_io(
            path_hint,
            ext_hint,
            lease.io_vtable_addr as *const StIoVTable,
            lease.io_handle_addr as *mut core::ffi::c_void,
        ) {
            Ok(()) => {
                let (info, gapless) = build_plugin_track_info(
                    dec,
                    &candidate.plugin_id,
                    &candidate.type_id,
                    source_metadata,
                )?;
                let io_owner = DecoderIoOwner::Source {
                    stream_id: lease.stream_id,
                    lease_id: lease.lease_id,
                    io_handle_addr: lease.io_handle_addr,
                };
                return Ok(Some((controller, info, gapless, io_owner, control_rx)));
            }
            Err(e) => {
                let _ = source_close_stream_via_runtime_blocking(lease.stream_id);
                last_err = Some(format!(
                    "decoder open_with_io failed for {}::{}: {e:#}",
                    candidate.plugin_id, candidate.type_id
                ));
            }
        }
    }

    match last_err {
        Some(e) => Err(format!(
            "failed to open any v2 decoder for source stream `{path_hint}`: {e}"
        )),
        None => Ok(None),
    }
}
