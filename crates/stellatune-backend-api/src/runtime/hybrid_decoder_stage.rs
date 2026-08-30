use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use stellatune_audio_builtin_adapters::builtin_decoder::{
    BuiltinDecoder, builtin_decoder_score_for_ext, builtin_decoder_supported_extensions,
};
use stellatune_audio_builtin_adapters::ncm_decoder::NcmDecoder;
use stellatune_audio_builtin_adapters::playlist_decoder::PlaylistDecoder;
use stellatune_audio_builtin_adapters::source_local::local_track_token_from_source_handle;
use stellatune_audio_core::pipeline::context::{
    AudioBlock, GaplessTrimSpec, PipelineContext, SourceHandle, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::{Stage, StageFlow};

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
        Arc::new(NcmUserDecoderProvider),
        Arc::new(PrebuiltUserDecoderProvider),
        Arc::new(PlaylistUserDecoderProvider),
    ]
}

pub fn decoder_supported_extensions_hybrid() -> Vec<String> {
    decoder_supported_extensions_hybrid_with_user_decoders(&default_user_decoder_providers())
}

pub fn decoder_supported_extensions_hybrid_with_user_decoders(
    providers: &[SharedUserDecoderProvider],
) -> Vec<String> {
    let mut extensions = providers
        .iter()
        .flat_map(|provider| provider.supported_extensions())
        .collect::<Vec<_>>();
    extensions.sort();
    extensions.dedup();
    extensions
}

pub struct HybridDecoderStage {
    read_frames: u32,
    active: Option<Box<dyn UserDecoderImplementation>>,
    last_position_ms: i64,
    providers: Vec<SharedUserDecoderProvider>,
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

    pub fn with_user_decoder_providers(providers: Vec<SharedUserDecoderProvider>) -> Self {
        Self {
            read_frames: DEFAULT_READ_FRAMES,
            active: None,
            last_position_ms: 0,
            providers,
        }
    }

    pub fn add_user_decoder_provider(mut self, provider: SharedUserDecoderProvider) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn with_read_frames(mut self, read_frames: u32) -> Self {
        self.read_frames = read_frames.max(1);
        self
    }

    fn open(&self, locator: &str) -> Result<Box<dyn UserDecoderImplementation>, String> {
        open_with_providers(locator, &self.providers)
    }
}

impl Stage for HybridDecoderStage {
    fn refresh_runtime_state(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        self.last_position_ms = ctx.position_ms;
        let decoder = self.active.as_mut().ok_or(PipelineError::NotPrepared)?;
        if let Some(position_ms) = ctx.pending_seek_ms {
            decoder
                .seek_ms(position_ms.max(0) as u64)
                .map_err(PipelineError::StageFailure)?;
        }
        Ok(())
    }
}

impl DecoderStage for HybridDecoderStage {
    fn prepare(
        &mut self,
        source: &SourceHandle,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.active = None;
        let token = local_track_token_from_source_handle(source).ok_or_else(|| {
            PipelineError::StageFailure("native decoder requires a native source handle".into())
        })?;
        let locator = decode_track_ref_token(token)
            .map_err(PipelineError::StageFailure)?
            .locator;
        let decoder = self.open(&locator).map_err(PipelineError::StageFailure)?;
        let spec = decoder.spec();
        self.active = Some(decoder);
        Ok(spec)
    }

    fn current_gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.active.as_ref()?.gapless_trim_spec()
    }

    fn estimated_remaining_frames(&self) -> Option<u64> {
        let decoder = self.active.as_ref()?;
        let remaining_ms = decoder
            .duration_ms_hint()?
            .saturating_sub(self.last_position_ms.max(0) as u64);
        Some(
            ((remaining_ms as u128) * decoder.spec().sample_rate.max(1) as u128 / 1000)
                .min(u64::MAX as u128) as u64,
        )
    }

    fn next_block(
        &mut self,
        out: &mut AudioBlock,
        ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError> {
        self.last_position_ms = ctx.position_ms;
        let decoder = self.active.as_mut().ok_or(PipelineError::NotPrepared)?;
        match decoder
            .next_block(self.read_frames as usize)
            .map_err(PipelineError::StageFailure)?
        {
            Some(samples) => {
                let channels = decoder.spec().channels.max(1) as usize;
                if !samples.len().is_multiple_of(channels) {
                    return Err(PipelineError::StageFailure(format!(
                        "native decoder produced misaligned block: samples={} channels={channels}",
                        samples.len()
                    )));
                }
                out.channels = decoder.spec().channels;
                out.samples = samples;
                Ok(StageFlow::Continue)
            },
            None => Ok(StageFlow::Eof),
        }
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.active = None;
    }
}

pub fn probe_track_decode_info_hybrid(
    track_token: &str,
) -> Result<HybridProbedTrackDecodeInfo, String> {
    probe_track_decode_info_hybrid_with_user_decoders(
        track_token,
        &default_user_decoder_providers(),
    )
}

pub fn probe_track_decode_info_hybrid_with_user_decoders(
    track_token: &str,
    providers: &[SharedUserDecoderProvider],
) -> Result<HybridProbedTrackDecodeInfo, String> {
    let track = decode_track_ref_token(track_token)?;
    let decoder = open_with_providers(&track.locator, providers)?;
    Ok(HybridProbedTrackDecodeInfo {
        sample_rate: decoder.spec().sample_rate,
        channels: decoder.spec().channels,
        duration_ms: decoder.duration_ms_hint(),
        metadata_json: None,
        decoder_plugin_id: None,
        decoder_type_id: None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TrackRefTokenWire {
    #[serde(rename = "source_id")]
    _source_id: String,
    #[serde(rename = "track_id")]
    _track_id: String,
    locator: String,
}

struct TrackRefToken {
    locator: String,
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
            locator: parsed.locator,
        });
    }
    Ok(TrackRefToken {
        locator: token.to_string(),
    })
}

fn normalize_ext_hint(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn ext_hint_from_path(path: &str) -> String {
    let trimmed = path.trim();
    let without_query = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    Path::new(without_query)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(normalize_ext_hint)
        .unwrap_or_default()
}

fn open_with_providers(
    locator: &str,
    providers: &[SharedUserDecoderProvider],
) -> Result<Box<dyn UserDecoderImplementation>, String> {
    let ext = ext_hint_from_path(locator);
    let mut candidates = providers
        .iter()
        .enumerate()
        .filter_map(|(index, provider)| {
            let score = if ext.is_empty() {
                Some(1)
            } else {
                provider.score_for_extension(&ext)
            }?;
            Some((score, provider.implementation_id().to_string(), index))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    if candidates.is_empty() {
        return Err(format!(
            "no native decoder candidates available for `{locator}` (ext=`{ext}`)"
        ));
    }
    let mut errors = Vec::new();
    for (_, id, index) in candidates {
        match catch_unwind(AssertUnwindSafe(|| providers[index].open(locator))) {
            Ok(Ok(decoder)) => return Ok(decoder),
            Ok(Err(error)) => {
                errors.push(format!("native decoder `{id}` open failed: {error}"));
            },
            Err(payload) => {
                errors.push(format!(
                    "native decoder `{id}` panicked while opening: {}",
                    panic_payload_message(payload.as_ref())
                ));
            },
        }
    }
    Err(errors.join("; "))
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload")
}

struct NcmUserDecoderProvider;

impl UserDecoderProvider for NcmUserDecoderProvider {
    fn implementation_id(&self) -> &str {
        "builtin.ncm.native-rust"
    }
    fn score_for_extension(&self, ext: &str) -> Option<u16> {
        ext.eq_ignore_ascii_case("ncm").then_some(100)
    }
    fn supported_extensions(&self) -> Vec<String> {
        vec!["ncm".into()]
    }
    fn open(&self, locator: &str) -> Result<Box<dyn UserDecoderImplementation>, String> {
        Ok(Box::new(NcmUserDecoderInstance(NcmDecoder::open(locator)?)))
    }
}

struct NcmUserDecoderInstance(NcmDecoder);
impl UserDecoderImplementation for NcmUserDecoderInstance {
    fn spec(&self) -> StreamSpec {
        self.0.spec()
    }
    fn duration_ms_hint(&self) -> Option<u64> {
        self.0.duration_ms_hint()
    }
    fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.0.gapless_trim_spec()
    }
    fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        self.0.seek_ms(position_ms)
    }
    fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        self.0.next_block(frames)
    }
}

struct PrebuiltUserDecoderProvider;
impl UserDecoderProvider for PrebuiltUserDecoderProvider {
    fn implementation_id(&self) -> &str {
        "builtin.symphonia"
    }
    fn score_for_extension(&self, ext: &str) -> Option<u16> {
        builtin_decoder_score_for_ext(ext)
    }
    fn supported_extensions(&self) -> Vec<String> {
        builtin_decoder_supported_extensions()
    }
    fn open(&self, locator: &str) -> Result<Box<dyn UserDecoderImplementation>, String> {
        Ok(Box::new(PrebuiltUserDecoderInstance(BuiltinDecoder::open(
            locator,
        )?)))
    }
}

struct PrebuiltUserDecoderInstance(BuiltinDecoder);
impl UserDecoderImplementation for PrebuiltUserDecoderInstance {
    fn spec(&self) -> StreamSpec {
        self.0.spec()
    }
    fn duration_ms_hint(&self) -> Option<u64> {
        self.0.effective_duration_ms_hint()
    }
    fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.0.gapless_trim_spec()
    }
    fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        self.0.seek_ms(position_ms)
    }
    fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        self.0.next_block(frames)
    }
}

struct PlaylistUserDecoderProvider;
impl UserDecoderProvider for PlaylistUserDecoderProvider {
    fn implementation_id(&self) -> &str {
        "builtin.playlist"
    }
    fn score_for_extension(&self, ext: &str) -> Option<u16> {
        matches!(ext, "m3u" | "m3u8").then_some(100)
    }
    fn supported_extensions(&self) -> Vec<String> {
        vec!["m3u".into(), "m3u8".into()]
    }
    fn open(&self, locator: &str) -> Result<Box<dyn UserDecoderImplementation>, String> {
        Ok(Box::new(PlaylistUserDecoderInstance(
            PlaylistDecoder::open(locator)?,
        )))
    }
}

struct PlaylistUserDecoderInstance(PlaylistDecoder);
impl UserDecoderImplementation for PlaylistUserDecoderInstance {
    fn spec(&self) -> StreamSpec {
        self.0.spec()
    }
    fn duration_ms_hint(&self) -> Option<u64> {
        self.0.duration_ms_hint()
    }
    fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.0.gapless_trim_spec()
    }
    fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        self.0.seek_ms(position_ms)
    }
    fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        self.0.next_block(frames)
    }
}
