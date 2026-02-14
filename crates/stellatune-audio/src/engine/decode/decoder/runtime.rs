use crossbeam_channel::Receiver;
use serde::Deserialize;

use stellatune_core::{TrackDecodeInfo, TrackRef};
use stellatune_decode::TrackSpec;
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_endpoint::DecoderWorkerController;

use super::gapless::GaplessTrimSpec;
use super::io::DecoderIoOwner;

mod candidates;
mod open;

const TRACK_REF_TOKEN_PREFIX: &str = "stref-json:";

#[derive(Debug, Deserialize)]
pub(super) struct SourceStreamLocator {
    pub(super) plugin_id: String,
    pub(super) type_id: String,
    pub(super) config: serde_json::Value,
    pub(super) track: serde_json::Value,
    #[serde(default)]
    pub(super) ext_hint: String,
    #[serde(default)]
    pub(super) path_hint: String,
    #[serde(default)]
    pub(super) decoder_plugin_id: Option<String>,
    #[serde(default)]
    pub(super) decoder_type_id: Option<String>,
}

#[derive(Debug, Clone)]
struct DecoderCandidate {
    plugin_id: String,
    type_id: String,
    default_config_json: String,
}

pub(super) type PluginOpenDecoder = (
    DecoderWorkerController,
    TrackDecodeInfo,
    GaplessTrimSpec,
    DecoderIoOwner,
    Receiver<WorkerControlMessage>,
);

pub(super) fn decode_engine_track_token(token: &str) -> Result<TrackRef, String> {
    if let Some(json) = token.strip_prefix(TRACK_REF_TOKEN_PREFIX) {
        return serde_json::from_str::<TrackRef>(json)
            .map_err(|e| format!("invalid track ref token: {e}"));
    }
    Ok(TrackRef::for_local_path(token.to_string()))
}

pub(super) fn build_builtin_track_info(spec: TrackSpec) -> TrackDecodeInfo {
    TrackDecodeInfo {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        duration_ms: None,
        metadata_json: None,
        decoder_plugin_id: None,
        decoder_type_id: None,
    }
}

pub(super) fn ext_hint_from_path(path: &str) -> String {
    candidates::ext_hint_from_path(path)
}

fn select_decoder_candidates(
    ext_hint: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> Result<Vec<DecoderCandidate>, String> {
    candidates::select_decoder_candidates(ext_hint, decoder_plugin_id, decoder_type_id)
}

pub(super) fn has_decoder_candidates(
    ext_hint: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> bool {
    candidates::has_decoder_candidates(ext_hint, decoder_plugin_id, decoder_type_id)
}

pub(super) fn try_open_decoder_for_local_path(
    path: &str,
    ext_hint: &str,
) -> Result<Option<PluginOpenDecoder>, String> {
    open::try_open_decoder_for_local_path(path, ext_hint)
}

pub(super) fn try_open_decoder_for_source_stream(
    source: &SourceStreamLocator,
    path_hint: &str,
    ext_hint: &str,
) -> Result<Option<PluginOpenDecoder>, String> {
    open::try_open_decoder_for_source_stream(source, path_hint, ext_hint)
}

pub(super) fn runtime_has_source_catalog(plugin_id: &str, type_id: &str) -> bool {
    candidates::runtime_has_source_catalog(plugin_id, type_id)
}
