use std::sync::{Arc, Mutex};
use tracing::debug;

use stellatune_core::TrackDecodeInfo;
use stellatune_decode::{Decoder, TrackSpec};

/// Built-in decoder "priority" when selecting between a plugin decoder and the built-in Symphonia
/// decoder. Plugins can return a probe score > this value to override the built-in decoder even for
/// formats the built-in decoder can handle.
const BUILTIN_DECODER_SCORE: u8 = 50;

pub(crate) enum EngineDecoder {
    Builtin(Decoder),
    Plugin {
        dec: stellatune_plugins::DecoderInstance,
        spec: TrackSpec,
    },
}

impl EngineDecoder {
    pub fn spec(&self) -> TrackSpec {
        match self {
            Self::Builtin(d) => d.spec(),
            Self::Plugin { spec, .. } => *spec,
        }
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        match self {
            Self::Builtin(d) => d.seek_ms(position_ms).map_err(|e| e.to_string()),
            Self::Plugin { dec, .. } => dec.seek_ms(position_ms).map_err(|e| e.to_string()),
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
        }
    }
}

pub(crate) fn open_engine_decoder(
    path: &str,
    plugins: &Arc<Mutex<stellatune_plugins::PluginManager>>,
) -> Result<(EngineDecoder, TrackDecodeInfo), String> {
    let Ok(pm) = plugins.lock() else {
        let d = Decoder::open(path).map_err(|e| format!("failed to open decoder: {e}"))?;
        let spec = d.spec();
        let info = TrackDecodeInfo {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            duration_ms: None,
            metadata_json: None,
            decoder_plugin_id: None,
            decoder_type_id: None,
        };
        return Ok((EngineDecoder::Builtin(d), info));
    };

    let plugin_probe = pm
        .probe_best_decoder(path)
        .map_err(|e| format!("plugin probe failed: {e:#}"))?;

    match plugin_probe {
        Some((key, score)) if score > BUILTIN_DECODER_SCORE => {
            match pm.open_decoder(key, path) {
                Ok(mut dec) => {
                    let spec = dec.spec();
                    if spec.sample_rate == 0 {
                        return Err("plugin decoder returned sample_rate=0".to_string());
                    }
                    let duration_ms = dec.duration_ms();
                    let metadata_json = dec.metadata_json().ok().flatten();
                    let info = TrackDecodeInfo {
                        sample_rate: spec.sample_rate,
                        channels: spec.channels,
                        duration_ms,
                        metadata_json,
                        decoder_plugin_id: Some(dec.plugin_id().to_string()),
                        decoder_type_id: Some(dec.decoder_type_id().to_string()),
                    };
                    return Ok((
                        EngineDecoder::Plugin {
                            spec: TrackSpec {
                                sample_rate: spec.sample_rate,
                                channels: spec.channels,
                            },
                            dec,
                        },
                        info,
                    ));
                }
                Err(e) => {
                    debug!("plugin decoder open failed (score={score}), falling back: {e:#}");
                }
            }

            let d = Decoder::open(path).map_err(|e| format!("failed to open decoder: {e}"))?;
            let spec = d.spec();
            let info = TrackDecodeInfo {
                sample_rate: spec.sample_rate,
                channels: spec.channels,
                duration_ms: None,
                metadata_json: None,
                decoder_plugin_id: None,
                decoder_type_id: None,
            };
            Ok((EngineDecoder::Builtin(d), info))
        }
        _ => match Decoder::open(path) {
            Ok(d) => {
                let spec = d.spec();
                let info = TrackDecodeInfo {
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                    duration_ms: None,
                    metadata_json: None,
                    decoder_plugin_id: None,
                    decoder_type_id: None,
                };
                Ok((EngineDecoder::Builtin(d), info))
            }
            Err(e) => {
                if let Some((key, score)) = plugin_probe {
                    debug!("built-in decoder failed, trying plugin fallback (score={score}): {e}");
                    let mut dec = pm
                        .open_decoder(key, path)
                        .map_err(|e| format!("failed to open plugin decoder: {e:#}"))?;
                    let spec = dec.spec();
                    if spec.sample_rate == 0 {
                        return Err("plugin decoder returned sample_rate=0".to_string());
                    }
                    let duration_ms = dec.duration_ms();
                    let metadata_json = dec.metadata_json().ok().flatten();
                    let info = TrackDecodeInfo {
                        sample_rate: spec.sample_rate,
                        channels: spec.channels,
                        duration_ms,
                        metadata_json,
                        decoder_plugin_id: Some(dec.plugin_id().to_string()),
                        decoder_type_id: Some(dec.decoder_type_id().to_string()),
                    };
                    return Ok((
                        EngineDecoder::Plugin {
                            spec: TrackSpec {
                                sample_rate: spec.sample_rate,
                                channels: spec.channels,
                            },
                            dec,
                        },
                        info,
                    ));
                }
                Err(format!("failed to open decoder: {e}"))
            }
        },
    }
}
