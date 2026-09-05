use std::path::Path;

use stellatune_audio_builtin_adapters::builtin_decoder::builtin_decoder_supported_extensions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbedTrackDecodeInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub metadata_json: Option<String>,
    pub decoder_plugin_id: Option<String>,
    pub decoder_type_id: Option<String>,
}

pub fn decoder_supported_extensions() -> Vec<String> {
    let mut extensions = builtin_decoder_supported_extensions();
    extensions.extend(super::shared_typescript_runtime().local_file_extensions());
    extensions.sort();
    extensions.dedup();
    extensions
}

pub async fn probe_local_track(path: &Path) -> Result<ProbedTrackDecodeInfo, String> {
    let super::local_decoder::LocalDecoder {
        decoder,
        plugin_id,
        capability_id,
    } = super::local_decoder::open_local_decoder(path).await?;
    let spec = decoder.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channel_layout.channel_count();
    let duration_ms = decoder.effective_duration_ms_hint();
    Ok(ProbedTrackDecodeInfo {
        sample_rate,
        channels,
        duration_ms,
        metadata_json: None,
        decoder_plugin_id: plugin_id,
        decoder_type_id: capability_id,
    })
}
