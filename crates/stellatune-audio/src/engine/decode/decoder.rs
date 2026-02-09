use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::debug;

use serde::Deserialize;
use stellatune_core::{TrackDecodeInfo, TrackPlayability, TrackRef};
use stellatune_decode::{Decoder, TrackSpec, supports_path};

/// Built-in decoder "priority" when selecting between a plugin decoder and the built-in Symphonia
/// decoder. Plugins can return a probe score > this value to override the built-in decoder even for
/// formats the built-in decoder can handle.
const BUILTIN_DECODER_SCORE: u8 = 50;
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
        if let Some(ext) = Path::new(path).extension().and_then(|v| v.to_str())
            && pm.probe_best_decoder_hint(ext).is_some()
        {
            return TrackPlayability {
                track: track.clone(),
                playable: true,
                reason: None,
            };
        }
        return match pm.can_decode_path(path) {
            Ok(true) => TrackPlayability {
                track: track.clone(),
                playable: true,
                reason: None,
            },
            Ok(false) => TrackPlayability {
                track: track.clone(),
                playable: false,
                reason: Some("no_decoder_for_local_track".to_string()),
            },
            Err(_) => TrackPlayability {
                track: track.clone(),
                playable: false,
                reason: Some("decoder_probe_failed".to_string()),
            },
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

    let decoder_key = match (
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    ) {
        (Some(plugin_id), Some(type_id)) => pm.find_decoder_key(plugin_id, type_id),
        _ => pm
            .probe_best_decoder_hint(source.ext_hint.trim())
            .map(|(key, _)| key),
    };

    if decoder_key.is_none() {
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
        let Ok(pm) = snapshot_plugins(plugins) else {
            let d = Decoder::open(path).map_err(|e| format!("failed to open decoder: {e}"))?;
            let spec = d.spec();
            let info = build_builtin_track_info(spec);
            return Ok((Box::new(EngineDecoder::Builtin(d)), info));
        };

        let plugin_probe = pm
            .probe_best_decoder(path)
            .map_err(|e| format!("plugin probe failed: {e:#}"))?;

        return match plugin_probe {
            Some((key, score)) if score > BUILTIN_DECODER_SCORE => {
                match pm.open_decoder(key, path) {
                    Ok(mut dec) => {
                        debug!(
                            path,
                            plugin_id = dec.plugin_id(),
                            decoder_type_id = dec.decoder_type_id(),
                            score,
                            "using plugin decoder for local track"
                        );
                        let spec = dec.spec();
                        let info = build_plugin_track_info(&mut dec, None)?;
                        Ok((
                            Box::new(EngineDecoder::Plugin {
                                spec: TrackSpec {
                                    sample_rate: spec.sample_rate,
                                    channels: spec.channels,
                                },
                                dec,
                            }),
                            info,
                        ))
                    }
                    Err(e) => {
                        if !supports_path(path) {
                            return Err(format!(
                                "plugin decoder open failed (score={score}) and built-in decoder does not support extension for `{path}`: {e:#}"
                            ));
                        }
                        debug!("plugin decoder open failed (score={score}), falling back: {e:#}");
                        let d = Decoder::open(path)
                            .map_err(|e| format!("failed to open decoder: {e}"))?;
                        let spec = d.spec();
                        let info = build_builtin_track_info(spec);
                        debug!(path, score, "using built-in decoder fallback");
                        Ok((Box::new(EngineDecoder::Builtin(d)), info))
                    }
                }
            }
            _ => match Decoder::open(path) {
                Ok(d) => {
                    let spec = d.spec();
                    let info = build_builtin_track_info(spec);
                    debug!(path, "using built-in decoder for local track");
                    Ok((Box::new(EngineDecoder::Builtin(d)), info))
                }
                Err(e) => {
                    if let Some((key, score)) = plugin_probe {
                        debug!(
                            "built-in decoder failed, trying plugin fallback (score={score}): {e}"
                        );
                        let mut dec = pm
                            .open_decoder(key, path)
                            .map_err(|e| format!("failed to open plugin decoder: {e:#}"))?;
                        let spec = dec.spec();
                        let info = build_plugin_track_info(&mut dec, None)?;
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
                    Err(format!("failed to open decoder: {e}"))
                }
            },
        };
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
    let (stream, source_metadata) = pm
        .source_open_stream::<_, _, serde_json::Value>(source_key, &source.config, &source.track)
        .map_err(|e| format!("source open_stream failed: {e:#}"))?;
    let ext_hint = source.ext_hint.trim().to_string();
    let path_hint = if source.path_hint.trim().is_empty() {
        track.stable_key()
    } else {
        source.path_hint.trim().to_string()
    };

    let decoder_key = match (
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    ) {
        (Some(plugin_id), Some(type_id)) => {
            pm.find_decoder_key(plugin_id, type_id).ok_or_else(|| {
                format!(
                    "decoder not found for source track: plugin_id={} type_id={}",
                    plugin_id, type_id
                )
            })?
        }
        _ => pm
            .probe_best_decoder_hint(&ext_hint)
            .map(|(key, _)| key)
            .ok_or_else(|| format!("no plugin decoder found for source ext hint `{ext_hint}`"))?,
    };

    let mut dec = pm
        .open_decoder_with_source_stream(decoder_key, &path_hint, &ext_hint, stream)
        .map_err(|e| format!("failed to open decoder on source stream: {e:#}"))?;
    let spec = dec.spec();
    let info = build_plugin_track_info(&mut dec, source_metadata)?;
    Ok((
        Box::new(EngineDecoder::Plugin {
            spec: TrackSpec {
                sample_rate: spec.sample_rate,
                channels: spec.channels,
            },
            dec,
        }),
        info,
    ))
}
