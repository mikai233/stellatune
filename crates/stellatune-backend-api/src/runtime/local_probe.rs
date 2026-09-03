use std::path::Path;

use stellatune_audio_builtin_adapters::builtin_decoder::{
    BuiltinDecoder, builtin_decoder_supported_extensions,
};
use stellatune_audio_builtin_adapters::ncm_decoder::NcmDecoder;

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
    extensions.push("ncm".to_owned());
    extensions.sort();
    extensions.dedup();
    extensions
}

pub fn probe_local_track(path: &Path) -> Result<ProbedTrackDecodeInfo, String> {
    let path_text = path.to_string_lossy();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let (sample_rate, channels, duration_ms) = if extension.eq_ignore_ascii_case("ncm") {
        let decoder = NcmDecoder::open(&path_text)?;
        let spec = decoder.spec();
        (
            spec.sample_rate,
            spec.channel_layout.channel_count(),
            decoder.duration_ms_hint(),
        )
    } else {
        let decoder = BuiltinDecoder::open(&path_text)?;
        let spec = decoder.spec();
        (
            spec.sample_rate,
            spec.channel_layout.channel_count(),
            decoder.effective_duration_ms_hint(),
        )
    };
    Ok(ProbedTrackDecodeInfo {
        sample_rate,
        channels,
        duration_ms,
        metadata_json: None,
        decoder_plugin_id: None,
        decoder_type_id: None,
    })
}
