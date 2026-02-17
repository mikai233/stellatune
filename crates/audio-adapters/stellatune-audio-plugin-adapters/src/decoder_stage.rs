use std::collections::HashSet;
use std::ffi::c_void;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::slice;

use crossbeam_channel::Receiver;
use serde::Deserialize;
use serde_json::Value;
use stellatune_audio_core::pipeline::context::{
    AudioBlock, GaplessTrimSpec, PipelineContext, SourceHandle, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_plugin_api::{
    ST_DECODER_INFO_FLAG_HAS_DURATION, ST_ERR_INVALID_ARG, ST_ERR_IO, StIoVTable, StSeekWhence,
    StStatus, StStr,
};
use stellatune_plugins::runtime::handle::shared_runtime_service;
use stellatune_plugins::runtime::introspection::CapabilityKind as RuntimeCapabilityKind;
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_controller::WorkerApplyPendingOutcome;
use stellatune_plugins::runtime::worker_endpoint::{
    DecoderWorkerController, SourceCatalogWorkerController,
};

use crate::source_plugin::PluginSourcePayload;
use crate::source_plugin::plugin_track_token_from_source_handle;

const DEFAULT_READ_FRAMES: u32 = 1024;

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
            let (mut controller, control_rx) = match create_decoder_controller(
                &candidate.plugin_id,
                &candidate.type_id,
                &candidate.default_config_json,
            ) {
                Ok(v) => v,
                Err(error) => {
                    last_error = Some(error);
                    continue;
                },
            };

            let mut io_owner = DecoderIoOwner::local(path)?;
            let open_args = DecoderOpenArgs {
                path_hint: path.to_string(),
                ext_hint: ext_hint.clone(),
            };
            let opened = match open_decoder_instance(&mut controller, &open_args, &mut io_owner) {
                Ok(v) => v,
                Err(error) => {
                    last_error = Some(format!(
                        "decoder open_with_io failed for {}::{} on `{path}`: {error}",
                        candidate.plugin_id, candidate.type_id
                    ));
                    continue;
                },
            };

            return Ok(PreparedDecoderState {
                plugin_id: candidate.plugin_id,
                type_id: candidate.type_id,
                controller,
                control_rx,
                io_owner,
                open_args,
                stream_spec: opened.stream_spec,
                gapless_trim_spec: opened.gapless_trim_spec,
                duration_ms_hint: opened.duration_ms_hint,
            });
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
            &ext_hint,
            forced_plugin_id.as_deref(),
            forced_type_id.as_deref(),
        )?;

        let mut last_error: Option<String> = None;
        for candidate in candidates {
            let (mut controller, control_rx) = match create_decoder_controller(
                &candidate.plugin_id,
                &candidate.type_id,
                &candidate.default_config_json,
            ) {
                Ok(v) => v,
                Err(error) => {
                    last_error = Some(error);
                    continue;
                },
            };

            let source_lease = match SourceStreamLease::open(&locator) {
                Ok(v) => v,
                Err(error) => {
                    last_error = Some(error);
                    continue;
                },
            };
            let mut io_owner = DecoderIoOwner::source(source_lease);
            let open_args = DecoderOpenArgs {
                path_hint: path_hint.clone(),
                ext_hint: ext_hint.clone(),
            };
            let opened = match open_decoder_instance(&mut controller, &open_args, &mut io_owner) {
                Ok(v) => v,
                Err(error) => {
                    last_error = Some(format!(
                        "decoder open_with_io failed for {}::{} on source `{}`: {error}",
                        candidate.plugin_id, candidate.type_id, path_hint
                    ));
                    continue;
                },
            };

            return Ok(PreparedDecoderState {
                plugin_id: candidate.plugin_id,
                type_id: candidate.type_id,
                controller,
                control_rx,
                io_owner,
                open_args,
                stream_spec: opened.stream_spec,
                gapless_trim_spec: opened.gapless_trim_spec,
                duration_ms_hint: opened.duration_ms_hint,
            });
        }

        Err(last_error.unwrap_or_else(|| {
            format!("failed to open any decoder candidate for source track `{path_hint}`")
        }))
    }

    fn refresh_decoder_runtime_control(&mut self) -> Result<(), PipelineError> {
        let Some(prepared) = self.prepared.as_mut() else {
            return Ok(());
        };

        while let Ok(message) = prepared.control_rx.try_recv() {
            prepared.controller.on_control_message(message);
        }
        if !prepared.controller.has_pending_recreate() && !prepared.controller.has_pending_destroy()
        {
            return Ok(());
        }

        let previous_state_json = prepared
            .controller
            .instance()
            .and_then(|instance| instance.export_state_json().ok().flatten());
        let outcome = prepared.controller.apply_pending().map_err(|e| {
            PipelineError::StageFailure(format!(
                "decoder apply_pending failed for {}::{}: {e}",
                prepared.plugin_id, prepared.type_id
            ))
        })?;
        match outcome {
            WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
                let reopened = open_decoder_instance(
                    &mut prepared.controller,
                    &prepared.open_args,
                    &mut prepared.io_owner,
                )
                .map_err(PipelineError::StageFailure)?;
                if reopened.stream_spec != prepared.stream_spec {
                    return Err(PipelineError::StageFailure(format!(
                        "decoder runtime recreate changed stream spec for {}::{} ({}ch@{} -> {}ch@{})",
                        prepared.plugin_id,
                        prepared.type_id,
                        prepared.stream_spec.channels,
                        prepared.stream_spec.sample_rate,
                        reopened.stream_spec.channels,
                        reopened.stream_spec.sample_rate
                    )));
                }
                if let Some(state_json) = previous_state_json
                    && let Some(instance) = prepared.controller.instance_mut()
                {
                    let _ = instance.import_state_json(&state_json);
                }
                self.gapless_trim_spec = reopened.gapless_trim_spec;
                self.duration_ms_hint = reopened.duration_ms_hint;
                self.last_runtime_error = None;
                Ok(())
            },
            WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
                Err(PipelineError::StageFailure(format!(
                    "decoder controller has no active instance after control for {}::{}",
                    prepared.plugin_id, prepared.type_id
                )))
            },
        }
    }

    fn apply_pending_seek(&mut self, ctx: &PipelineContext) -> Result<(), PipelineError> {
        let Some(position_ms) = ctx.pending_seek_ms else {
            return Ok(());
        };
        let Some(prepared) = self.prepared.as_mut() else {
            return Err(PipelineError::NotPrepared);
        };
        let Some(decoder) = prepared.controller.instance_mut() else {
            return Err(PipelineError::StageFailure(format!(
                "decoder instance unavailable for seek: {}::{}",
                prepared.plugin_id, prepared.type_id
            )));
        };
        decoder.seek_ms(position_ms.max(0) as u64).map_err(|e| {
            PipelineError::StageFailure(format!(
                "decoder seek failed for {}::{}: {e}",
                prepared.plugin_id, prepared.type_id
            ))
        })?;
        Ok(())
    }

    fn clear_prepared(&mut self) {
        if let Some(mut prepared) = self.prepared.take() {
            prepared.controller.request_destroy();
            let _ = prepared.controller.apply_pending();
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
        if let Err(error) = self.refresh_decoder_runtime_control() {
            self.last_runtime_error = Some(error.to_string());
            return Err(error);
        }
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
        let Some(decoder) = prepared.controller.instance_mut() else {
            self.last_runtime_error = Some(format!(
                "decoder instance unavailable for {}::{}",
                prepared.plugin_id, prepared.type_id
            ));
            return StageStatus::Fatal;
        };

        let frames = self.read_frames.max(1);
        let (samples, _frames_read, eof) = match decoder.read_interleaved_f32(frames) {
            Ok(v) => v,
            Err(error) => {
                self.last_runtime_error = Some(format!(
                    "decoder read failed for {}::{}: {error}",
                    prepared.plugin_id, prepared.type_id
                ));
                return StageStatus::Fatal;
            },
        };
        if samples.is_empty() {
            if eof {
                return StageStatus::Eof;
            }
            self.last_runtime_error = Some(format!(
                "decoder returned 0 frames without eof for {}::{}",
                prepared.plugin_id, prepared.type_id
            ));
            return StageStatus::Fatal;
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
    let track_token = track_token.trim();
    if track_token.is_empty() {
        return Err("track token is empty".to_string());
    }

    let mut stage = PluginDecoderStage::new();
    let source = SourceHandle::new(PluginSourcePayload {
        track_token: track_token.to_string(),
    });
    let mut ctx = PipelineContext::default();

    let result = (|| {
        let spec = stage
            .prepare(&source, &mut ctx)
            .map_err(|e| format!("probe prepare failed: {e}"))?;

        let prepared = stage
            .prepared
            .as_mut()
            .ok_or_else(|| "decoder probe prepared state missing".to_string())?;
        let metadata_json = prepared
            .controller
            .instance_mut()
            .and_then(|instance| instance.get_metadata_json().ok().flatten());
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
    default_config_json: String,
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
    for candidate in service.list_decoder_candidates_for_ext_cached(&ext) {
        if !seen.insert((candidate.plugin_id.clone(), candidate.type_id.clone())) {
            continue;
        }
        let Some(capability) = service.find_capability_cached(
            &candidate.plugin_id,
            RuntimeCapabilityKind::Decoder,
            &candidate.type_id,
        ) else {
            continue;
        };
        out.push(DecoderCandidate {
            plugin_id: candidate.plugin_id,
            type_id: candidate.type_id,
            default_config_json: capability.default_config_json,
        });
    }
    out
}

fn runtime_all_decoder_candidates() -> Vec<DecoderCandidate> {
    let service = shared_runtime_service();
    let mut plugin_ids = service.cached_capability_plugin_ids();
    plugin_ids.sort();

    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let mut capabilities = service.list_capabilities_cached(&plugin_id);
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            if capability.kind != RuntimeCapabilityKind::Decoder {
                continue;
            }
            out.push(DecoderCandidate {
                plugin_id: plugin_id.clone(),
                type_id: capability.type_id,
                default_config_json: capability.default_config_json,
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
            let capability = service
                .find_capability_cached(plugin_id, RuntimeCapabilityKind::Decoder, type_id)
                .ok_or_else(|| {
                    format!(
                        "decoder not found: plugin_id={} type_id={}",
                        plugin_id, type_id
                    )
                })?;
            Ok(vec![DecoderCandidate {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                default_config_json: capability.default_config_json,
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

fn create_decoder_controller(
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
) -> Result<(DecoderWorkerController, Receiver<WorkerControlMessage>), String> {
    let endpoint = stellatune_runtime::block_on(
        shared_runtime_service().bind_decoder_worker_endpoint(plugin_id, type_id),
    )
    .map_err(|e| {
        format!(
            "bind_decoder_worker_endpoint failed for {}::{}: {e}",
            plugin_id, type_id
        )
    })?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| {
        format!(
            "decoder apply_pending failed for {}::{}: {e}",
            plugin_id, type_id
        )
    })? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok((controller, control_rx))
        },
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "decoder controller has no instance for {}::{}",
            plugin_id, type_id
        )),
    }
}

fn create_source_catalog_controller(
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
) -> Result<
    (
        SourceCatalogWorkerController,
        Receiver<WorkerControlMessage>,
    ),
    String,
> {
    let endpoint = stellatune_runtime::block_on(
        shared_runtime_service().bind_source_catalog_worker_endpoint(plugin_id, type_id),
    )
    .map_err(|e| {
        format!(
            "bind_source_catalog_worker_endpoint failed for {}::{}: {e}",
            plugin_id, type_id
        )
    })?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| {
        format!(
            "source catalog apply_pending failed for {}::{}: {e}",
            plugin_id, type_id
        )
    })? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok((controller, control_rx))
        },
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "source catalog controller has no instance for {}::{}",
            plugin_id, type_id
        )),
    }
}

struct SourceStreamLease {
    controller: SourceCatalogWorkerController,
    control_rx: Receiver<WorkerControlMessage>,
    io_vtable_addr: usize,
    io_handle_addr: usize,
}

impl SourceStreamLease {
    fn open(locator: &SourceStreamLocator) -> Result<Self, String> {
        let config_json = serde_json::to_string(&locator.config)
            .map_err(|e| format!("invalid source config json: {e}"))?;
        let track_json = serde_json::to_string(&locator.track)
            .map_err(|e| format!("invalid source track json: {e}"))?;
        let (mut controller, control_rx) =
            create_source_catalog_controller(&locator.plugin_id, &locator.type_id, &config_json)?;
        let Some(source) = controller.instance_mut() else {
            return Err(format!(
                "source catalog instance missing for {}::{}",
                locator.plugin_id, locator.type_id
            ));
        };
        let (stream, _meta) = stellatune_runtime::block_on(source.open_stream(track_json.as_str()))
            .map_err(|e| {
                format!(
                    "source open_stream failed for {}::{}: {e}",
                    locator.plugin_id, locator.type_id
                )
            })?;
        Ok(Self {
            controller,
            control_rx,
            io_vtable_addr: stream.io_vtable as usize,
            io_handle_addr: stream.io_handle as usize,
        })
    }
}

impl Drop for SourceStreamLease {
    fn drop(&mut self) {
        if self.io_handle_addr == 0 {
            return;
        }
        while let Ok(message) = self.control_rx.try_recv() {
            self.controller.on_control_message(message);
        }
        if self.controller.has_pending_destroy() || self.controller.has_pending_recreate() {
            let _ = self.controller.apply_pending();
        }
        if let Some(source) = self.controller.instance_mut() {
            source.close_stream(self.io_handle_addr as *mut c_void);
        }
        self.io_handle_addr = 0;
    }
}

struct LocalFileIoHandle {
    file: File,
}

enum DecoderIoOwner {
    Local(Box<LocalFileIoHandle>),
    Source(Box<SourceStreamLease>),
}

impl DecoderIoOwner {
    fn local(path: &str) -> Result<Self, String> {
        let file =
            File::open(path).map_err(|e| format!("failed to open local file `{path}`: {e}"))?;
        Ok(Self::Local(Box::new(LocalFileIoHandle { file })))
    }

    fn source(source: SourceStreamLease) -> Self {
        Self::Source(Box::new(source))
    }

    fn io_vtable_ptr(&self) -> *const StIoVTable {
        match self {
            Self::Local(_) => &LOCAL_FILE_IO_VTABLE as *const StIoVTable,
            Self::Source(source) => source.io_vtable_addr as *const StIoVTable,
        }
    }

    fn io_handle_ptr(&mut self) -> *mut c_void {
        match self {
            Self::Local(file) => (&mut **file) as *mut LocalFileIoHandle as *mut c_void,
            Self::Source(source) => source.io_handle_addr as *mut c_void,
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
    handle: *mut c_void,
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
        unsafe { slice::from_raw_parts_mut(out, len) }
    };
    match state.file.read(out_slice) {
        Ok(read) => {
            unsafe {
                *out_read = read;
            }
            StStatus::ok()
        },
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_seek(
    handle: *mut c_void,
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
        },
        StSeekWhence::Current => SeekFrom::Current(offset),
        StSeekWhence::End => SeekFrom::End(offset),
    };
    match state.file.seek(seek_from) {
        Ok(position) => {
            unsafe {
                *out_pos = position;
            }
            StStatus::ok()
        },
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_tell(handle: *mut c_void, out_pos: *mut u64) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    match state.file.stream_position() {
        Ok(position) => {
            unsafe {
                *out_pos = position;
            }
            StStatus::ok()
        },
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_size(handle: *mut c_void, out_size: *mut u64) -> StStatus {
    if handle.is_null() || out_size.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    match state.file.metadata() {
        Ok(metadata) => {
            unsafe {
                *out_size = metadata.len();
            }
            StStatus::ok()
        },
        Err(_) => status_code(ST_ERR_IO),
    }
}

static LOCAL_FILE_IO_VTABLE: StIoVTable = StIoVTable {
    read: local_io_read,
    seek: Some(local_io_seek),
    tell: Some(local_io_tell),
    size: Some(local_io_size),
};

#[derive(Debug, Clone)]
struct DecoderOpenArgs {
    path_hint: String,
    ext_hint: String,
}

#[derive(Debug, Clone, Copy)]
struct OpenedDecoder {
    stream_spec: StreamSpec,
    gapless_trim_spec: Option<GaplessTrimSpec>,
    duration_ms_hint: Option<u64>,
}

fn open_decoder_instance(
    controller: &mut DecoderWorkerController,
    open_args: &DecoderOpenArgs,
    io_owner: &mut DecoderIoOwner,
) -> Result<OpenedDecoder, String> {
    let Some(decoder) = controller.instance_mut() else {
        return Err("decoder controller has no active instance".to_string());
    };
    decoder
        .open_with_io(
            open_args.path_hint.as_str(),
            open_args.ext_hint.as_str(),
            io_owner.io_vtable_ptr(),
            io_owner.io_handle_ptr(),
        )
        .map_err(|e| format!("decoder open_with_io failed: {e}"))?;
    let info = decoder
        .get_info()
        .map_err(|e| format!("decoder get_info failed: {e}"))?;
    if info.spec.sample_rate == 0 || info.spec.channels == 0 {
        return Err(format!(
            "decoder returned invalid stream spec: sample_rate={} channels={}",
            info.spec.sample_rate, info.spec.channels
        ));
    }
    let stream_spec = StreamSpec {
        sample_rate: info.spec.sample_rate,
        channels: info.spec.channels,
    };
    let gapless = GaplessTrimSpec {
        head_frames: info.encoder_delay_frames,
        tail_frames: info.encoder_padding_frames,
    };
    let duration_ms_hint = if info.flags & ST_DECODER_INFO_FLAG_HAS_DURATION != 0 {
        Some(info.duration_ms)
    } else {
        None
    };
    Ok(OpenedDecoder {
        stream_spec,
        gapless_trim_spec: (!gapless.is_disabled()).then_some(gapless),
        duration_ms_hint,
    })
}

struct PreparedDecoderState {
    plugin_id: String,
    type_id: String,
    controller: DecoderWorkerController,
    control_rx: Receiver<WorkerControlMessage>,
    io_owner: DecoderIoOwner,
    open_args: DecoderOpenArgs,
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
