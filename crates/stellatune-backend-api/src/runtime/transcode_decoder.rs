use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use stellatune_audio_builtin_adapters::builtin_decoder::{
    BuiltinDecoder, BuiltinDecoderMetadata, builtin_decoder_score_for_ext, extension_from_path,
};
use stellatune_plugins::host_runtime::RuntimeCapabilityKind;
use stellatune_plugins::host_runtime::RuntimeDecoderPlugin;
use stellatune_plugins::runtime::model::{
    RuntimeArtwork, RuntimeArtworkKind, RuntimeAudioTags, RuntimeEncodedAudioFormat,
    RuntimeMediaMetadata, RuntimePcmF32Chunk,
};

use super::shared_plugin_runtime;

#[derive(Debug, Clone)]
pub struct TranscodeDecoderInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub metadata: Option<RuntimeMediaMetadata>,
    pub decoder_plugin_id: Option<String>,
    pub decoder_type_id: Option<String>,
}

pub trait TranscodeDecoderSession: Send {
    fn info(&self) -> TranscodeDecoderInfo;
    fn read_pcm_f32(&mut self, max_frames: u32) -> Result<RuntimePcmF32Chunk, String>;
    fn close(&mut self) -> Result<(), String>;
}

pub fn open_local_transcode_decoder(
    path: &str,
) -> Result<Box<dyn TranscodeDecoderSession>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("source path is empty".to_string());
    }
    let source = Path::new(path);
    if !source.exists() {
        return Err(format!("source file does not exist: {}", source.display()));
    }
    if !source.is_file() {
        return Err(format!("source path is not a file: {}", source.display()));
    }

    let ext_hint = extension_from_path(path);
    let mut candidates = select_local_decoder_candidates(ext_hint.as_str());
    if candidates.is_empty() {
        return Err(format!(
            "no local decoder candidates available for extension `{}`",
            if ext_hint.is_empty() {
                "<none>"
            } else {
                ext_hint.as_str()
            }
        ));
    }
    sort_candidates(&mut candidates);

    let mut errors = Vec::<String>::new();
    for candidate in candidates {
        match candidate {
            TranscodeDecoderCandidate::Builtin { .. } => match open_builtin_decoder(path) {
                Ok(decoder) => return Ok(Box::new(decoder)),
                Err(error) => errors.push(error),
            },
            TranscodeDecoderCandidate::Plugin {
                plugin_id, type_id, ..
            } => match open_plugin_decoder(path, ext_hint.as_str(), &plugin_id, &type_id) {
                Ok(decoder) => return Ok(Box::new(decoder)),
                Err(error) => errors.push(error),
            },
        }
    }

    if errors.is_empty() {
        Err("failed to open local decoder".to_string())
    } else {
        Err(errors.join("; "))
    }
}

#[derive(Debug, Clone)]
enum TranscodeDecoderCandidate {
    Builtin {
        score: u16,
    },
    Plugin {
        plugin_id: String,
        type_id: String,
        score: u16,
    },
}

impl TranscodeDecoderCandidate {
    fn score(&self) -> u16 {
        match self {
            Self::Builtin { score } => *score,
            Self::Plugin { score, .. } => *score,
        }
    }
}

fn select_local_decoder_candidates(ext_hint: &str) -> Vec<TranscodeDecoderCandidate> {
    let ext = ext_hint.trim().trim_start_matches('.').to_ascii_lowercase();
    let mut out = Vec::<TranscodeDecoderCandidate>::new();
    let mut seen_plugin_candidates = HashSet::<(String, String)>::new();

    if let Some(score) = builtin_decoder_score_for_ext(ext.as_str()) {
        out.push(TranscodeDecoderCandidate::Builtin { score });
    }

    for candidate in runtime_scored_plugin_candidates(ext.as_str()) {
        let (plugin_id, type_id) = match &candidate {
            TranscodeDecoderCandidate::Plugin {
                plugin_id, type_id, ..
            } => (plugin_id.clone(), type_id.clone()),
            TranscodeDecoderCandidate::Builtin { .. } => continue,
        };
        if seen_plugin_candidates.insert((plugin_id, type_id)) {
            out.push(candidate);
        }
    }

    // Always keep a full plugin fallback list so transcode can still work when
    // extension scoring metadata is incomplete or absent.
    for candidate in runtime_all_plugin_candidates() {
        let (plugin_id, type_id) = match &candidate {
            TranscodeDecoderCandidate::Plugin {
                plugin_id, type_id, ..
            } => (plugin_id.clone(), type_id.clone()),
            TranscodeDecoderCandidate::Builtin { .. } => continue,
        };
        if seen_plugin_candidates.insert((plugin_id, type_id)) {
            out.push(candidate);
        }
    }
    out
}

fn runtime_scored_plugin_candidates(ext_hint: &str) -> Vec<TranscodeDecoderCandidate> {
    let ext = ext_hint.trim().trim_start_matches('.').to_ascii_lowercase();
    if ext.is_empty() {
        return Vec::new();
    }
    let service = shared_plugin_runtime();
    service
        .list_decoder_candidates_for_ext(ext.as_str())
        .into_iter()
        .filter_map(|candidate| {
            service
                .find_capability(
                    &candidate.plugin_id,
                    RuntimeCapabilityKind::Decoder,
                    &candidate.type_id,
                )
                .map(|_| TranscodeDecoderCandidate::Plugin {
                    plugin_id: candidate.plugin_id,
                    type_id: candidate.type_id,
                    score: candidate.score,
                })
        })
        .collect()
}

fn runtime_all_plugin_candidates() -> Vec<TranscodeDecoderCandidate> {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.decoder_capability_plugin_ids();
    plugin_ids.sort();
    let mut out = Vec::<TranscodeDecoderCandidate>::new();
    for plugin_id in plugin_ids {
        let mut capabilities = service.list_decoder_capabilities(plugin_id.as_str());
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            out.push(TranscodeDecoderCandidate::Plugin {
                plugin_id: plugin_id.clone(),
                type_id: capability.type_id,
                score: 0,
            });
        }
    }
    out
}

fn sort_candidates(candidates: &mut [TranscodeDecoderCandidate]) {
    candidates.sort_by(|a, b| {
        let kind_rank = |value: &TranscodeDecoderCandidate| match value {
            TranscodeDecoderCandidate::Builtin { .. } => 0_u8,
            TranscodeDecoderCandidate::Plugin { .. } => 1_u8,
        };

        kind_rank(a)
            .cmp(&kind_rank(b))
            .then_with(|| b.score().cmp(&a.score()))
            .then_with(|| match (a, b) {
                (
                    TranscodeDecoderCandidate::Builtin { .. },
                    TranscodeDecoderCandidate::Plugin { .. },
                ) => Ordering::Less,
                (
                    TranscodeDecoderCandidate::Plugin { .. },
                    TranscodeDecoderCandidate::Builtin { .. },
                ) => Ordering::Greater,
                (
                    TranscodeDecoderCandidate::Builtin { .. },
                    TranscodeDecoderCandidate::Builtin { .. },
                ) => Ordering::Equal,
                (
                    TranscodeDecoderCandidate::Plugin {
                        plugin_id: left_plugin_id,
                        type_id: left_type_id,
                        ..
                    },
                    TranscodeDecoderCandidate::Plugin {
                        plugin_id: right_plugin_id,
                        type_id: right_type_id,
                        ..
                    },
                ) => left_plugin_id
                    .cmp(right_plugin_id)
                    .then_with(|| left_type_id.cmp(right_type_id)),
            })
    });
}

fn open_builtin_decoder(path: &str) -> Result<BuiltinTranscodeDecoder, String> {
    let decoder = BuiltinDecoder::open(path)?;
    let spec = decoder.spec();
    let duration_ms = decoder.duration_ms_hint();
    let metadata = map_builtin_decoder_metadata(path, spec, duration_ms, decoder.metadata());
    if spec.sample_rate == 0 || spec.channels == 0 {
        return Err(format!(
            "builtin decoder returned invalid stream spec: sample_rate={} channels={}",
            spec.sample_rate, spec.channels
        ));
    }
    Ok(BuiltinTranscodeDecoder {
        decoder,
        info: TranscodeDecoderInfo {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            duration_ms,
            metadata,
            decoder_plugin_id: None,
            decoder_type_id: None,
        },
    })
}

fn map_builtin_decoder_metadata(
    path: &str,
    spec: stellatune_audio_core::pipeline::context::StreamSpec,
    duration_ms: Option<u64>,
    metadata: BuiltinDecoderMetadata,
) -> Option<RuntimeMediaMetadata> {
    let has_tags = metadata.title.is_some()
        || metadata.album.is_some()
        || !metadata.artists.is_empty()
        || !metadata.album_artists.is_empty()
        || !metadata.genres.is_empty()
        || metadata.track_number.is_some()
        || metadata.track_total.is_some()
        || metadata.disc_number.is_some()
        || metadata.disc_total.is_some()
        || metadata.year.is_some()
        || metadata.comment.is_some();
    let has_cover = metadata
        .cover_data
        .as_ref()
        .is_some_and(|bytes| !bytes.is_empty());

    if !has_tags && !has_cover {
        return None;
    }

    let artworks = match metadata.cover_data {
        Some(bytes) if !bytes.is_empty() => vec![RuntimeArtwork {
            kind: RuntimeArtworkKind::FrontCover,
            mime: metadata
                .cover_mime
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "image/jpeg".to_string()),
            description: None,
            width: None,
            height: None,
            data: bytes,
        }],
        _ => Vec::new(),
    };
    let ext = extension_from_path(path);
    let codec = if ext.is_empty() {
        "builtin".to_string()
    } else {
        ext.clone()
    };
    Some(RuntimeMediaMetadata {
        tags: RuntimeAudioTags {
            title: metadata.title,
            album: metadata.album,
            artists: metadata.artists,
            album_artists: metadata.album_artists,
            genres: metadata.genres,
            track_number: metadata.track_number,
            track_total: metadata.track_total,
            disc_number: metadata.disc_number,
            disc_total: metadata.disc_total,
            year: metadata.year,
            comment: metadata.comment,
        },
        duration_ms,
        format: RuntimeEncodedAudioFormat {
            codec,
            sample_rate: Some(spec.sample_rate),
            channels: Some(spec.channels),
            bitrate_kbps: None,
            container: (!ext.is_empty()).then_some(ext),
        },
        artworks,
        extras: Vec::new(),
    })
}

fn open_plugin_decoder(
    path: &str,
    ext_hint: &str,
    plugin_id: &str,
    type_id: &str,
) -> Result<PluginTranscodeDecoder, String> {
    let source = Path::new(path);
    let mut decoder = shared_plugin_runtime()
        .create_decoder_plugin(plugin_id, type_id)
        .map_err(|error| {
            format!(
                "create decoder plugin failed for {}::{}: {error}",
                plugin_id, type_id
            )
        })?;
    let session_handle = decoder
        .open_file(
            source,
            (!ext_hint.trim().is_empty()).then_some(ext_hint.trim()),
        )
        .map_err(|error| {
            format!(
                "decoder open_file failed for {}::{} on {}: {error}",
                plugin_id,
                type_id,
                source.display()
            )
        })?;

    let decoder_info = match decoder.info(session_handle) {
        Ok(info) => info,
        Err(error) => {
            let _ = decoder.close(session_handle);
            return Err(format!(
                "decoder info failed for {}::{} after open_file: {error}",
                plugin_id, type_id
            ));
        },
    };
    if decoder_info.sample_rate == 0 || decoder_info.channels == 0 {
        let _ = decoder.close(session_handle);
        return Err(format!(
            "decoder returned invalid stream spec for {}::{}: sample_rate={} channels={}",
            plugin_id, type_id, decoder_info.sample_rate, decoder_info.channels
        ));
    }
    let metadata = decoder.metadata(session_handle).ok();

    Ok(PluginTranscodeDecoder {
        decoder,
        session_handle: Some(session_handle),
        info: TranscodeDecoderInfo {
            sample_rate: decoder_info.sample_rate,
            channels: decoder_info.channels,
            duration_ms: decoder_info.duration_ms,
            metadata,
            decoder_plugin_id: Some(plugin_id.to_string()),
            decoder_type_id: Some(type_id.to_string()),
        },
    })
}

struct BuiltinTranscodeDecoder {
    decoder: BuiltinDecoder,
    info: TranscodeDecoderInfo,
}

impl TranscodeDecoderSession for BuiltinTranscodeDecoder {
    fn info(&self) -> TranscodeDecoderInfo {
        self.info.clone()
    }

    fn read_pcm_f32(&mut self, max_frames: u32) -> Result<RuntimePcmF32Chunk, String> {
        let channels = self.info.channels.max(1) as usize;
        let maybe_samples = self.decoder.next_block(max_frames.max(1) as usize)?;
        let Some(samples) = maybe_samples else {
            return Ok(RuntimePcmF32Chunk {
                interleaved_f32le: Vec::new(),
                frames: 0,
                eof: true,
            });
        };
        if !samples.len().is_multiple_of(channels) {
            return Err(format!(
                "builtin decoder produced misaligned block: samples={} channels={channels}",
                samples.len()
            ));
        }
        let mut bytes = Vec::<u8>::with_capacity(samples.len() * 4);
        for sample in samples {
            bytes.extend_from_slice(sample.to_le_bytes().as_slice());
        }
        let frames = (bytes.len() / 4 / channels).min(u32::MAX as usize) as u32;
        Ok(RuntimePcmF32Chunk {
            interleaved_f32le: bytes,
            frames,
            eof: false,
        })
    }

    fn close(&mut self) -> Result<(), String> {
        Ok(())
    }
}

struct PluginTranscodeDecoder {
    decoder: RuntimeDecoderPlugin,
    session_handle: Option<u64>,
    info: TranscodeDecoderInfo,
}

impl PluginTranscodeDecoder {
    fn required_session_handle(&self) -> Result<u64, String> {
        self.session_handle
            .ok_or_else(|| "plugin transcode decoder session is closed".to_string())
    }
}

impl TranscodeDecoderSession for PluginTranscodeDecoder {
    fn info(&self) -> TranscodeDecoderInfo {
        self.info.clone()
    }

    fn read_pcm_f32(&mut self, max_frames: u32) -> Result<RuntimePcmF32Chunk, String> {
        let session_handle = self.required_session_handle()?;
        self.decoder
            .read_pcm_f32(session_handle, max_frames.max(1))
            .map_err(|error| format!("plugin decoder read failed: {error}"))
    }

    fn close(&mut self) -> Result<(), String> {
        let Some(session_handle) = self.session_handle.take() else {
            return Ok(());
        };
        self.decoder
            .close(session_handle)
            .map_err(|error| format!("plugin decoder close failed: {error}"))
    }
}

impl Drop for PluginTranscodeDecoder {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::open_local_transcode_decoder;

    #[test]
    fn debug_transcode_decoder_from_env() {
        let Some(path) = std::env::var_os("STELLATUNE_DEBUG_METADATA_PATH") else {
            eprintln!("skip: STELLATUNE_DEBUG_METADATA_PATH is not set");
            return;
        };
        let path = path.to_string_lossy().to_string();
        let mut decoder = open_local_transcode_decoder(path.as_str()).expect("open decoder");
        let info = decoder.info();
        eprintln!(
            "decoder plugin={:?} type={:?} sample_rate={} channels={} duration_ms={:?}",
            info.decoder_plugin_id,
            info.decoder_type_id,
            info.sample_rate,
            info.channels,
            info.duration_ms
        );
        let metadata = info.metadata.as_ref();
        eprintln!(
            "metadata present={} title={:?} album={:?} artists={:?} track={:?}/{:?} disc={:?}/{:?} year={:?} comment_len={} artworks={}",
            metadata.is_some(),
            metadata.and_then(|m| m.tags.title.clone()),
            metadata.and_then(|m| m.tags.album.clone()),
            metadata.map(|m| m.tags.artists.clone()).unwrap_or_default(),
            metadata.and_then(|m| m.tags.track_number),
            metadata.and_then(|m| m.tags.track_total),
            metadata.and_then(|m| m.tags.disc_number),
            metadata.and_then(|m| m.tags.disc_total),
            metadata.and_then(|m| m.tags.year),
            metadata
                .and_then(|m| m.tags.comment.as_ref().map(|v| v.len()))
                .unwrap_or(0),
            metadata.map(|m| m.artworks.len()).unwrap_or(0)
        );
        assert!(metadata.is_some(), "transcode decoder metadata is empty");
        decoder.close().expect("close decoder");
    }
}
