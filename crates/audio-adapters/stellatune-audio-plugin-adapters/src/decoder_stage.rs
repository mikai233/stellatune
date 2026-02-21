use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;
use stellatune_audio_core::pipeline::context::{
    AudioBlock, GaplessTrimSpec, PipelineContext, SourceHandle, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_wasm_plugins::error::Error as WasmPluginError;
use stellatune_wasm_plugins::executor::plugin_instance::source::RuntimeOpenedSourceStreamHandle;
use stellatune_wasm_plugins::host::stream::{HostStreamHandle, StreamSeekWhence};
use stellatune_wasm_plugins::host_runtime::{
    RuntimeCapabilityKind, RuntimeDecoderPlugin, shared_runtime_service,
};

use crate::source_plugin::PluginSourcePayload;
use crate::source_plugin::plugin_track_token_from_source_handle;

const DEFAULT_READ_FRAMES: u32 = 1024;
const SOURCE_STREAM_READ_BYTES: u32 = 64 * 1024;

pub struct PluginDecoderStage {
    forced_decoder_plugin_id: Option<String>,
    forced_decoder_type_id: Option<String>,
    read_frames: u32,
    prepared: Option<PreparedDecoderState>,
    gapless_trim_spec: Option<GaplessTrimSpec>,
    duration_ms_hint: Option<u64>,
    last_position_ms: i64,
    last_runtime_error: Option<String>,
}

impl Default for PluginDecoderStage {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginDecoderStage {
    pub fn new() -> Self {
        Self {
            forced_decoder_plugin_id: None,
            forced_decoder_type_id: None,
            read_frames: DEFAULT_READ_FRAMES,
            prepared: None,
            gapless_trim_spec: None,
            duration_ms_hint: None,
            last_position_ms: 0,
            last_runtime_error: None,
        }
    }

    pub fn with_decoder_selector(mut self, plugin_id: String, type_id: String) -> Self {
        let plugin_id = plugin_id.trim().to_string();
        let type_id = type_id.trim().to_string();
        if !plugin_id.is_empty() && !type_id.is_empty() {
            self.forced_decoder_plugin_id = Some(plugin_id);
            self.forced_decoder_type_id = Some(type_id);
        }
        self
    }

    pub fn with_read_frames(mut self, read_frames: u32) -> Self {
        self.read_frames = read_frames.max(1);
        self
    }

    pub fn last_runtime_error(&self) -> Option<&str> {
        self.last_runtime_error.as_deref()
    }

    fn resolve_track_ref(source: &SourceHandle) -> Result<TrackRefToken, PipelineError> {
        let Some(track_token) = plugin_track_token_from_source_handle(source) else {
            return Err(PipelineError::StageFailure(
                "plugin decoder requires plugin source payload".to_string(),
            ));
        };
        decode_track_ref_token(track_token).map_err(PipelineError::StageFailure)
    }

    fn decoder_selector_for_track(
        &self,
        source_locator: Option<&SourceStreamLocator>,
    ) -> Result<(Option<String>, Option<String>), String> {
        if let Some(locator) = source_locator
            && locator.decoder_plugin_id.is_some() != locator.decoder_type_id.is_some()
        {
            return Err(
                "source locator decoder selector must contain both plugin_id and type_id"
                    .to_string(),
            );
        }
        let plugin_id = source_locator
            .and_then(|locator| locator.decoder_plugin_id.as_deref())
            .map(str::to_string)
            .or_else(|| self.forced_decoder_plugin_id.clone());
        let type_id = source_locator
            .and_then(|locator| locator.decoder_type_id.as_deref())
            .map(str::to_string)
            .or_else(|| self.forced_decoder_type_id.clone());
        if plugin_id.is_some() != type_id.is_some() {
            return Err(
                "decoder selector must contain both plugin_id and type_id or none".to_string(),
            );
        }
        Ok((plugin_id, type_id))
    }

    fn prepare_local_track(&self, track: &TrackRefToken) -> Result<PreparedDecoderState, String> {
        let path = track.locator.trim();
        if path.is_empty() {
            return Err("local track locator is empty".to_string());
        }
        let ext_hint = ext_hint_from_path(path);
        let (forced_plugin_id, forced_type_id) = self.decoder_selector_for_track(None)?;
        let candidates = select_decoder_candidates(
            &ext_hint,
            forced_plugin_id.as_deref(),
            forced_type_id.as_deref(),
        )?;

        let mut last_error: Option<String> = None;
        for candidate in candidates {
            let stream = open_local_file_stream(path)?;
            match open_decoder_for_stream(
                &candidate.plugin_id,
                &candidate.type_id,
                stream,
                ext_hint.as_str(),
            ) {
                Ok(prepared) => return Ok(prepared),
                Err(error) => {
                    last_error = Some(error);
                },
            }
        }
        Err(last_error.unwrap_or_else(|| {
            format!("failed to open any decoder candidate for local track `{path}`")
        }))
    }

    fn prepare_source_track(&self, track: &TrackRefToken) -> Result<PreparedDecoderState, String> {
        let locator: SourceStreamLocator = serde_json::from_str(track.locator.as_str())
            .map_err(|e| format!("invalid source locator json: {e}"))?;
        let ext_hint = normalize_ext_hint(locator.ext_hint.as_str());
        let path_hint = if locator.path_hint.trim().is_empty() {
            track.stable_key()
        } else {
            locator.path_hint.trim().to_string()
        };
        let (forced_plugin_id, forced_type_id) = self.decoder_selector_for_track(Some(&locator))?;
        let candidates = select_decoder_candidates(
            ext_hint.as_str(),
            forced_plugin_id.as_deref(),
            forced_type_id.as_deref(),
        )?;

        let mut last_error = None::<String>;
        for candidate in candidates {
            let (stream, decoder_ext_hint) =
                open_source_stream_for_decoder(&locator, ext_hint.as_str())?;
            match open_decoder_for_stream(
                &candidate.plugin_id,
                &candidate.type_id,
                stream,
                decoder_ext_hint.as_str(),
            ) {
                Ok(prepared) => return Ok(prepared),
                Err(error) => {
                    last_error = Some(error);
                },
            }
        }
        Err(last_error.unwrap_or_else(|| {
            format!("failed to open any decoder candidate for source track `{path_hint}`")
        }))
    }

    fn apply_pending_seek(&mut self, ctx: &PipelineContext) -> Result<(), PipelineError> {
        let Some(position_ms) = ctx.pending_seek_ms else {
            return Ok(());
        };
        let Some(prepared) = self.prepared.as_mut() else {
            return Err(PipelineError::NotPrepared);
        };
        prepared
            .decoder
            .seek_ms(prepared.session_handle, position_ms.max(0) as u64)
            .map_err(|error| {
                PipelineError::StageFailure(format!(
                    "decoder seek failed for {}::{}: {error}",
                    prepared.plugin_id, prepared.type_id
                ))
            })?;
        Ok(())
    }

    fn clear_prepared(&mut self) {
        if let Some(mut prepared) = self.prepared.take() {
            let _ = prepared.decoder.close(prepared.session_handle);
        }
        self.gapless_trim_spec = None;
        self.duration_ms_hint = None;
        self.last_runtime_error = None;
    }
}

impl DecoderStage for PluginDecoderStage {
    fn prepare(
        &mut self,
        source: &SourceHandle,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.clear_prepared();
        let track = Self::resolve_track_ref(source)?;
        let prepared = if track.source_id.trim().eq_ignore_ascii_case("local") {
            self.prepare_local_track(&track)
                .map_err(PipelineError::StageFailure)?
        } else {
            self.prepare_source_track(&track)
                .map_err(PipelineError::StageFailure)?
        };

        let spec = prepared.stream_spec;
        self.gapless_trim_spec = prepared.gapless_trim_spec;
        self.duration_ms_hint = prepared.duration_ms_hint;
        self.last_runtime_error = None;
        self.prepared = Some(prepared);
        Ok(spec)
    }

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        self.last_position_ms = ctx.position_ms;
        if let Err(error) = self.apply_pending_seek(ctx) {
            self.last_runtime_error = Some(error.to_string());
            return Err(error);
        }
        Ok(())
    }

    fn current_gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.gapless_trim_spec
    }

    fn estimated_remaining_frames(&self) -> Option<u64> {
        let prepared = self.prepared.as_ref()?;
        let duration_ms = self.duration_ms_hint?;
        let position_ms = self.last_position_ms.max(0) as u64;
        let remaining_ms = duration_ms.saturating_sub(position_ms);
        let frames = (remaining_ms as u128)
            .saturating_mul(prepared.stream_spec.sample_rate.max(1) as u128)
            / 1000;
        Some(frames.min(u64::MAX as u128) as u64)
    }

    fn next_block(&mut self, out: &mut AudioBlock, ctx: &mut PipelineContext) -> StageStatus {
        self.last_position_ms = ctx.position_ms;
        let Some(prepared) = self.prepared.as_mut() else {
            self.last_runtime_error = Some("decoder is not prepared".to_string());
            return StageStatus::Fatal;
        };

        let chunk = match prepared
            .decoder
            .read_pcm_f32(prepared.session_handle, self.read_frames.max(1))
        {
            Ok(chunk) => chunk,
            Err(error) => {
                self.last_runtime_error = Some(format!(
                    "decoder read failed for {}::{}: {error}",
                    prepared.plugin_id, prepared.type_id
                ));
                return StageStatus::Fatal;
            },
        };
        if chunk.interleaved_f32le.is_empty() {
            return if chunk.eof {
                StageStatus::Eof
            } else {
                self.last_runtime_error = Some(format!(
                    "decoder returned 0 bytes without eof for {}::{}",
                    prepared.plugin_id, prepared.type_id
                ));
                StageStatus::Fatal
            };
        }
        if !chunk.interleaved_f32le.len().is_multiple_of(4) {
            self.last_runtime_error = Some(format!(
                "decoder produced invalid byte length for {}::{}: {}",
                prepared.plugin_id,
                prepared.type_id,
                chunk.interleaved_f32le.len()
            ));
            return StageStatus::Fatal;
        }
        let mut samples = Vec::<f32>::with_capacity(chunk.interleaved_f32le.len() / 4);
        for bytes in chunk.interleaved_f32le.chunks_exact(4) {
            samples.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        }

        let channels = prepared.stream_spec.channels.max(1) as usize;
        if !samples.len().is_multiple_of(channels) {
            self.last_runtime_error = Some(format!(
                "decoder produced misaligned sample block for {}::{}: samples={} channels={}",
                prepared.plugin_id,
                prepared.type_id,
                samples.len(),
                channels
            ));
            return StageStatus::Fatal;
        }
        out.channels = prepared.stream_spec.channels;
        out.samples = samples;
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.clear_prepared();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbedTrackDecodeInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub metadata_json: Option<String>,
    pub decoder_plugin_id: String,
    pub decoder_type_id: String,
}

pub fn probe_track_decode_info(track_token: &str) -> Result<ProbedTrackDecodeInfo, String> {
    probe_track_decode_info_with_decoder_selector(track_token, None, None)
}

pub fn probe_track_decode_info_with_decoder_selector(
    track_token: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> Result<ProbedTrackDecodeInfo, String> {
    if decoder_plugin_id.is_some() != decoder_type_id.is_some() {
        return Err(
            "invalid decoder selector: both plugin_id and type_id are required".to_string(),
        );
    }
    let track_token = track_token.trim();
    if track_token.is_empty() {
        return Err("track token is empty".to_string());
    }

    let mut stage = PluginDecoderStage::new();
    if let (Some(plugin_id), Some(type_id)) = (decoder_plugin_id, decoder_type_id) {
        stage = stage.with_decoder_selector(plugin_id.to_string(), type_id.to_string());
    }
    let source = SourceHandle::new(PluginSourcePayload {
        track_token: track_token.to_string(),
    });
    let mut ctx = PipelineContext::default();
    let result = (|| {
        let spec = stage
            .prepare(&source, &mut ctx)
            .map_err(|error| format!("probe prepare failed: {error}"))?;
        let prepared = stage
            .prepared
            .as_mut()
            .ok_or_else(|| "decoder probe prepared state missing".to_string())?;
        let metadata_json = prepared
            .decoder
            .metadata(prepared.session_handle)
            .ok()
            .and_then(|metadata| serde_json::to_string(&metadata).ok());
        Ok(ProbedTrackDecodeInfo {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            duration_ms: prepared.duration_ms_hint,
            metadata_json,
            decoder_plugin_id: prepared.plugin_id.clone(),
            decoder_type_id: prepared.type_id.clone(),
        })
    })();
    stage.stop(&mut ctx);
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TrackRefTokenWire {
    source_id: String,
    track_id: String,
    locator: String,
}

#[derive(Debug, Clone)]
struct TrackRefToken {
    source_id: String,
    track_id: String,
    locator: String,
}

impl TrackRefToken {
    fn for_local_path(path: String) -> Self {
        Self {
            source_id: "local".to_string(),
            track_id: path.clone(),
            locator: path,
        }
    }

    fn stable_key(&self) -> String {
        format!("{}:{}", self.source_id, self.track_id)
    }
}

fn decode_track_ref_token(track_token: &str) -> Result<TrackRefToken, String> {
    let token = track_token.trim();
    if token.is_empty() {
        return Err("track token is empty".to_string());
    }

    if token.starts_with('{')
        && let Ok(parsed) = serde_json::from_str::<TrackRefTokenWire>(token)
    {
        return Ok(TrackRefToken {
            source_id: parsed.source_id,
            track_id: parsed.track_id,
            locator: parsed.locator,
        });
    }
    Ok(TrackRefToken::for_local_path(token.to_string()))
}

#[derive(Debug, Clone, Deserialize)]
struct SourceStreamLocator {
    plugin_id: String,
    type_id: String,
    config: Value,
    track: Value,
    #[serde(default)]
    ext_hint: String,
    #[serde(default)]
    path_hint: String,
    #[serde(default)]
    decoder_plugin_id: Option<String>,
    #[serde(default)]
    decoder_type_id: Option<String>,
}

#[derive(Debug, Clone)]
struct DecoderCandidate {
    plugin_id: String,
    type_id: String,
}

fn normalize_ext_hint(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn ext_hint_from_path(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(normalize_ext_hint)
        .unwrap_or_default()
}

fn runtime_scored_decoder_candidates(ext_hint: &str) -> Vec<DecoderCandidate> {
    let ext = normalize_ext_hint(ext_hint);
    if ext.is_empty() {
        return Vec::new();
    }
    let service = shared_runtime_service();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for candidate in service.list_decoder_candidates_for_ext(ext.as_str()) {
        if !seen.insert((candidate.plugin_id.clone(), candidate.type_id.clone())) {
            continue;
        }
        let Some(_) = service.find_capability(
            &candidate.plugin_id,
            RuntimeCapabilityKind::Decoder,
            &candidate.type_id,
        ) else {
            continue;
        };
        out.push(DecoderCandidate {
            plugin_id: candidate.plugin_id,
            type_id: candidate.type_id,
        });
    }
    out
}

fn runtime_all_decoder_candidates() -> Vec<DecoderCandidate> {
    let service = shared_runtime_service();
    let mut plugin_ids = service.decoder_capability_plugin_ids();
    plugin_ids.sort();
    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        for capability in service.list_capabilities(&plugin_id) {
            if capability.kind != RuntimeCapabilityKind::Decoder {
                continue;
            }
            out.push(DecoderCandidate {
                plugin_id: plugin_id.clone(),
                type_id: capability.type_id,
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
            let service = shared_runtime_service();
            let Some(_) =
                service.find_capability(plugin_id, RuntimeCapabilityKind::Decoder, type_id)
            else {
                return Err(format!(
                    "decoder not found: plugin_id={} type_id={}",
                    plugin_id, type_id
                ));
            };
            Ok(vec![DecoderCandidate {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
            }])
        },
        (Some(value), None) | (None, Some(value)) => Err(format!(
            "invalid decoder selector: both plugin_id and type_id are required, got `{value}` only"
        )),
        (None, None) => {
            let mut candidates = runtime_scored_decoder_candidates(ext_hint);
            if candidates.is_empty() {
                candidates = runtime_all_decoder_candidates();
            }
            if candidates.is_empty() {
                Err("no decoder candidates available".to_string())
            } else {
                Ok(candidates)
            }
        },
    }
}

fn open_source_stream_for_decoder(
    locator: &SourceStreamLocator,
    fallback_ext_hint: &str,
) -> Result<(Box<dyn HostStreamHandle>, String), String> {
    let mut source = shared_runtime_service()
        .create_source_plugin(&locator.plugin_id, &locator.type_id)
        .map_err(|error| {
            format!(
                "create source plugin failed for {}::{}: {error}",
                locator.plugin_id, locator.type_id
            )
        })?;
    let config_json = serde_json::to_string(&locator.config)
        .map_err(|e| format!("invalid source config json: {e}"))?;
    source
        .apply_config_update_json(config_json.as_str())
        .map_err(|error| {
            format!(
                "source apply_config_update_json failed for {}::{}: {error}",
                locator.plugin_id, locator.type_id
            )
        })?;
    let track_json = serde_json::to_string(&locator.track)
        .map_err(|e| format!("invalid source track json: {e}"))?;
    let stream = source
        .open_stream_json(track_json.as_str())
        .map_err(|error| {
            format!(
                "source open_stream_json failed for {}::{}: {error}",
                locator.plugin_id, locator.type_id
            )
        })?;
    let ext_hint = normalize_ext_hint(stream.ext_hint.as_deref().unwrap_or(fallback_ext_hint));
    let ext_hint = if ext_hint.is_empty() {
        normalize_ext_hint(fallback_ext_hint)
    } else {
        ext_hint
    };
    match stream.handle {
        RuntimeOpenedSourceStreamHandle::Passthrough(handle) => Ok((handle, ext_hint)),
        RuntimeOpenedSourceStreamHandle::Processed(stream_handle) => {
            let mut bytes = Vec::<u8>::new();
            let read_result = (|| -> Result<(), String> {
                loop {
                    let chunk = source
                        .read(stream_handle, SOURCE_STREAM_READ_BYTES)
                        .map_err(|error| {
                            format!(
                                "source stream read failed for {}::{}: {error}",
                                locator.plugin_id, locator.type_id
                            )
                        })?;
                    if !chunk.bytes.is_empty() {
                        bytes.extend_from_slice(&chunk.bytes);
                    }
                    if chunk.eof {
                        break;
                    }
                }
                Ok(())
            })();
            let close_result = source.close_stream(stream_handle).map_err(|error| {
                format!(
                    "source close_stream failed for {}::{}: {error}",
                    locator.plugin_id, locator.type_id
                )
            });
            if let Err(error) = close_result
                && read_result.is_ok()
            {
                return Err(error);
            }
            read_result?;
            let bytes: Arc<[u8]> = bytes.into();
            Ok((Box::new(MemoryHostStreamHandle::new(bytes)), ext_hint))
        },
    }
}

fn open_local_file_stream(path: &str) -> Result<Box<dyn HostStreamHandle>, String> {
    let file = File::open(path).map_err(|error| {
        format!(
            "failed to open local track stream `{}`: {error}",
            Path::new(path).display()
        )
    })?;
    Ok(Box::new(FileHostStreamHandle::new(file)))
}

fn open_decoder_for_stream(
    plugin_id: &str,
    type_id: &str,
    stream: Box<dyn HostStreamHandle>,
    ext_hint: &str,
) -> Result<PreparedDecoderState, String> {
    let mut decoder = shared_runtime_service()
        .create_decoder_plugin(plugin_id, type_id)
        .map_err(|error| {
            format!(
                "create decoder plugin failed for {}::{}: {error}",
                plugin_id, type_id
            )
        })?;
    let session_handle = decoder
        .open_stream(
            stream,
            (!ext_hint.trim().is_empty()).then_some(ext_hint.trim()),
        )
        .map_err(|error| {
            format!(
                "decoder open_stream failed for {}::{}: {error}",
                plugin_id, type_id
            )
        })?;
    let info = decoder.info(session_handle).map_err(|error| {
        format!(
            "decoder info failed for {}::{} after open_stream: {error}",
            plugin_id, type_id
        )
    })?;
    if info.sample_rate == 0 || info.channels == 0 {
        let _ = decoder.close(session_handle);
        return Err(format!(
            "decoder returned invalid stream spec for {}::{}: sample_rate={} channels={}",
            plugin_id, type_id, info.sample_rate, info.channels
        ));
    }
    let gapless = GaplessTrimSpec {
        head_frames: info.encoder_delay_frames,
        tail_frames: info.encoder_padding_frames,
    };
    Ok(PreparedDecoderState {
        plugin_id: plugin_id.to_string(),
        type_id: type_id.to_string(),
        decoder,
        session_handle,
        stream_spec: StreamSpec {
            sample_rate: info.sample_rate,
            channels: info.channels,
        },
        gapless_trim_spec: (!gapless.is_disabled()).then_some(gapless),
        duration_ms_hint: info.duration_ms,
    })
}

struct FileHostStreamHandle {
    file: File,
}

impl FileHostStreamHandle {
    fn new(file: File) -> Self {
        Self { file }
    }
}

impl HostStreamHandle for FileHostStreamHandle {
    fn read(&mut self, max_bytes: u32) -> Result<Vec<u8>, WasmPluginError> {
        let max_bytes = max_bytes.max(1) as usize;
        let mut buf = vec![0u8; max_bytes];
        let read = self.file.read(&mut buf)?;
        buf.truncate(read);
        Ok(buf)
    }

    fn seek(&mut self, offset: i64, whence: StreamSeekWhence) -> Result<u64, WasmPluginError> {
        let target = match whence {
            StreamSeekWhence::Start => {
                if offset < 0 {
                    return Err(WasmPluginError::invalid_input(
                        "negative offset with seek start",
                    ));
                }
                SeekFrom::Start(offset as u64)
            },
            StreamSeekWhence::Current => SeekFrom::Current(offset),
            StreamSeekWhence::End => SeekFrom::End(offset),
        };
        self.file.seek(target).map_err(WasmPluginError::from)
    }

    fn tell(&mut self) -> Result<u64, WasmPluginError> {
        self.file
            .seek(SeekFrom::Current(0))
            .map_err(WasmPluginError::from)
    }

    fn size(&mut self) -> Result<u64, WasmPluginError> {
        let pos = self.file.seek(SeekFrom::Current(0))?;
        let end = self.file.seek(SeekFrom::End(0))?;
        self.file.seek(SeekFrom::Start(pos))?;
        Ok(end)
    }
}

struct MemoryHostStreamHandle {
    bytes: Arc<[u8]>,
    position: u64,
}

impl MemoryHostStreamHandle {
    fn new(bytes: Arc<[u8]>) -> Self {
        Self { bytes, position: 0 }
    }

    fn seek_target(&self, offset: i64, whence: StreamSeekWhence) -> Result<u64, WasmPluginError> {
        match whence {
            StreamSeekWhence::Start => {
                if offset < 0 {
                    return Err(WasmPluginError::invalid_input(
                        "negative offset with seek start",
                    ));
                }
                Ok(offset as u64)
            },
            StreamSeekWhence::Current => {
                if offset >= 0 {
                    self.position.checked_add(offset as u64).ok_or_else(|| {
                        WasmPluginError::invalid_input("seek overflow with current base")
                    })
                } else {
                    self.position
                        .checked_sub(offset.unsigned_abs())
                        .ok_or_else(|| {
                            WasmPluginError::invalid_input("seek underflow with current base")
                        })
                }
            },
            StreamSeekWhence::End => {
                let end = self.bytes.len() as u64;
                if offset >= 0 {
                    end.checked_add(offset as u64).ok_or_else(|| {
                        WasmPluginError::invalid_input("seek overflow with end base")
                    })
                } else {
                    end.checked_sub(offset.unsigned_abs()).ok_or_else(|| {
                        WasmPluginError::invalid_input("seek underflow with end base")
                    })
                }
            },
        }
    }
}

impl HostStreamHandle for MemoryHostStreamHandle {
    fn read(&mut self, max_bytes: u32) -> Result<Vec<u8>, WasmPluginError> {
        let max_bytes = max_bytes.max(1) as usize;
        let len = self.bytes.len() as u64;
        if self.position >= len {
            return Ok(Vec::new());
        }
        let start = self.position as usize;
        let end = start.saturating_add(max_bytes).min(self.bytes.len());
        self.position = end as u64;
        Ok(self.bytes[start..end].to_vec())
    }

    fn seek(&mut self, offset: i64, whence: StreamSeekWhence) -> Result<u64, WasmPluginError> {
        let position = self.seek_target(offset, whence)?;
        self.position = position;
        Ok(position)
    }

    fn tell(&mut self) -> Result<u64, WasmPluginError> {
        Ok(self.position)
    }

    fn size(&mut self) -> Result<u64, WasmPluginError> {
        Ok(self.bytes.len() as u64)
    }
}

struct PreparedDecoderState {
    plugin_id: String,
    type_id: String,
    decoder: RuntimeDecoderPlugin,
    session_handle: u64,
    stream_spec: StreamSpec,
    gapless_trim_spec: Option<GaplessTrimSpec>,
    duration_ms_hint: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::{TrackRefToken, decode_track_ref_token, ext_hint_from_path};

    #[test]
    fn decode_track_token_defaults_to_local_path() {
        let track = decode_track_ref_token("C:/Music/song.flac").expect("decode failed");
        assert_eq!(track.source_id, "local");
        assert_eq!(track.track_id, "C:/Music/song.flac");
        assert_eq!(track.locator, "C:/Music/song.flac");
    }

    #[test]
    fn decode_track_token_accepts_json_payload() {
        let track =
            decode_track_ref_token(r#"{"source_id":"plugin","track_id":"t1","locator":"{}"}"#)
                .expect("decode failed");
        assert_eq!(track.source_id, "plugin");
        assert_eq!(track.track_id, "t1");
        assert_eq!(track.locator, "{}");
    }

    #[test]
    fn stable_key_formats_source_and_track_id() {
        let track = TrackRefToken {
            source_id: "source-a".to_string(),
            track_id: "track-b".to_string(),
            locator: "{}".to_string(),
        };
        assert_eq!(track.stable_key(), "source-a:track-b");
    }

    #[test]
    fn ext_hint_extracts_lowercase_extension_without_dot() {
        assert_eq!(ext_hint_from_path("C:/music/Track.FLAC"), "flac");
        assert_eq!(ext_hint_from_path("C:/music/noext"), "");
    }
}
