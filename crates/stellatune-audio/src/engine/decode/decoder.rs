use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tracing::debug;

use serde::Deserialize;
use stellatune_core::{TrackDecodeInfo, TrackPlayability, TrackRef};
use stellatune_decode::{Decoder, TrackSpec, supports_path};
use stellatune_plugin_api::{
    ST_DECODER_INFO_FLAG_HAS_DURATION, ST_ERR_INVALID_ARG, ST_ERR_IO, StIoVTable, StSeekWhence,
    StStatus, StStr,
};
use stellatune_plugins::runtime::CapabilityKind as RuntimeCapabilityKind;

const TRACK_REF_TOKEN_PREFIX: &str = "stref-json:";
const GAPLESS_ENTRY_DECLICK_MS: usize = 2;

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

pub(crate) enum DecoderIoOwner {
    Local(Box<LocalFileIoHandle>),
    Source {
        source: stellatune_plugins::SourceCatalogInstance,
        io_handle_addr: usize,
    },
}

impl DecoderIoOwner {
    fn io_vtable_ptr(&self) -> *const StIoVTable {
        match self {
            Self::Local(_) => &LOCAL_FILE_IO_VTABLE as *const StIoVTable,
            Self::Source { .. } => core::ptr::null(),
        }
    }

    fn io_handle_ptr(&mut self) -> *mut core::ffi::c_void {
        match self {
            Self::Local(file) => (&mut **file) as *mut LocalFileIoHandle as *mut core::ffi::c_void,
            Self::Source { io_handle_addr, .. } => *io_handle_addr as *mut core::ffi::c_void,
        }
    }

    fn local(path: &str) -> Result<Self, String> {
        let file =
            File::open(path).map_err(|e| format!("failed to open local file `{path}`: {e}"))?;
        Ok(Self::Local(Box::new(LocalFileIoHandle { file })))
    }
}

impl Drop for DecoderIoOwner {
    fn drop(&mut self) {
        if let Self::Source {
            source,
            io_handle_addr,
        } = self
            && *io_handle_addr != 0
        {
            source.close_stream(*io_handle_addr as *mut core::ffi::c_void);
            *io_handle_addr = 0;
        }
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

static LOCAL_FILE_IO_VTABLE: StIoVTable = StIoVTable {
    read: local_io_read,
    seek: Some(local_io_seek),
    tell: Some(local_io_tell),
    size: Some(local_io_size),
};

#[derive(Debug, Clone, Copy, Default)]
struct GaplessTrimSpec {
    head_frames: u32,
    tail_frames: u32,
}

#[derive(Debug, Default)]
pub(crate) struct GaplessTrimState {
    initial_head_samples: usize,
    head_samples_remaining: usize,
    tail_hold_samples: usize,
    tail_buffer: VecDeque<f32>,
    pending_output: VecDeque<f32>,
    eof_reached: bool,
    channels: usize,
    entry_ramp_total_frames: usize,
    entry_ramp_applied_frames: usize,
    entry_ramp_active: bool,
}

impl GaplessTrimState {
    fn new(spec: GaplessTrimSpec, channels: usize, sample_rate: u32) -> Self {
        let channels = channels.max(1);
        let initial_head_samples = (spec.head_frames as usize).saturating_mul(channels);
        let tail_hold_samples = (spec.tail_frames as usize).saturating_mul(channels);
        let entry_ramp_total_frames = ((sample_rate.max(1) as usize) * GAPLESS_ENTRY_DECLICK_MS)
            .saturating_div(1000)
            .max(1);
        let entry_ramp_active = initial_head_samples > 0;
        Self {
            initial_head_samples,
            head_samples_remaining: initial_head_samples,
            tail_hold_samples,
            tail_buffer: VecDeque::new(),
            pending_output: VecDeque::new(),
            eof_reached: false,
            channels,
            entry_ramp_total_frames,
            entry_ramp_applied_frames: 0,
            entry_ramp_active,
        }
    }

    fn reset_for_seek(&mut self, position_ms: u64) {
        self.pending_output.clear();
        self.tail_buffer.clear();
        self.eof_reached = false;
        self.entry_ramp_applied_frames = 0;
        self.head_samples_remaining = if position_ms == 0 {
            self.entry_ramp_active = self.initial_head_samples > 0;
            self.initial_head_samples
        } else {
            self.entry_ramp_active = false;
            0
        };
    }

    fn apply_entry_ramp_in_place(&mut self, samples: &mut [f32]) {
        if !self.entry_ramp_active || samples.is_empty() {
            return;
        }
        let channels = self.channels.max(1);
        let frames = samples.len() / channels;
        if frames == 0 {
            return;
        }
        let remaining = self
            .entry_ramp_total_frames
            .saturating_sub(self.entry_ramp_applied_frames);
        if remaining == 0 {
            self.entry_ramp_active = false;
            return;
        }
        let apply_frames = remaining.min(frames);
        for frame in 0..apply_frames {
            let progress_frame = self.entry_ramp_applied_frames + frame + 1;
            let t = (progress_frame as f32 / self.entry_ramp_total_frames as f32).clamp(0.0, 1.0);
            let gain = t.sqrt();
            let base = frame * channels;
            for ch in 0..channels {
                samples[base + ch] *= gain;
            }
        }
        self.entry_ramp_applied_frames =
            self.entry_ramp_applied_frames.saturating_add(apply_frames);
        if self.entry_ramp_applied_frames >= self.entry_ramp_total_frames {
            self.entry_ramp_active = false;
        }
    }

    fn push_decoded_samples(&mut self, mut samples: Vec<f32>) {
        if self.head_samples_remaining > 0 {
            let trim = self.head_samples_remaining.min(samples.len());
            if trim == samples.len() {
                samples.clear();
            } else if trim > 0 {
                samples = samples.split_off(trim);
            }
            self.head_samples_remaining = self.head_samples_remaining.saturating_sub(trim);
        }
        if samples.is_empty() {
            return;
        }
        self.apply_entry_ramp_in_place(&mut samples);

        if self.tail_hold_samples == 0 {
            self.pending_output.extend(samples);
            return;
        }

        self.tail_buffer.extend(samples);
        let releasable = self
            .tail_buffer
            .len()
            .saturating_sub(self.tail_hold_samples);
        if releasable > 0 {
            self.pending_output
                .extend(self.tail_buffer.drain(..releasable));
        }
    }

    fn on_eof(&mut self) {
        self.eof_reached = true;
        // Holdback region is exactly the gapless tail trim.
        self.tail_buffer.clear();
    }
}

pub(crate) enum EngineDecoder {
    Builtin {
        dec: Decoder,
        gapless: GaplessTrimState,
    },
    Plugin {
        dec: stellatune_plugins::DecoderInstance,
        spec: TrackSpec,
        gapless: GaplessTrimState,
        _io_owner: DecoderIoOwner,
    },
}

impl EngineDecoder {
    pub fn spec(&self) -> TrackSpec {
        match self {
            Self::Builtin { dec, .. } => dec.spec(),
            Self::Plugin { spec, .. } => *spec,
        }
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        match self {
            Self::Builtin { dec, gapless } => {
                dec.seek_ms(position_ms).map_err(|e| e.to_string())?;
                gapless.reset_for_seek(position_ms);
                Ok(())
            }
            Self::Plugin { dec, gapless, .. } => {
                dec.seek_ms(position_ms).map_err(|e| e.to_string())?;
                gapless.reset_for_seek(position_ms);
                Ok(())
            }
        }
    }

    fn raw_next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        match self {
            Self::Builtin { dec, .. } => dec.next_block(frames).map_err(|e| e.to_string()),
            Self::Plugin { dec, .. } => {
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

    fn gapless_state_mut(&mut self) -> &mut GaplessTrimState {
        match self {
            Self::Builtin { gapless, .. } => gapless,
            Self::Plugin { gapless, .. } => gapless,
        }
    }

    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        let channels = self.spec().channels.max(1) as usize;
        let want_samples = frames.saturating_mul(channels).max(channels);

        loop {
            let need_more = {
                let gapless = self.gapless_state_mut();
                gapless.pending_output.len() < want_samples && !gapless.eof_reached
            };
            if !need_more {
                break;
            }
            match self.raw_next_block(frames)? {
                Some(samples) => self.gapless_state_mut().push_decoded_samples(samples),
                None => self.gapless_state_mut().on_eof(),
            }
        }

        let gapless = self.gapless_state_mut();
        if gapless.pending_output.is_empty() {
            if gapless.eof_reached {
                return Ok(None);
            }
            return Err("decoder produced no samples without eof".to_string());
        }

        let take = want_samples.min(gapless.pending_output.len());
        let mut out = Vec::with_capacity(take);
        out.extend(gapless.pending_output.drain(..take));
        Ok(Some(out))
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

#[derive(Debug, Clone)]
struct DecoderCandidate {
    plugin_id: String,
    type_id: String,
    default_config_json: String,
}

fn build_plugin_track_info(
    dec: &mut stellatune_plugins::DecoderInstance,
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

fn runtime_scored_decoder_candidates(ext_hint: &str) -> Vec<DecoderCandidate> {
    let ext = normalize_ext_hint(ext_hint);
    if ext.is_empty() {
        return Vec::new();
    }
    let service = stellatune_plugins::shared_runtime_service();
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
        out.push(DecoderCandidate {
            plugin_id: candidate.plugin_id,
            type_id: candidate.type_id,
            default_config_json: cap.default_config_json,
        });
    }
    out
}

fn runtime_all_decoder_candidates() -> Vec<DecoderCandidate> {
    let service = stellatune_plugins::shared_runtime_service();
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
            out.push(DecoderCandidate {
                plugin_id: plugin_id.clone(),
                type_id: cap.type_id,
                default_config_json: cap.default_config_json,
            });
        }
    }
    out
}

fn select_decoder_candidates(
    ext_hint: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> Result<Vec<DecoderCandidate>, String> {
    match (decoder_plugin_id, decoder_type_id) {
        (Some(plugin_id), Some(type_id)) => {
            let service = stellatune_plugins::shared_runtime_service();
            let cap = service
                .resolve_active_capability(plugin_id, RuntimeCapabilityKind::Decoder, type_id)
                .ok_or_else(|| {
                    format!(
                        "decoder not found for source track: plugin_id={} type_id={}",
                        plugin_id, type_id
                    )
                })?;
            Ok(vec![DecoderCandidate {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                default_config_json: cap.default_config_json,
            }])
        }
        (Some(plugin_id), None) | (None, Some(plugin_id)) => Err(format!(
            "invalid decoder selector: both plugin_id and type_id are required, got `{plugin_id}` only"
        )),
        (None, None) => {
            let mut out = runtime_scored_decoder_candidates(ext_hint);
            if out.is_empty() {
                out = runtime_all_decoder_candidates();
            }
            if out.is_empty() {
                Err("no v2 decoder candidates available".to_string())
            } else {
                Ok(out)
            }
        }
    }
}

fn try_open_decoder_for_local_path(
    path: &str,
    ext_hint: &str,
) -> Result<
    Option<(
        stellatune_plugins::DecoderInstance,
        TrackDecodeInfo,
        GaplessTrimSpec,
        DecoderIoOwner,
    )>,
    String,
> {
    let candidates = select_decoder_candidates(ext_hint, None, None).unwrap_or_default();
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut last_err: Option<String> = None;
    for candidate in candidates {
        let mut dec = match stellatune_plugins::shared_runtime_service().create_decoder_instance(
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

        match dec.open_with_io(
            path,
            ext_hint,
            io_owner.io_vtable_ptr(),
            io_owner.io_handle_ptr(),
        ) {
            Ok(()) => {
                let (info, gapless) = build_plugin_track_info(
                    &mut dec,
                    &candidate.plugin_id,
                    &candidate.type_id,
                    None,
                )?;
                return Ok(Some((dec, info, gapless, io_owner)));
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

fn try_open_decoder_for_source_stream(
    source: &SourceStreamLocator,
    path_hint: &str,
    ext_hint: &str,
) -> Result<
    Option<(
        stellatune_plugins::DecoderInstance,
        TrackDecodeInfo,
        GaplessTrimSpec,
        DecoderIoOwner,
    )>,
    String,
> {
    let candidates = select_decoder_candidates(
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

    let mut source_inst = match stellatune_plugins::shared_runtime_service()
        .create_source_catalog_instance(&source.plugin_id, &source.type_id, &config_json)
    {
        Ok(v) => v,
        Err(e) => {
            return Err(format!(
                "create_source_catalog_instance failed for {}::{}: {e:#}",
                source.plugin_id, source.type_id
            ));
        }
    };

    let mut last_err: Option<String> = None;
    for candidate in candidates {
        let mut dec = match stellatune_plugins::shared_runtime_service().create_decoder_instance(
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

        let (stream, source_metadata_json) = match source_inst.open_stream(&track_json) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(format!("source open_stream failed: {e:#}"));
                continue;
            }
        };
        let source_metadata = source_metadata_json.and_then(|raw| {
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

        match dec.open_with_io(path_hint, ext_hint, stream.io_vtable, stream.io_handle) {
            Ok(()) => {
                let (info, gapless) = build_plugin_track_info(
                    &mut dec,
                    &candidate.plugin_id,
                    &candidate.type_id,
                    source_metadata,
                )?;
                let io_owner = DecoderIoOwner::Source {
                    source: source_inst,
                    io_handle_addr: stream.io_handle as usize,
                };
                return Ok(Some((dec, info, gapless, io_owner)));
            }
            Err(e) => {
                source_inst.close_stream(stream.io_handle);
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

fn runtime_has_source_catalog(plugin_id: &str, type_id: &str) -> bool {
    stellatune_plugins::shared_runtime_service()
        .resolve_active_capability(plugin_id, RuntimeCapabilityKind::SourceCatalog, type_id)
        .is_some()
}

pub(crate) fn assess_track_playability(track: &TrackRef) -> TrackPlayability {
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
        if !select_decoder_candidates(&ext_hint, None, None)
            .unwrap_or_default()
            .is_empty()
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

    if !runtime_has_source_catalog(&source.plugin_id, &source.type_id) {
        return TrackPlayability {
            track: track.clone(),
            playable: false,
            reason: Some("source_catalog_unavailable".to_string()),
        };
    }

    if select_decoder_candidates(
        source.ext_hint.trim(),
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    )
    .unwrap_or_default()
    .is_empty()
    {
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
) -> Result<(Box<EngineDecoder>, TrackDecodeInfo), String> {
    let track = decode_engine_track_token(track_token)?;

    // Local tracks keep built-in decoder fallback behavior.
    if track.source_id.trim().eq_ignore_ascii_case("local") {
        let path = track.locator.trim();
        if path.is_empty() {
            return Err("local track locator is empty".to_string());
        }
        let ext_hint = ext_hint_from_path(path);

        // Keep built-in first for local files when supported.
        if supports_path(path) {
            match Decoder::open(path) {
                Ok(d) => {
                    let spec = d.spec();
                    let info = build_builtin_track_info(spec);
                    let gapless = GaplessTrimState::new(
                        GaplessTrimSpec {
                            head_frames: d.encoder_delay_frames(),
                            tail_frames: d.encoder_padding_frames(),
                        },
                        spec.channels as usize,
                        spec.sample_rate,
                    );
                    debug!(path, "using built-in decoder for local track");
                    return Ok((Box::new(EngineDecoder::Builtin { dec: d, gapless }), info));
                }
                Err(e) => {
                    debug!("built-in decoder open failed, trying plugin decoders: {e}");
                }
            }
        }

        match try_open_decoder_for_local_path(path, &ext_hint) {
            Ok(Some((dec, info, gapless_spec, io_owner))) => {
                return Ok((
                    Box::new(EngineDecoder::Plugin {
                        spec: TrackSpec {
                            sample_rate: info.sample_rate,
                            channels: info.channels,
                        },
                        dec,
                        gapless: GaplessTrimState::new(
                            gapless_spec,
                            info.channels as usize,
                            info.sample_rate,
                        ),
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

        let d = Decoder::open(path).map_err(|e| format!("failed to open decoder: {e}"))?;
        let spec = d.spec();
        let info = build_builtin_track_info(spec);
        let gapless = GaplessTrimState::new(
            GaplessTrimSpec {
                head_frames: d.encoder_delay_frames(),
                tail_frames: d.encoder_padding_frames(),
            },
            spec.channels as usize,
            spec.sample_rate,
        );
        return Ok((Box::new(EngineDecoder::Builtin { dec: d, gapless }), info));
    }

    // Plugin-backed source track.
    let source = serde_json::from_str::<SourceStreamLocator>(&track.locator)
        .map_err(|e| format!("invalid source track locator json: {e}"))?;
    let ext_hint = source.ext_hint.trim().to_string();
    let path_hint = if source.path_hint.trim().is_empty() {
        track.stable_key()
    } else {
        source.path_hint.trim().to_string()
    };

    match try_open_decoder_for_source_stream(&source, &path_hint, &ext_hint) {
        Ok(Some((dec, info, gapless_spec, io_owner))) => {
            return Ok((
                Box::new(EngineDecoder::Plugin {
                    spec: TrackSpec {
                        sample_rate: info.sample_rate,
                        channels: info.channels,
                    },
                    dec,
                    gapless: GaplessTrimState::new(
                        gapless_spec,
                        info.channels as usize,
                        info.sample_rate,
                    ),
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
    Err(format!(
        "failed to open v2 decoder on source stream `{path_hint}` (ext hint `{ext_hint}`)"
    ))
}
