use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use stellatune_audio_builtin_adapters::builtin_decoder::{
    BuiltinDecoder, builtin_decoder_score_for_ext, builtin_decoder_supported_extensions,
};
use stellatune_audio_builtin_adapters::playlist_decoder::PlaylistDecoder;
use stellatune_audio_core::pipeline::context::{
    AudioBlock, GaplessTrimSpec, PipelineContext, SourceHandle, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_plugin_adapters::stages::{
    PluginDecoderStage, plugin_track_token_from_source_handle,
    probe_track_decode_info_with_decoder_selector,
};
use stellatune_plugins::host_runtime::RuntimeCapabilityKind;

use super::shared_plugin_runtime;

const DEFAULT_READ_FRAMES: u32 = 1024;

pub type SharedUserDecoderProvider = Arc<dyn UserDecoderProvider>;

pub trait UserDecoderImplementation: Send {
    fn spec(&self) -> StreamSpec;
    fn duration_ms_hint(&self) -> Option<u64>;
    fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec>;
    fn seek_ms(&mut self, position_ms: u64) -> Result<(), String>;
    fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String>;
}

pub trait UserDecoderProvider: Send + Sync {
    fn implementation_id(&self) -> &str;
    fn score_for_extension(&self, ext_hint: &str) -> Option<u16>;
    fn supported_extensions(&self) -> Vec<String>;
    fn open(&self, locator: &str) -> Result<Box<dyn UserDecoderImplementation>, String>;
}

pub fn default_user_decoder_providers() -> Vec<SharedUserDecoderProvider> {
    vec![
        Arc::new(PrebuiltUserDecoderProvider),
        Arc::new(PlaylistUserDecoderProvider),
    ]
}

pub fn decoder_supported_extensions_hybrid() -> Vec<String> {
    let providers = default_user_decoder_providers();
    decoder_supported_extensions_hybrid_with_user_decoders(providers.as_slice())
}

pub fn decoder_supported_extensions_hybrid_with_user_decoders(
    user_decoder_providers: &[SharedUserDecoderProvider],
) -> Vec<String> {
    let service = shared_plugin_runtime();
    let mut out = service.decoder_supported_extensions();
    for provider in user_decoder_providers {
        out.extend(provider.supported_extensions());
    }
    if service.decoder_has_wildcard_candidate() {
        out.push("*".to_string());
    }
    out.sort();
    out.dedup();
    out
}

pub struct HybridDecoderStage {
    read_frames: u32,
    active: Option<ActiveHybridDecoder>,
    last_runtime_error: Option<String>,
    last_position_ms: i64,
    user_decoder_providers: Vec<SharedUserDecoderProvider>,
}

enum ActiveHybridDecoder {
    UserImplementation {
        decoder: Box<dyn UserDecoderImplementation>,
    },
    Plugin {
        stage: Box<PluginDecoderStage>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridProbedTrackDecodeInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub metadata_json: Option<String>,
    pub decoder_plugin_id: Option<String>,
    pub decoder_type_id: Option<String>,
}

impl Default for HybridDecoderStage {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridDecoderStage {
    pub fn new() -> Self {
        Self::with_user_decoder_providers(default_user_decoder_providers())
    }

    pub fn with_user_decoder_providers(
        user_decoder_providers: Vec<SharedUserDecoderProvider>,
    ) -> Self {
        Self {
            read_frames: DEFAULT_READ_FRAMES,
            active: None,
            last_runtime_error: None,
            last_position_ms: 0,
            user_decoder_providers,
        }
    }

    pub fn add_user_decoder_provider(
        mut self,
        user_decoder_provider: SharedUserDecoderProvider,
    ) -> Self {
        self.user_decoder_providers.push(user_decoder_provider);
        self
    }

    pub fn with_read_frames(mut self, read_frames: u32) -> Self {
        self.read_frames = read_frames.max(1);
        self
    }

    fn clear_prepared(&mut self, ctx: &mut PipelineContext) {
        if let Some(active) = self.active.as_mut()
            && let ActiveHybridDecoder::Plugin { stage } = active
        {
            stage.stop(ctx);
        }
        self.active = None;
        self.last_runtime_error = None;
    }

    fn resolve_track_ref(source: &SourceHandle) -> Result<TrackRefToken, PipelineError> {
        let Some(track_token) = plugin_track_token_from_source_handle(source) else {
            return Err(PipelineError::StageFailure(
                "hybrid decoder requires plugin source payload".to_string(),
            ));
        };
        decode_track_ref_token(track_token).map_err(PipelineError::StageFailure)
    }

    fn prepare_local_track(
        &mut self,
        source: &SourceHandle,
        ctx: &mut PipelineContext,
        track: &TrackRefToken,
    ) -> Result<StreamSpec, String> {
        let path = track.locator.trim();
        if path.is_empty() {
            return Err("local track locator is empty".to_string());
        }
        let ext_hint = ext_hint_from_path(path);
        let mut candidates = select_local_hybrid_candidates(
            ext_hint.as_str(),
            self.user_decoder_providers.as_slice(),
        );
        if candidates.is_empty() {
            return Err(build_no_decoder_candidates_error(
                path,
                ext_hint.as_str(),
                self.user_decoder_providers.as_slice(),
            ));
        }
        sort_hybrid_candidates(&mut candidates);

        let mut errors = Vec::new();
        for candidate in candidates {
            match candidate {
                HybridDecoderCandidate::UserImplementation {
                    provider_index,
                    implementation_id,
                    ..
                } => {
                    let Some(provider) = self.user_decoder_providers.get(provider_index) else {
                        errors.push(format!(
                            "user decoder provider missing for `{implementation_id}` (index={provider_index})"
                        ));
                        continue;
                    };
                    match provider.open(path) {
                        Ok(decoder) => {
                            let spec = decoder.spec();
                            self.active = Some(ActiveHybridDecoder::UserImplementation { decoder });
                            self.last_runtime_error = None;
                            return Ok(spec);
                        },
                        Err(error) => errors.push(format!(
                            "user decoder `{implementation_id}` open failed: {error}"
                        )),
                    }
                },
                HybridDecoderCandidate::Plugin {
                    plugin_id, type_id, ..
                } => {
                    let mut stage = PluginDecoderStage::new()
                        .with_read_frames(self.read_frames)
                        .with_decoder_selector(plugin_id.clone(), type_id.clone());
                    match stage.prepare(source, ctx) {
                        Ok(spec) => {
                            self.active = Some(ActiveHybridDecoder::Plugin {
                                stage: Box::new(stage),
                            });
                            self.last_runtime_error = None;
                            return Ok(spec);
                        },
                        Err(error) => {
                            errors.push(format!(
                                "plugin decoder {plugin_id}::{type_id} prepare failed: {error}"
                            ));
                        },
                    }
                },
            }
        }

        Err(errors.join("; "))
    }

    fn prepare_source_track(
        &mut self,
        source: &SourceHandle,
        ctx: &mut PipelineContext,
        track: &TrackRefToken,
    ) -> Result<StreamSpec, String> {
        let locator: SourceStreamLocator = serde_json::from_str(track.locator.as_str())
            .map_err(|e| format!("invalid source locator json: {e}"))?;
        let ext_hint = normalize_ext_hint(locator.ext_hint.as_str());
        let mut candidates = select_plugin_candidates(
            ext_hint.as_str(),
            locator.decoder_plugin_id.as_deref(),
            locator.decoder_type_id.as_deref(),
        )?;
        sort_hybrid_candidates(&mut candidates);

        let mut errors = Vec::new();
        for candidate in candidates {
            let HybridDecoderCandidate::Plugin {
                plugin_id, type_id, ..
            } = candidate
            else {
                continue;
            };
            let mut stage = PluginDecoderStage::new()
                .with_read_frames(self.read_frames)
                .with_decoder_selector(plugin_id.clone(), type_id.clone());
            match stage.prepare(source, ctx) {
                Ok(spec) => {
                    self.active = Some(ActiveHybridDecoder::Plugin {
                        stage: Box::new(stage),
                    });
                    self.last_runtime_error = None;
                    return Ok(spec);
                },
                Err(error) => {
                    errors.push(format!(
                        "plugin decoder {plugin_id}::{type_id} prepare failed: {error}"
                    ));
                },
            }
        }

        if errors.is_empty() {
            Err("failed to open any source decoder candidate".to_string())
        } else {
            Err(errors.join("; "))
        }
    }
}

impl DecoderStage for HybridDecoderStage {
    fn prepare(
        &mut self,
        source: &SourceHandle,
        ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.clear_prepared(ctx);
        let track = Self::resolve_track_ref(source)?;
        let spec = if track.source_id.trim().eq_ignore_ascii_case("local") {
            self.prepare_local_track(source, ctx, &track)
        } else {
            self.prepare_source_track(source, ctx, &track)
        }
        .map_err(PipelineError::StageFailure)?;
        Ok(spec)
    }

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        self.last_position_ms = ctx.position_ms;
        match self.active.as_mut() {
            Some(ActiveHybridDecoder::UserImplementation { decoder }) => {
                if let Some(position_ms) = ctx.pending_seek_ms
                    && let Err(error) = decoder.seek_ms(position_ms.max(0) as u64)
                {
                    self.last_runtime_error = Some(error.clone());
                    return Err(PipelineError::StageFailure(error));
                }
                Ok(())
            },
            Some(ActiveHybridDecoder::Plugin { stage }) => {
                if let Err(error) = stage.sync_runtime_control(ctx) {
                    self.last_runtime_error = Some(error.to_string());
                    return Err(error);
                }
                Ok(())
            },
            None => Err(PipelineError::NotPrepared),
        }
    }

    fn current_gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        match self.active.as_ref() {
            Some(ActiveHybridDecoder::UserImplementation { decoder }) => {
                decoder.gapless_trim_spec()
            },
            Some(ActiveHybridDecoder::Plugin { stage }) => stage.current_gapless_trim_spec(),
            None => None,
        }
    }

    fn estimated_remaining_frames(&self) -> Option<u64> {
        match self.active.as_ref() {
            Some(ActiveHybridDecoder::UserImplementation { decoder }) => {
                let duration_ms = decoder.duration_ms_hint()?;
                let position_ms = self.last_position_ms.max(0) as u64;
                let remaining_ms = duration_ms.saturating_sub(position_ms);
                let frames = (remaining_ms as u128)
                    .saturating_mul(decoder.spec().sample_rate.max(1) as u128)
                    / 1000;
                Some(frames.min(u64::MAX as u128) as u64)
            },
            Some(ActiveHybridDecoder::Plugin { stage }) => stage.estimated_remaining_frames(),
            None => None,
        }
    }

    fn next_block(&mut self, out: &mut AudioBlock, ctx: &mut PipelineContext) -> StageStatus {
        self.last_position_ms = ctx.position_ms;
        match self.active.as_mut() {
            Some(ActiveHybridDecoder::UserImplementation { decoder }) => {
                match decoder.next_block(self.read_frames as usize) {
                    Ok(Some(samples)) => {
                        let channels = decoder.spec().channels.max(1) as usize;
                        if !samples.len().is_multiple_of(channels) {
                            self.last_runtime_error = Some(format!(
                                "user decoder produced misaligned block: samples={} channels={channels}",
                                samples.len()
                            ));
                            return StageStatus::Fatal;
                        }
                        out.channels = decoder.spec().channels;
                        out.samples = samples;
                        StageStatus::Ok
                    },
                    Ok(None) => StageStatus::Eof,
                    Err(error) => {
                        self.last_runtime_error = Some(error);
                        StageStatus::Fatal
                    },
                }
            },
            Some(ActiveHybridDecoder::Plugin { stage }) => stage.next_block(out, ctx),
            None => {
                self.last_runtime_error = Some("decoder is not prepared".to_string());
                StageStatus::Fatal
            },
        }
    }

    fn flush(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        match self.active.as_mut() {
            Some(ActiveHybridDecoder::UserImplementation { .. }) => Ok(()),
            Some(ActiveHybridDecoder::Plugin { stage }) => stage.flush(ctx),
            None => Ok(()),
        }
    }

    fn stop(&mut self, ctx: &mut PipelineContext) {
        self.clear_prepared(ctx);
    }
}

pub fn probe_track_decode_info_hybrid(
    track_token: &str,
) -> Result<HybridProbedTrackDecodeInfo, String> {
    let providers = default_user_decoder_providers();
    probe_track_decode_info_hybrid_with_user_decoders(track_token, providers.as_slice())
}

pub fn probe_track_decode_info_hybrid_with_user_decoders(
    track_token: &str,
    user_decoder_providers: &[SharedUserDecoderProvider],
) -> Result<HybridProbedTrackDecodeInfo, String> {
    let track_token = track_token.trim();
    if track_token.is_empty() {
        return Err("track token is empty".to_string());
    }
    let track = decode_track_ref_token(track_token)?;

    if track.source_id.trim().eq_ignore_ascii_case("local") {
        let path = track.locator.trim();
        if path.is_empty() {
            return Err("local track locator is empty".to_string());
        }
        let ext_hint = ext_hint_from_path(path);
        let mut candidates =
            select_local_hybrid_candidates(ext_hint.as_str(), user_decoder_providers);
        if candidates.is_empty() {
            return Err(build_no_decoder_candidates_error(
                path,
                ext_hint.as_str(),
                user_decoder_providers,
            ));
        }
        sort_hybrid_candidates(&mut candidates);

        let mut errors = Vec::new();
        for candidate in candidates {
            match candidate {
                HybridDecoderCandidate::UserImplementation {
                    provider_index,
                    implementation_id,
                    ..
                } => {
                    let Some(provider) = user_decoder_providers.get(provider_index) else {
                        errors.push(format!(
                            "user decoder provider missing for `{implementation_id}` (index={provider_index})"
                        ));
                        continue;
                    };
                    match provider.open(path) {
                        Ok(decoder) => {
                            return Ok(HybridProbedTrackDecodeInfo {
                                sample_rate: decoder.spec().sample_rate,
                                channels: decoder.spec().channels,
                                duration_ms: decoder.duration_ms_hint(),
                                metadata_json: None,
                                decoder_plugin_id: None,
                                decoder_type_id: None,
                            });
                        },
                        Err(error) => errors.push(format!(
                            "user decoder `{implementation_id}` probe failed: {error}"
                        )),
                    }
                },
                HybridDecoderCandidate::Plugin {
                    plugin_id, type_id, ..
                } => match probe_track_decode_info_with_decoder_selector(
                    track_token,
                    Some(plugin_id.as_str()),
                    Some(type_id.as_str()),
                ) {
                    Ok(probed) => {
                        return Ok(HybridProbedTrackDecodeInfo {
                            sample_rate: probed.sample_rate,
                            channels: probed.channels,
                            duration_ms: probed.duration_ms,
                            metadata_json: probed.metadata_json,
                            decoder_plugin_id: Some(probed.decoder_plugin_id),
                            decoder_type_id: Some(probed.decoder_type_id),
                        });
                    },
                    Err(error) => errors.push(format!(
                        "plugin decoder {plugin_id}::{type_id} probe failed: {error}"
                    )),
                },
            }
        }
        return Err(errors.join("; "));
    }

    let source: SourceStreamLocator = serde_json::from_str(track.locator.as_str())
        .map_err(|e| format!("invalid source track locator json: {e}"))?;
    let ext_hint = normalize_ext_hint(source.ext_hint.as_str());
    let mut candidates = select_plugin_candidates(
        ext_hint.as_str(),
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    )?;
    sort_hybrid_candidates(&mut candidates);
    let mut errors = Vec::new();
    for candidate in candidates {
        let HybridDecoderCandidate::Plugin {
            plugin_id, type_id, ..
        } = candidate
        else {
            continue;
        };
        match probe_track_decode_info_with_decoder_selector(
            track_token,
            Some(plugin_id.as_str()),
            Some(type_id.as_str()),
        ) {
            Ok(probed) => {
                return Ok(HybridProbedTrackDecodeInfo {
                    sample_rate: probed.sample_rate,
                    channels: probed.channels,
                    duration_ms: probed.duration_ms,
                    metadata_json: probed.metadata_json,
                    decoder_plugin_id: Some(probed.decoder_plugin_id),
                    decoder_type_id: Some(probed.decoder_type_id),
                });
            },
            Err(error) => errors.push(format!(
                "plugin decoder {plugin_id}::{type_id} probe failed: {error}"
            )),
        }
    }
    Err(errors.join("; "))
}

#[derive(Debug, Clone, Deserialize)]
struct SourceStreamLocator {
    #[serde(default)]
    ext_hint: String,
    #[serde(default)]
    decoder_plugin_id: Option<String>,
    #[serde(default)]
    decoder_type_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TrackRefTokenWire {
    source_id: String,
    #[serde(rename = "track_id")]
    _track_id: String,
    locator: String,
}

#[derive(Debug, Clone)]
struct TrackRefToken {
    source_id: String,
    locator: String,
}

impl TrackRefToken {
    fn for_local_path(path: String) -> Self {
        Self {
            source_id: "local".to_string(),
            locator: path,
        }
    }
}

#[derive(Debug, Clone)]
enum HybridDecoderCandidate {
    UserImplementation {
        provider_index: usize,
        implementation_id: String,
        score: u16,
    },
    Plugin {
        plugin_id: String,
        type_id: String,
        score: u16,
    },
}

impl HybridDecoderCandidate {
    fn score(&self) -> u16 {
        match self {
            Self::UserImplementation { score, .. } => *score,
            Self::Plugin { score, .. } => *score,
        }
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
            locator: parsed.locator,
        });
    }

    Ok(TrackRefToken::for_local_path(token.to_string()))
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

fn select_local_hybrid_candidates(
    ext_hint: &str,
    user_decoder_providers: &[SharedUserDecoderProvider],
) -> Vec<HybridDecoderCandidate> {
    let ext = normalize_ext_hint(ext_hint);
    let mut out = Vec::new();
    if ext.is_empty() {
        out.extend(runtime_all_plugin_candidates());
    } else {
        let plugin_candidates = runtime_scored_plugin_candidates(ext.as_str());
        let user_has_ext_candidate = user_decoder_providers
            .iter()
            .any(|provider| provider.score_for_extension(ext.as_str()).is_some());
        if !plugin_candidates.is_empty() {
            out.extend(plugin_candidates);
        } else if !user_has_ext_candidate {
            // Fall back to all plugin decoders only for unknown extensions.
            out.extend(runtime_all_plugin_candidates());
        }
    }

    for (provider_index, provider) in user_decoder_providers.iter().enumerate() {
        let score = if ext.is_empty() {
            Some(1)
        } else {
            provider.score_for_extension(ext.as_str())
        };
        if let Some(score) = score {
            out.push(HybridDecoderCandidate::UserImplementation {
                provider_index,
                implementation_id: provider.implementation_id().to_string(),
                score,
            });
        }
    }

    out
}

fn runtime_scored_plugin_candidates(ext_hint: &str) -> Vec<HybridDecoderCandidate> {
    let ext = normalize_ext_hint(ext_hint);
    if ext.is_empty() {
        return Vec::new();
    }
    let service = shared_plugin_runtime();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for candidate in service.list_decoder_candidates_for_ext(ext.as_str()) {
        if !seen.insert((candidate.plugin_id.clone(), candidate.type_id.clone())) {
            continue;
        }
        let Some(_capability) = service.find_capability(
            &candidate.plugin_id,
            RuntimeCapabilityKind::Decoder,
            &candidate.type_id,
        ) else {
            continue;
        };
        out.push(HybridDecoderCandidate::Plugin {
            plugin_id: candidate.plugin_id,
            type_id: candidate.type_id,
            score: candidate.score,
        });
    }
    out
}

fn runtime_all_plugin_candidates() -> Vec<HybridDecoderCandidate> {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.decoder_capability_plugin_ids();
    plugin_ids.sort();

    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let mut capabilities = service.list_capabilities_snapshot(&plugin_id);
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            if capability.kind != RuntimeCapabilityKind::Decoder {
                continue;
            }
            out.push(HybridDecoderCandidate::Plugin {
                plugin_id: plugin_id.clone(),
                type_id: capability.type_id,
                score: 1,
            });
        }
    }
    out
}

fn select_plugin_candidates(
    ext_hint: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> Result<Vec<HybridDecoderCandidate>, String> {
    match (decoder_plugin_id, decoder_type_id) {
        (Some(plugin_id), Some(type_id)) => {
            let service = shared_plugin_runtime();
            let _capability = service
                .find_capability(plugin_id, RuntimeCapabilityKind::Decoder, type_id)
                .ok_or_else(|| {
                    format!(
                        "decoder not found: plugin_id={} type_id={}",
                        plugin_id, type_id
                    )
                })?;
            Ok(vec![HybridDecoderCandidate::Plugin {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                score: u16::MAX,
            }])
        },
        (Some(value), None) | (None, Some(value)) => Err(format!(
            "invalid decoder selector: both plugin_id and type_id are required, got `{value}` only"
        )),
        (None, None) => {
            let candidates = runtime_scored_plugin_candidates(ext_hint);
            if candidates.is_empty() {
                Err(build_no_source_decoder_candidates_error(ext_hint))
            } else {
                Ok(candidates)
            }
        },
    }
}

fn build_no_source_decoder_candidates_error(ext_hint: &str) -> String {
    let ext = normalize_ext_hint(ext_hint);
    let service = shared_plugin_runtime();
    let mut decoder_plugin_ids = service.decoder_capability_plugin_ids();
    decoder_plugin_ids.sort();
    let mut supported_exts = service.decoder_supported_extensions();
    supported_exts.sort();
    let has_wildcard = service.decoder_has_wildcard_candidate();
    format!(
        "no decoder candidates available for source stream (ext=`{ext}`, decoder_plugins={decoder_plugin_ids:?}, decoder_supported_exts={supported_exts:?}, decoder_wildcard={has_wildcard})"
    )
}

fn sort_hybrid_candidates(candidates: &mut [HybridDecoderCandidate]) {
    candidates.sort_by(|a, b| {
        b.score().cmp(&a.score()).then_with(|| match (a, b) {
            (
                HybridDecoderCandidate::UserImplementation {
                    implementation_id: a_id,
                    ..
                },
                HybridDecoderCandidate::UserImplementation {
                    implementation_id: b_id,
                    ..
                },
            ) => a_id.cmp(b_id),
            (
                HybridDecoderCandidate::UserImplementation { .. },
                HybridDecoderCandidate::Plugin { .. },
            ) => Ordering::Less,
            (
                HybridDecoderCandidate::Plugin { .. },
                HybridDecoderCandidate::UserImplementation { .. },
            ) => Ordering::Greater,
            (
                HybridDecoderCandidate::Plugin {
                    plugin_id: a_plugin,
                    type_id: a_type,
                    ..
                },
                HybridDecoderCandidate::Plugin {
                    plugin_id: b_plugin,
                    type_id: b_type,
                    ..
                },
            ) => a_plugin.cmp(b_plugin).then_with(|| a_type.cmp(b_type)),
        })
    });
}

fn build_no_decoder_candidates_error(
    path: &str,
    ext_hint: &str,
    user_decoder_providers: &[SharedUserDecoderProvider],
) -> String {
    let ext = normalize_ext_hint(ext_hint);
    let service = shared_plugin_runtime();
    let mut active_plugin_ids = service.active_plugin_ids();
    active_plugin_ids.sort();
    let mut decoder_plugin_ids = service.decoder_capability_plugin_ids();
    decoder_plugin_ids.sort();
    let mut supported_exts = service.decoder_supported_extensions();
    supported_exts.sort();
    let has_wildcard = service.decoder_has_wildcard_candidate();

    let mut matched_user_impls = Vec::<String>::new();
    for provider in user_decoder_providers {
        let score = if ext.is_empty() {
            Some(1)
        } else {
            provider.score_for_extension(ext.as_str())
        };
        if let Some(score) = score {
            matched_user_impls.push(format!("{}:{score}", provider.implementation_id()));
        }
    }
    matched_user_impls.sort();

    let message = format!(
        "no decoder candidates available for local `{path}` (ext=`{ext}`, active_plugins={active_plugin_ids:?}, decoder_plugins={decoder_plugin_ids:?}, decoder_supported_exts={supported_exts:?}, decoder_wildcard={has_wildcard}, matched_user_decoders={matched_user_impls:?})"
    );
    tracing::warn!("{message}");
    message
}

struct PrebuiltUserDecoderProvider;

impl UserDecoderProvider for PrebuiltUserDecoderProvider {
    fn implementation_id(&self) -> &str {
        "prebuilt.symphonia.local_file"
    }

    fn score_for_extension(&self, ext_hint: &str) -> Option<u16> {
        builtin_decoder_score_for_ext(ext_hint)
    }

    fn supported_extensions(&self) -> Vec<String> {
        builtin_decoder_supported_extensions()
    }

    fn open(&self, locator: &str) -> Result<Box<dyn UserDecoderImplementation>, String> {
        let decoder = BuiltinDecoder::open(locator)?;
        Ok(Box::new(PrebuiltUserDecoderInstance { decoder }))
    }
}

struct PrebuiltUserDecoderInstance {
    decoder: BuiltinDecoder,
}

impl UserDecoderImplementation for PrebuiltUserDecoderInstance {
    fn spec(&self) -> StreamSpec {
        self.decoder.spec()
    }

    fn duration_ms_hint(&self) -> Option<u64> {
        self.decoder.duration_ms_hint()
    }

    fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.decoder.gapless_trim_spec()
    }

    fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        self.decoder.seek_ms(position_ms)
    }

    fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        self.decoder.next_block(frames)
    }
}

struct PlaylistUserDecoderProvider;

impl UserDecoderProvider for PlaylistUserDecoderProvider {
    fn implementation_id(&self) -> &str {
        "prebuilt.playlist_m3u8"
    }

    fn score_for_extension(&self, ext_hint: &str) -> Option<u16> {
        match ext_hint {
            "m3u" | "m3u8" => Some(100),
            _ => None,
        }
    }

    fn supported_extensions(&self) -> Vec<String> {
        vec!["m3u".to_string(), "m3u8".to_string()]
    }

    fn open(&self, locator: &str) -> Result<Box<dyn UserDecoderImplementation>, String> {
        let decoder = PlaylistDecoder::open(locator)?;
        Ok(Box::new(PlaylistUserDecoderInstance { decoder }))
    }
}

struct PlaylistUserDecoderInstance {
    decoder: PlaylistDecoder,
}

impl UserDecoderImplementation for PlaylistUserDecoderInstance {
    fn spec(&self) -> StreamSpec {
        self.decoder.spec()
    }

    fn duration_ms_hint(&self) -> Option<u64> {
        self.decoder.duration_ms_hint()
    }

    fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.decoder.gapless_trim_spec()
    }

    fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        self.decoder.seek_ms(position_ms)
    }

    fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        self.decoder.next_block(frames)
    }
}
