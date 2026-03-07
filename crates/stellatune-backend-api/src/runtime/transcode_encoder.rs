use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use stellatune_plugins::host_runtime::RuntimeEncoderPlugin;
use stellatune_plugins::runtime::model::{
    RuntimeAudioSpec, RuntimeEncodeTarget, RuntimeEncodedAudioFormat, RuntimeMediaMetadata,
    RuntimePcmF32Chunk,
};

use super::shared_plugin_runtime;

const MAX_ENCODE_BYTES: u32 = 64 * 1024;

#[derive(Debug, Clone)]
pub struct TranscodeEncoderDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
    pub target_formats: Vec<TranscodeTargetFormatDescriptor>,
}

#[derive(Debug, Clone)]
pub struct TranscodeTargetFormatDescriptor {
    pub ext: String,
    pub label: String,
    pub lossless: bool,
    pub bitrate_choices_kbps: Vec<u32>,
    pub default_bitrate_kbps: Option<u32>,
    pub options_schema_json: Option<String>,
    pub default_options_json: Option<String>,
}

pub trait TranscodeEncoderSession: Send {
    fn write_pcm_f32(&mut self, chunk: RuntimePcmF32Chunk) -> Result<u32, String>;
    fn finish(&mut self) -> Result<(), String>;
    fn close(&mut self) -> Result<(), String>;
    fn written_bytes(&self) -> u64;
}

pub fn list_local_transcode_encoders() -> Vec<TranscodeEncoderDescriptor> {
    let service = shared_plugin_runtime();
    let mut out = Vec::<TranscodeEncoderDescriptor>::new();

    let mut plugins = service.active_plugins_snapshot();
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    for plugin in plugins {
        let mut capabilities = service.list_encoder_capabilities(plugin.id.as_str());
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            let type_id = capability.type_id;
            let display_name = capability.display_name;
            let config_schema_json = capability.config_schema_json;
            let default_config_json = capability.default_config_json;
            let target_formats =
                normalize_target_formats(capability.encoder_formats, type_id.as_str(), out.len());
            out.push(TranscodeEncoderDescriptor {
                plugin_id: plugin.id.clone(),
                plugin_name: plugin.name.clone(),
                type_id,
                display_name,
                config_schema_json,
                default_config_json,
                target_formats,
            });
        }
    }

    out.sort_by(|a, b| {
        a.display_name
            .to_ascii_lowercase()
            .cmp(&b.display_name.to_ascii_lowercase())
            .then_with(|| {
                a.plugin_name
                    .to_ascii_lowercase()
                    .cmp(&b.plugin_name.to_ascii_lowercase())
            })
            .then_with(|| a.type_id.cmp(&b.type_id))
            .then_with(|| a.plugin_id.cmp(&b.plugin_id))
    });
    out
}

#[allow(clippy::too_many_arguments)]
pub fn open_local_transcode_encoder(
    output_path: &str,
    encoder_plugin_id: &str,
    encoder_type_id: &str,
    sample_rate: u32,
    channels: u16,
    metadata: Option<RuntimeMediaMetadata>,
    encoder_config_json: &str,
    encoder_options_json: Option<&str>,
    target_format_ext: Option<&str>,
    target_bitrate_kbps: Option<u32>,
) -> Result<Box<dyn TranscodeEncoderSession>, String> {
    let output_path = output_path.trim();
    if output_path.is_empty() {
        return Err("output path is empty".to_string());
    }
    let plugin_id = encoder_plugin_id.trim();
    if plugin_id.is_empty() {
        return Err("encoder plugin id is empty".to_string());
    }
    let type_id = encoder_type_id.trim();
    if type_id.is_empty() {
        return Err("encoder type id is empty".to_string());
    }
    if sample_rate == 0 || channels == 0 {
        return Err(format!(
            "invalid encoder stream spec: sample_rate={} channels={channels}",
            sample_rate
        ));
    }

    open_plugin_transcode_encoder(
        output_path,
        plugin_id,
        type_id,
        sample_rate,
        channels,
        metadata,
        encoder_config_json,
        encoder_options_json,
        target_format_ext,
        target_bitrate_kbps,
    )
    .map(|encoder| Box::new(encoder) as Box<dyn TranscodeEncoderSession>)
}

#[allow(clippy::too_many_arguments)]
fn open_plugin_transcode_encoder(
    output_path: &str,
    plugin_id: &str,
    type_id: &str,
    sample_rate: u32,
    channels: u16,
    metadata: Option<RuntimeMediaMetadata>,
    encoder_config_json: &str,
    encoder_options_json: Option<&str>,
    target_format_ext: Option<&str>,
    target_bitrate_kbps: Option<u32>,
) -> Result<PluginTranscodeEncoder, String> {
    let service = shared_plugin_runtime();
    let mut encoder = service
        .create_encoder_plugin(plugin_id, type_id)
        .map_err(|error| {
            format!(
                "failed to create encoder plugin {}::{}: {error}",
                plugin_id, type_id
            )
        })?;

    let target_ext = target_format_ext
        .map(normalize_extension)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| output_path_extension(output_path));
    let target_codec = if target_ext.is_empty() {
        type_id.to_string()
    } else {
        target_ext.clone()
    };
    let options_json = encoder_options_json
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let encoder_session = encoder
        .create(
            RuntimeAudioSpec {
                sample_rate,
                channels,
            },
            RuntimeEncodeTarget {
                format: RuntimeEncodedAudioFormat {
                    codec: target_codec,
                    sample_rate: Some(sample_rate),
                    channels: Some(channels),
                    bitrate_kbps: target_bitrate_kbps.filter(|value| *value > 0),
                    container: (!target_ext.is_empty()).then_some(target_ext.clone()),
                },
                ext_hint: (!target_ext.is_empty()).then_some(target_ext),
                options_json,
            },
            metadata,
        )
        .map_err(|error| {
            format!(
                "failed to create encoder session for {}::{}: {error}",
                plugin_id, type_id
            )
        })?;

    let config_json = encoder_config_json.trim();
    if !config_json.is_empty() {
        encoder
            .apply_config_update_json(encoder_session, config_json)
            .map_err(|error| {
                format!(
                    "failed to apply encoder config for {}::{}: {error}",
                    plugin_id, type_id
                )
            })?;
    }

    let output_file = File::create(output_path)
        .map_err(|error| format!("failed to create output file `{output_path}`: {error}"))?;
    Ok(PluginTranscodeEncoder {
        encoder,
        session_handle: Some(encoder_session),
        output_writer: BufWriter::new(output_file),
        written_bytes: 0,
    })
}

struct PluginTranscodeEncoder {
    encoder: RuntimeEncoderPlugin,
    session_handle: Option<u64>,
    output_writer: BufWriter<File>,
    written_bytes: u64,
}

impl PluginTranscodeEncoder {
    fn required_session_handle(&self) -> Result<u64, String> {
        self.session_handle
            .ok_or_else(|| "plugin transcode encoder session is closed".to_string())
    }

    fn drain_output(&mut self, max_bytes: u32) -> Result<bool, String> {
        let session_handle = self.required_session_handle()?;
        let mut saw_eof = false;
        loop {
            let chunk = self
                .encoder
                .read_encoded(session_handle, max_bytes.max(1))
                .map_err(|error| format!("plugin encoder read failed: {error}"))?;
            if !chunk.bytes.is_empty() {
                self.output_writer
                    .write_all(chunk.bytes.as_slice())
                    .map_err(|error| format!("write output failed: {error}"))?;
                self.written_bytes = self.written_bytes.saturating_add(chunk.bytes.len() as u64);
            }
            if chunk.eof {
                saw_eof = true;
                break;
            }
            if chunk.bytes.is_empty() {
                break;
            }
        }
        Ok(saw_eof)
    }
}

impl TranscodeEncoderSession for PluginTranscodeEncoder {
    fn write_pcm_f32(&mut self, chunk: RuntimePcmF32Chunk) -> Result<u32, String> {
        let session_handle = self.required_session_handle()?;
        let consumed = self
            .encoder
            .write_pcm_f32(session_handle, chunk.clone())
            .map_err(|error| format!("plugin encoder write failed: {error}"))?;
        let _ = self.drain_output(MAX_ENCODE_BYTES)?;
        Ok(consumed)
    }

    fn finish(&mut self) -> Result<(), String> {
        for _ in 0..64 {
            let saw_eof = self.drain_output(MAX_ENCODE_BYTES)?;
            if saw_eof {
                break;
            }
        }
        self.output_writer
            .flush()
            .map_err(|error| format!("flush output failed: {error}"))?;
        Ok(())
    }

    fn close(&mut self) -> Result<(), String> {
        let Some(session_handle) = self.session_handle.take() else {
            return Ok(());
        };
        let _ = self.output_writer.flush();
        self.encoder
            .close(session_handle)
            .map_err(|error| format!("plugin encoder close failed: {error}"))
    }

    fn written_bytes(&self) -> u64 {
        self.written_bytes
    }
}

impl Drop for PluginTranscodeEncoder {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn normalize_extension(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn output_path_extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(normalize_extension)
        .unwrap_or_default()
}

fn normalize_target_formats(
    raw_formats: Vec<stellatune_plugins::host_runtime::RuntimeEncoderFormatDescriptor>,
    type_id: &str,
    fallback_order: usize,
) -> Vec<TranscodeTargetFormatDescriptor> {
    if !raw_formats.is_empty() {
        return raw_formats
            .into_iter()
            .map(|item| TranscodeTargetFormatDescriptor {
                ext: normalize_extension(item.ext.as_str()),
                label: item.label.trim().to_string(),
                lossless: item.lossless,
                bitrate_choices_kbps: item
                    .bitrate_choices_kbps
                    .into_iter()
                    .filter(|value| *value > 0)
                    .collect(),
                default_bitrate_kbps: item.default_bitrate_kbps.filter(|value| *value > 0),
                options_schema_json: item.options_schema_json,
                default_options_json: item.default_options_json,
            })
            .filter(|item| !item.ext.is_empty())
            .collect();
    }

    let ignored = ["encoder", "encode", "audio", "plugin"];
    let mut fallback_ext = type_id
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(normalize_extension)
        .rev()
        .find(|segment| !ignored.contains(&segment.as_str()))
        .unwrap_or_else(|| format!("out{}", fallback_order + 1));
    if fallback_ext.is_empty() {
        fallback_ext = "out".to_string();
    }
    vec![TranscodeTargetFormatDescriptor {
        ext: fallback_ext.clone(),
        label: fallback_ext.to_ascii_uppercase(),
        lossless: true,
        bitrate_choices_kbps: Vec::new(),
        default_bitrate_kbps: None,
        options_schema_json: None,
        default_options_json: None,
    }]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::Value;

    use crate::runtime::transcode_decoder::open_local_transcode_decoder;
    use crate::runtime::transcode_encoder::open_local_transcode_encoder;

    #[test]
    fn debug_transcode_mp3_tags_from_env() {
        let Some(source_path) = std::env::var_os("STELLATUNE_DEBUG_METADATA_PATH") else {
            eprintln!("skip: STELLATUNE_DEBUG_METADATA_PATH is not set");
            return;
        };
        let source_path = source_path.to_string_lossy().to_string();
        let plugin_id = std::env::var("STELLATUNE_DEBUG_ENCODER_PLUGIN")
            .unwrap_or_else(|_| "dev.stellatune.codec.ffmpeg".to_string());
        let type_id = std::env::var("STELLATUNE_DEBUG_ENCODER_TYPE")
            .unwrap_or_else(|_| "ffmpeg_encode".to_string());
        let output_path = std::env::var("STELLATUNE_DEBUG_TRANSCODE_OUTPUT").unwrap_or_else(|_| {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|v| v.as_millis())
                .unwrap_or(0);
            let mut p = std::env::temp_dir();
            p.push(format!("stellatune_debug_transcode_{ts}.mp3"));
            p.to_string_lossy().to_string()
        });
        let _ = std::fs::remove_file(output_path.as_str());

        let mut decoder = open_local_transcode_decoder(source_path.as_str())
            .expect("open local transcode decoder");
        let info = decoder.info();
        let metadata = info.metadata.clone();
        assert!(metadata.is_some(), "decoder metadata is empty");

        let mut encoder = match open_local_transcode_encoder(
            output_path.as_str(),
            plugin_id.as_str(),
            type_id.as_str(),
            info.sample_rate,
            info.channels,
            metadata,
            "{}",
            None,
            Some("mp3"),
            Some(320),
        ) {
            Ok(encoder) => encoder,
            Err(error) => {
                if error.contains("is not installed") {
                    eprintln!("skip: encoder plugin is not installed: {error}");
                    return;
                }
                panic!("open local transcode encoder failed: {error}");
            },
        };

        loop {
            let chunk = decoder.read_pcm_f32(8192).expect("decoder read");
            let consumed = encoder.write_pcm_f32(chunk.clone()).expect("encoder write");
            assert_eq!(consumed, chunk.frames, "encoder partial consume");
            if chunk.eof {
                break;
            }
        }
        encoder.finish().expect("encoder finish");
        encoder.close().expect("encoder close");
        decoder.close().expect("decoder close");

        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                output_path.as_str(),
            ])
            .output()
            .expect("run ffprobe");
        assert!(
            probe.status.success(),
            "ffprobe failed: status={:?} stderr={}",
            probe.status.code(),
            String::from_utf8_lossy(probe.stderr.as_slice())
        );
        let json: Value = serde_json::from_slice(probe.stdout.as_slice()).expect("parse ffprobe");
        let tags = json
            .get("format")
            .and_then(|v| v.get("tags"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        eprintln!(
            "debug output={} tags={}",
            PathBuf::from(output_path.as_str()).display(),
            Value::Object(tags.clone())
        );

        let has_title = tags.contains_key("title") || tags.contains_key("TITLE");
        let has_artist = tags.contains_key("artist") || tags.contains_key("ARTIST");
        let has_album = tags.contains_key("album") || tags.contains_key("ALBUM");
        assert!(
            has_title && has_artist && has_album,
            "missing expected tags in output mp3"
        );
    }
}
