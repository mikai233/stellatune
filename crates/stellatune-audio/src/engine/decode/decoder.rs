use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::debug;

use serde::Deserialize;
use stellatune_core::{TrackDecodeInfo, TrackPlayability, TrackRef};
use stellatune_decode::{Decoder, TrackSpec, supports_path};
use stellatune_plugin_api::{
    ST_DECODER_INFO_FLAG_HAS_DURATION, ST_ERR_INVALID_ARG, ST_ERR_IO, StIoVTableV1, StSeekWhence,
    StStatus, StStr,
};
use stellatune_plugins::runtime::CapabilityKind as RuntimeCapabilityKind;

const TRACK_REF_TOKEN_PREFIX: &str = "stref-json:";

fn snapshot_plugins(
    plugins: &Arc<Mutex<stellatune_plugins::PluginManager>>,
) -> Result<stellatune_plugins::PluginManager, String> {
    plugins
        .lock()
        .map(|pm| pm.clone())
        .map_err(|_| "plugins mutex poisoned".to_string())
}

#[derive(Debug, Deserialize)]
struct SourceStreamLocator {
    plugin_id: String,
    type_id: String,
    config: serde_json::Value,
    track: serde_json::Value,
    #[serde(default)]
    ext_hint: String,
    #[serde(default)]
    path_hint: String,
    #[serde(default)]
    decoder_plugin_id: Option<String>,
    #[serde(default)]
    decoder_type_id: Option<String>,
}

pub(crate) struct LocalFileIoHandle {
    file: File,
}

pub(crate) enum DecoderIoOwnerV2 {
    Local(Box<LocalFileIoHandle>),
    LegacySource(stellatune_plugins::SourceStream),
}

impl DecoderIoOwnerV2 {
    fn io_vtable_ptr(&self) -> *const StIoVTableV1 {
        match self {
            Self::Local(_) => &LOCAL_FILE_IO_VTABLE as *const StIoVTableV1,
            Self::LegacySource(stream) => stream.io_vtable(),
        }
    }

    fn io_handle_ptr(&mut self) -> *mut core::ffi::c_void {
        match self {
            Self::Local(file) => (&mut **file) as *mut LocalFileIoHandle as *mut core::ffi::c_void,
            Self::LegacySource(stream) => stream.io_handle(),
        }
    }

    fn local(path: &str) -> Result<Self, String> {
        let file =
            File::open(path).map_err(|e| format!("failed to open local file `{path}`: {e}"))?;
        Ok(Self::Local(Box::new(LocalFileIoHandle { file })))
    }
}

fn status_code(code: i32) -> StStatus {
    StStatus {
        code,
        message: StStr::empty(),
    }
}

extern "C" fn local_io_read(
    handle: *mut core::ffi::c_void,
    out: *mut u8,
    len: usize,
    out_read: *mut usize,
) -> StStatus {
    if handle.is_null() || out_read.is_null() || (len > 0 && out.is_null()) {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    let out_slice: &mut [u8] = if len == 0 {
        &mut []
    } else {
        unsafe { core::slice::from_raw_parts_mut(out, len) }
    };
    match state.file.read(out_slice) {
        Ok(n) => {
            unsafe {
                *out_read = n;
            }
            StStatus::ok()
        }
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_seek(
    handle: *mut core::ffi::c_void,
    offset: i64,
    whence: StSeekWhence,
    out_pos: *mut u64,
) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    let seek_from = match whence {
        StSeekWhence::Start => {
            if offset < 0 {
                return status_code(ST_ERR_INVALID_ARG);
            }
            SeekFrom::Start(offset as u64)
        }
        StSeekWhence::Current => SeekFrom::Current(offset),
        StSeekWhence::End => SeekFrom::End(offset),
    };
    match state.file.seek(seek_from) {
        Ok(pos) => {
            unsafe {
                *out_pos = pos;
            }
            StStatus::ok()
        }
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_tell(handle: *mut core::ffi::c_void, out_pos: *mut u64) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    match state.file.stream_position() {
        Ok(pos) => {
            unsafe {
                *out_pos = pos;
            }
            StStatus::ok()
        }
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_size(handle: *mut core::ffi::c_void, out_size: *mut u64) -> StStatus {
    if handle.is_null() || out_size.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    match state.file.metadata() {
        Ok(meta) => {
            unsafe {
                *out_size = meta.len();
            }
            StStatus::ok()
        }
        Err(_) => status_code(ST_ERR_IO),
    }
}

static LOCAL_FILE_IO_VTABLE: StIoVTableV1 = StIoVTableV1 {
    read: local_io_read,
    seek: Some(local_io_seek),
    tell: Some(local_io_tell),
    size: Some(local_io_size),
};

pub(crate) enum EngineDecoder {
    Builtin(Decoder),
    Plugin {
        dec: stellatune_plugins::DecoderInstance,
        spec: TrackSpec,
    },
    PluginV2 {
        dec: stellatune_plugins::v2::DecoderInstanceV2,
        spec: TrackSpec,
        _io_owner: DecoderIoOwnerV2,
    },
}

impl EngineDecoder {
    pub fn spec(&self) -> TrackSpec {
        match self {
            Self::Builtin(d) => d.spec(),
            Self::Plugin { spec, .. } => *spec,
            Self::PluginV2 { spec, .. } => *spec,
        }
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        match self {
            Self::Builtin(d) => d.seek_ms(position_ms).map_err(|e| e.to_string()),
            Self::Plugin { dec, .. } => dec.seek_ms(position_ms).map_err(|e| e.to_string()),
            Self::PluginV2 { dec, .. } => dec.seek_ms(position_ms).map_err(|e| e.to_string()),
        }
    }

    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        match self {
            Self::Builtin(d) => d.next_block(frames).map_err(|e| e.to_string()),
            Self::Plugin { dec, .. } => {
                let (samples, eof) = dec
                    .read_interleaved_f32(frames as u32)
                    .map_err(|e| e.to_string())?;
                if samples.is_empty() {
                    if eof {
                        return Ok(None);
                    }
                    return Err("plugin decoder returned 0 frames without eof".to_string());
                }
                Ok(Some(samples))
            }
            Self::PluginV2 { dec, .. } => {
                let (samples, _frames_read, eof) = dec
                    .read_interleaved_f32(frames as u32)
                    .map_err(|e| e.to_string())?;
                if samples.is_empty() {
                    if eof {
                        return Ok(None);
                    }
                    return Err("plugin decoder returned 0 frames without eof".to_string());
                }
                Ok(Some(samples))
            }
        }
    }
}

fn decode_engine_track_token(token: &str) -> Result<stellatune_core::TrackRef, String> {
    if let Some(json) = token.strip_prefix(TRACK_REF_TOKEN_PREFIX) {
        return serde_json::from_str::<stellatune_core::TrackRef>(json)
            .map_err(|e| format!("invalid track ref token: {e}"));
    }
    Ok(stellatune_core::TrackRef::for_local_path(token.to_string()))
}

fn build_builtin_track_info(spec: TrackSpec) -> TrackDecodeInfo {
    TrackDecodeInfo {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        duration_ms: None,
        metadata_json: None,
        decoder_plugin_id: None,
        decoder_type_id: None,
    }
}

fn build_plugin_track_info(
    dec: &mut stellatune_plugins::DecoderInstance,
    fallback_metadata: Option<serde_json::Value>,
) -> Result<TrackDecodeInfo, String> {
    let spec = dec.spec();
    if spec.sample_rate == 0 {
        return Err("plugin decoder returned sample_rate=0".to_string());
    }
    let duration_ms = dec.duration_ms();
    let metadata = dec
        .metadata::<serde_json::Value>()
        .ok()
        .flatten()
        .or(fallback_metadata);
    let mut info = TrackDecodeInfo {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        duration_ms,
        metadata_json: None,
        decoder_plugin_id: Some(dec.plugin_id().to_string()),
        decoder_type_id: Some(dec.decoder_type_id().to_string()),
    };
    info.set_metadata(metadata.as_ref())
        .map_err(|e| format!("failed to serialize decoder metadata: {e}"))?;
    Ok(info)
}

#[derive(Debug, Clone)]
struct DecoderCandidateV2 {
    plugin_id: String,
    type_id: String,
    default_config_json: String,
}

fn build_plugin_track_info_v2(
    dec: &mut stellatune_plugins::v2::DecoderInstanceV2,
    plugin_id: &str,
    decoder_type_id: &str,
    fallback_metadata: Option<serde_json::Value>,
) -> Result<TrackDecodeInfo, String> {
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
    Ok(out)
}

fn list_decoder_keys(
    pm: &stellatune_plugins::PluginManager,
) -> Vec<stellatune_plugins::DecoderKey> {
    let mut types = pm.list_decoder_types();
    types.sort_by(|a, b| {
        a.plugin_id
            .cmp(&b.plugin_id)
            .then_with(|| a.type_id.cmp(&b.type_id))
    });
    types.into_iter().map(|t| t.key).collect()
}

fn normalize_ext_hint(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn ext_hint_from_path(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(normalize_ext_hint)
        .unwrap_or_default()
}

fn runtime_scored_decoder_candidates_v2(ext_hint: &str) -> Vec<DecoderCandidateV2> {
    let ext = normalize_ext_hint(ext_hint);
    if ext.is_empty() {
        return Vec::new();
    }
    let shared = stellatune_plugins::v2::shared_runtime_service_v2();
    let Ok(service) = shared.lock() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for candidate in service.decoder_candidates_for_ext(&ext) {
        if !seen.insert((candidate.plugin_id.clone(), candidate.type_id.clone())) {
            continue;
        }
        let Some(cap) = service.resolve_active_capability(
            &candidate.plugin_id,
            RuntimeCapabilityKind::Decoder,
            &candidate.type_id,
        ) else {
            continue;
        };
        out.push(DecoderCandidateV2 {
            plugin_id: candidate.plugin_id,
            type_id: candidate.type_id,
            default_config_json: cap.default_config_json,
        });
    }
    out
}

fn runtime_all_decoder_candidates_v2() -> Vec<DecoderCandidateV2> {
    let shared = stellatune_plugins::v2::shared_runtime_service_v2();
    let Ok(service) = shared.lock() else {
        return Vec::new();
    };
    let mut plugin_ids = service.active_plugin_ids();
    plugin_ids.sort();
    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let mut caps = service.list_active_capabilities(&plugin_id);
        caps.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for cap in caps {
            if cap.kind != RuntimeCapabilityKind::Decoder {
                continue;
            }
            out.push(DecoderCandidateV2 {
                plugin_id: plugin_id.clone(),
                type_id: cap.type_id,
                default_config_json: cap.default_config_json,
            });
        }
    }
    out
}

fn select_decoder_candidates_v2(
    ext_hint: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> Result<Vec<DecoderCandidateV2>, String> {
    match (decoder_plugin_id, decoder_type_id) {
        (Some(plugin_id), Some(type_id)) => {
            let shared = stellatune_plugins::v2::shared_runtime_service_v2();
            let service = shared
                .lock()
                .map_err(|_| "runtime service mutex poisoned".to_string())?;
            let cap = service
                .resolve_active_capability(plugin_id, RuntimeCapabilityKind::Decoder, type_id)
                .ok_or_else(|| {
                    format!(
                        "decoder not found for source track: plugin_id={} type_id={}",
                        plugin_id, type_id
                    )
                })?;
            Ok(vec![DecoderCandidateV2 {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                default_config_json: cap.default_config_json,
            }])
        }
        (Some(plugin_id), None) | (None, Some(plugin_id)) => Err(format!(
            "invalid decoder selector: both plugin_id and type_id are required, got `{plugin_id}` only"
        )),
        (None, None) => {
            let mut out = runtime_scored_decoder_candidates_v2(ext_hint);
            if out.is_empty() {
                out = runtime_all_decoder_candidates_v2();
            }
            if out.is_empty() {
                Err("no v2 decoder candidates available".to_string())
            } else {
                Ok(out)
            }
        }
    }
}

fn runtime_scored_decoder_keys(
    pm: &stellatune_plugins::PluginManager,
    ext_hint: &str,
) -> Vec<stellatune_plugins::DecoderKey> {
    let ext = normalize_ext_hint(ext_hint);
    if ext.is_empty() {
        return Vec::new();
    }

    let shared = stellatune_plugins::v2::shared_runtime_service_v2();
    let Ok(service) = shared.lock() else {
        return Vec::new();
    };
    let candidates = service.decoder_candidates_for_ext(&ext);
    let mut keys = Vec::new();
    let mut seen = HashSet::new();
    for candidate in candidates {
        let Some(key) = pm.find_decoder_key(&candidate.plugin_id, &candidate.type_id) else {
            continue;
        };
        if seen.insert(key) {
            keys.push(key);
        }
    }
    keys
}

fn select_decoder_keys(
    pm: &stellatune_plugins::PluginManager,
    ext_hint: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> Result<Vec<stellatune_plugins::DecoderKey>, String> {
    match (decoder_plugin_id, decoder_type_id) {
        (Some(plugin_id), Some(type_id)) => pm
            .find_decoder_key(plugin_id, type_id)
            .map(|key| vec![key])
            .ok_or_else(|| {
                format!(
                    "decoder not found for source track: plugin_id={} type_id={}",
                    plugin_id, type_id
                )
            }),
        (Some(plugin_id), None) | (None, Some(plugin_id)) => Err(format!(
            "invalid decoder selector: both plugin_id and type_id are required, got `{plugin_id}` only"
        )),
        (None, None) => {
            let mut keys = runtime_scored_decoder_keys(pm, ext_hint);
            if keys.is_empty() {
                keys = list_decoder_keys(pm);
            }
            if keys.is_empty() {
                Err("no plugin decoders available".to_string())
            } else {
                Ok(keys)
            }
        }
    }
}

fn try_open_plugin_decoder_for_local_path(
    pm: &stellatune_plugins::PluginManager,
    path: &str,
    ext_hint: &str,
) -> Result<Option<(stellatune_plugins::DecoderInstance, TrackDecodeInfo)>, String> {
    let mut keys = runtime_scored_decoder_keys(pm, ext_hint);
    if keys.is_empty() {
        keys = list_decoder_keys(pm);
    }
    if keys.is_empty() {
        return Ok(None);
    }

    let mut last_err: Option<String> = None;
    for key in keys {
        match pm.open_decoder(key, path) {
            Ok(mut dec) => {
                let spec = dec.spec();
                let info = build_plugin_track_info(&mut dec, None)?;
                debug!(
                    path,
                    plugin_id = dec.plugin_id(),
                    decoder_type_id = dec.decoder_type_id(),
                    "using plugin decoder for local track"
                );
                return Ok(Some((
                    dec,
                    TrackDecodeInfo {
                        sample_rate: spec.sample_rate,
                        channels: spec.channels,
                        duration_ms: info.duration_ms,
                        metadata_json: info.metadata_json.clone(),
                        decoder_plugin_id: info.decoder_plugin_id.clone(),
                        decoder_type_id: info.decoder_type_id.clone(),
                    },
                )));
            }
            Err(e) => {
                last_err = Some(format!("{e:#}"));
            }
        }
    }

    match last_err {
        Some(e) => Err(format!(
            "failed to open any plugin decoder for `{path}`: {e}"
        )),
        None => Ok(None),
    }
}

fn try_open_v2_decoder_for_local_path(
    path: &str,
    ext_hint: &str,
) -> Result<
    Option<(
        stellatune_plugins::v2::DecoderInstanceV2,
        TrackDecodeInfo,
        DecoderIoOwnerV2,
    )>,
    String,
> {
    let candidates = select_decoder_candidates_v2(ext_hint, None, None).unwrap_or_default();
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut last_err: Option<String> = None;
    for candidate in candidates {
        let shared = stellatune_plugins::v2::shared_runtime_service_v2();
        let mut dec = match shared.lock() {
            Ok(service) => match service.create_decoder_instance(
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
            },
            Err(_) => {
                last_err = Some("runtime service mutex poisoned".to_string());
                continue;
            }
        };

        let mut io_owner = match DecoderIoOwnerV2::local(path) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };

        match dec.open_with_io(
            path,
            ext_hint,
            io_owner.io_vtable_ptr(),
            io_owner.io_handle_ptr(),
        ) {
            Ok(()) => {
                let info = build_plugin_track_info_v2(
                    &mut dec,
                    &candidate.plugin_id,
                    &candidate.type_id,
                    None,
                )?;
                return Ok(Some((dec, info, io_owner)));
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

fn try_open_v2_decoder_for_legacy_source_stream(
    pm: &stellatune_plugins::PluginManager,
    source_key: stellatune_plugins::SourceCatalogKey,
    source: &SourceStreamLocator,
    path_hint: &str,
    ext_hint: &str,
) -> Result<
    Option<(
        stellatune_plugins::v2::DecoderInstanceV2,
        TrackDecodeInfo,
        DecoderIoOwnerV2,
    )>,
    String,
> {
    let candidates = select_decoder_candidates_v2(
        ext_hint,
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    )
    .unwrap_or_default();
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut last_err: Option<String> = None;
    for candidate in candidates {
        let shared = stellatune_plugins::v2::shared_runtime_service_v2();
        let mut dec = match shared.lock() {
            Ok(service) => match service.create_decoder_instance(
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
            },
            Err(_) => {
                last_err = Some("runtime service mutex poisoned".to_string());
                continue;
            }
        };

        let (stream, source_metadata) = match pm.source_open_stream::<_, _, serde_json::Value>(
            source_key,
            &source.config,
            &source.track,
        ) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(format!("source open_stream failed: {e:#}"));
                continue;
            }
        };
        let mut io_owner = DecoderIoOwnerV2::LegacySource(stream);

        match dec.open_with_io(
            path_hint,
            ext_hint,
            io_owner.io_vtable_ptr(),
            io_owner.io_handle_ptr(),
        ) {
            Ok(()) => {
                let info = build_plugin_track_info_v2(
                    &mut dec,
                    &candidate.plugin_id,
                    &candidate.type_id,
                    source_metadata,
                )?;
                return Ok(Some((dec, info, io_owner)));
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
            "failed to open any v2 decoder for source stream `{path_hint}`: {e}"
        )),
        None => Ok(None),
    }
}

pub(crate) fn assess_track_playability(
    track: &TrackRef,
    pm: &stellatune_plugins::PluginManager,
) -> TrackPlayability {
    if track.source_id.trim().eq_ignore_ascii_case("local") {
        let path = track.locator.trim();
        if path.is_empty() {
            return TrackPlayability {
                track: track.clone(),
                playable: false,
                reason: Some("local_track_locator_empty".to_string()),
            };
        }
        if supports_path(path) {
            return TrackPlayability {
                track: track.clone(),
                playable: true,
                reason: None,
            };
        }
        let ext_hint = ext_hint_from_path(path);
        if !select_decoder_candidates_v2(&ext_hint, None, None)
            .unwrap_or_default()
            .is_empty()
            || !runtime_scored_decoder_keys(pm, &ext_hint).is_empty()
            || !list_decoder_keys(pm).is_empty()
        {
            return TrackPlayability {
                track: track.clone(),
                playable: true,
                reason: None,
            };
        }
        return TrackPlayability {
            track: track.clone(),
            playable: false,
            reason: Some("no_decoder_for_local_track".to_string()),
        };
    }

    let source = match serde_json::from_str::<SourceStreamLocator>(&track.locator) {
        Ok(v) => v,
        Err(_) => {
            return TrackPlayability {
                track: track.clone(),
                playable: false,
                reason: Some("invalid_source_track_locator".to_string()),
            };
        }
    };

    let Some(_source_key) = pm.find_source_catalog_key(&source.plugin_id, &source.type_id) else {
        return TrackPlayability {
            track: track.clone(),
            playable: false,
            reason: Some("source_catalog_unavailable".to_string()),
        };
    };

    let runtime_ok = !select_decoder_candidates_v2(
        source.ext_hint.trim(),
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    )
    .unwrap_or_default()
    .is_empty();
    let legacy_ok = select_decoder_keys(
        pm,
        source.ext_hint.trim(),
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    )
    .map(|v| !v.is_empty())
    .unwrap_or(false);
    if !runtime_ok && !legacy_ok {
        return TrackPlayability {
            track: track.clone(),
            playable: false,
            reason: Some("source_decoder_unavailable".to_string()),
        };
    }

    TrackPlayability {
        track: track.clone(),
        playable: true,
        reason: None,
    }
}

pub(crate) fn open_engine_decoder(
    track_token: &str,
    plugins: &Arc<Mutex<stellatune_plugins::PluginManager>>,
) -> Result<(Box<EngineDecoder>, TrackDecodeInfo), String> {
    let track = decode_engine_track_token(track_token)?;

    // Local tracks keep built-in decoder fallback behavior.
    if track.source_id.trim().eq_ignore_ascii_case("local") {
        let path = track.locator.trim();
        if path.is_empty() {
            return Err("local track locator is empty".to_string());
        }
        let ext_hint = ext_hint_from_path(path);
        let Ok(pm) = snapshot_plugins(plugins) else {
            let d = Decoder::open(path).map_err(|e| format!("failed to open decoder: {e}"))?;
            let spec = d.spec();
            let info = build_builtin_track_info(spec);
            return Ok((Box::new(EngineDecoder::Builtin(d)), info));
        };

        // Keep built-in first for local files when supported.
        if supports_path(path) {
            match Decoder::open(path) {
                Ok(d) => {
                    let spec = d.spec();
                    let info = build_builtin_track_info(spec);
                    debug!(path, "using built-in decoder for local track");
                    return Ok((Box::new(EngineDecoder::Builtin(d)), info));
                }
                Err(e) => {
                    debug!("built-in decoder open failed, trying plugin decoders: {e}");
                }
            }
        }

        match try_open_v2_decoder_for_local_path(path, &ext_hint) {
            Ok(Some((dec, info, io_owner))) => {
                return Ok((
                    Box::new(EngineDecoder::PluginV2 {
                        spec: TrackSpec {
                            sample_rate: info.sample_rate,
                            channels: info.channels,
                        },
                        dec,
                        _io_owner: io_owner,
                    }),
                    info,
                ));
            }
            Ok(None) => {}
            Err(e) => {
                debug!("v2 local decoder open failed: {e}");
            }
        }

        if let Some((dec, info)) = try_open_plugin_decoder_for_local_path(&pm, path, &ext_hint)? {
            let spec = dec.spec();
            return Ok((
                Box::new(EngineDecoder::Plugin {
                    spec: TrackSpec {
                        sample_rate: spec.sample_rate,
                        channels: spec.channels,
                    },
                    dec,
                }),
                info,
            ));
        }

        let d = Decoder::open(path).map_err(|e| format!("failed to open decoder: {e}"))?;
        let spec = d.spec();
        let info = build_builtin_track_info(spec);
        return Ok((Box::new(EngineDecoder::Builtin(d)), info));
    }

    // Plugin-backed source track.
    let source = serde_json::from_str::<SourceStreamLocator>(&track.locator)
        .map_err(|e| format!("invalid source track locator json: {e}"))?;
    let pm = snapshot_plugins(plugins)?;
    let source_key = pm
        .find_source_catalog_key(&source.plugin_id, &source.type_id)
        .ok_or_else(|| {
            format!(
                "source catalog not found: plugin_id={} type_id={}",
                source.plugin_id, source.type_id
            )
        })?;
    let ext_hint = source.ext_hint.trim().to_string();
    let path_hint = if source.path_hint.trim().is_empty() {
        track.stable_key()
    } else {
        source.path_hint.trim().to_string()
    };

    match try_open_v2_decoder_for_legacy_source_stream(
        &pm, source_key, &source, &path_hint, &ext_hint,
    ) {
        Ok(Some((dec, info, io_owner))) => {
            return Ok((
                Box::new(EngineDecoder::PluginV2 {
                    spec: TrackSpec {
                        sample_rate: info.sample_rate,
                        channels: info.channels,
                    },
                    dec,
                    _io_owner: io_owner,
                }),
                info,
            ));
        }
        Ok(None) => {}
        Err(e) => {
            debug!("v2 source decoder open failed: {e}");
        }
    }

    let decoder_keys = select_decoder_keys(
        &pm,
        &ext_hint,
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    )?;

    let mut last_open_err: Option<String> = None;
    for decoder_key in decoder_keys {
        let (stream, source_metadata) = pm
            .source_open_stream::<_, _, serde_json::Value>(
                source_key,
                &source.config,
                &source.track,
            )
            .map_err(|e| format!("source open_stream failed: {e:#}"))?;
        match pm.open_decoder_with_source_stream(decoder_key, &path_hint, &ext_hint, stream) {
            Ok(mut dec) => {
                let spec = dec.spec();
                let info = build_plugin_track_info(&mut dec, source_metadata)?;
                return Ok((
                    Box::new(EngineDecoder::Plugin {
                        spec: TrackSpec {
                            sample_rate: spec.sample_rate,
                            channels: spec.channels,
                        },
                        dec,
                    }),
                    info,
                ));
            }
            Err(e) => {
                last_open_err = Some(format!("{e:#}"));
            }
        }
    }

    Err(match last_open_err {
        Some(e) => format!("failed to open decoder on source stream: {e}"),
        None => format!("no decoder candidates for source ext hint `{ext_hint}`"),
    })
}
