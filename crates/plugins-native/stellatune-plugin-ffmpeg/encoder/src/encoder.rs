use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
use stellatune_plugin_ffmpeg_common::FfmpegPluginConfig;
use stellatune_plugin_sdk::__private::stellatune_world_encoder::stellatune::plugin::sidecar;
use stellatune_plugin_sdk::prelude::*;

const PROBE_MAX_CAPTURE_BYTES: usize = 16 * 1024;
const IO_CHUNK_BYTES: u32 = 64 * 1024;
const FFMPEG_PROCESS_TIMEOUT_MS: u32 = 300_000;
const TEMP_FILE_PREFIX: &str = "st_ffmpeg_encoder";
const MAX_EMBEDDED_ARTWORK_BYTES: usize = 12 * 1024 * 1024;
const FLAC_MAX_COMPRESSION_LEVEL: u8 = 12;
const MP3_SUPPORTED_SAMPLE_RATES: &[u32] = &[
    8_000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000,
];
const OPUS_SUPPORTED_SAMPLE_RATES: &[u32] = &[8_000, 12_000, 16_000, 24_000, 48_000];
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramProbeReport {
    pub executable: String,
    pub exit_code: Option<i32>,
    pub stdout_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryProbeReport {
    pub ffmpeg: ProgramProbeReport,
    pub ffprobe: ProgramProbeReport,
}

pub struct FfmpegEncoderSession {
    input: AudioSpec,
    target: EncodeTarget,
    metadata: Option<MediaMetadata>,
    config: FfmpegPluginConfig,
    probe_report: BinaryProbeReport,
    process: Option<sidecar::Process>,
    write_control: Option<sidecar::Channel>,
    read_control: Option<sidecar::Channel>,
    ffmpeg_executable: Option<String>,
    ffmpeg_args_preview: Option<String>,
    ffmpeg_stdout_preview: Vec<u8>,
    input_eof: bool,
    process_finalized: bool,
    output_eof: bool,
    process_exit_code: Option<i32>,
}

impl FfmpegEncoderSession {
    pub fn new(
        input: AudioSpec,
        target: EncodeTarget,
        metadata: Option<MediaMetadata>,
    ) -> SdkResult<Self> {
        let config = FfmpegPluginConfig::default();
        let probe_report = probe_sidecar_binaries(&config)?;
        Ok(Self {
            input,
            target,
            metadata,
            config,
            probe_report,
            process: None,
            write_control: None,
            read_control: None,
            ffmpeg_executable: None,
            ffmpeg_args_preview: None,
            ffmpeg_stdout_preview: Vec::new(),
            input_eof: false,
            process_finalized: false,
            output_eof: false,
            process_exit_code: None,
        })
    }

    fn frame_bytes(&self) -> usize {
        (self.input.channels.max(1) as usize).saturating_mul(4)
    }

    fn target_ext_for_artwork(&self) -> Option<String> {
        normalized_ext_hint(self.target.ext_hint.as_deref())
            .or_else(|| normalized_ext_hint(self.target.format.container.as_deref()))
    }

    fn ensure_process_started(&mut self) -> SdkResult<()> {
        if self.process.is_some() && self.write_control.is_some() && self.read_control.is_some() {
            return Ok(());
        }
        if self.input.sample_rate == 0 || self.input.channels == 0 {
            return Err(SdkError::invalid_arg(format!(
                "invalid input spec for encoder: sample_rate={} channels={}",
                self.input.sample_rate, self.input.channels
            )));
        }

        let temp_id = next_temp_id();
        let artwork_path = prepare_artwork_input_path(
            &self.config,
            self.metadata.as_ref(),
            self.target_ext_for_artwork().as_deref(),
            temp_id.as_str(),
        );

        let encode_args = compose_encode_args(
            &self.config,
            &self.input,
            &self.target,
            self.metadata.as_ref(),
            artwork_path.as_deref(),
            "pipe:0",
            "pipe:1",
        )?;
        let ffmpeg_executable = self.config.ffmpeg_executable();
        let ffmpeg_args_preview = preview_args(encode_args.as_slice());
        let spec = sidecar::LaunchSpec {
            scope: sidecar::LaunchScope::Instance,
            executable: ffmpeg_executable.clone(),
            args: encode_args,
            preferred_control: vec![sidecar::TransportOption {
                kind: sidecar::TransportKind::Stdio,
                priority: 10,
                max_frame_bytes: None,
            }],
            preferred_data: Vec::new(),
            env: Vec::new(),
        };
        let process = sidecar::launch(&spec).map_err(map_sidecar_error)?;
        let write_control = process.open_control().map_err(map_sidecar_error)?;
        let read_control = process.open_control().map_err(map_sidecar_error)?;

        self.process = Some(process);
        self.write_control = Some(write_control);
        self.read_control = Some(read_control);
        self.ffmpeg_executable = Some(ffmpeg_executable);
        self.ffmpeg_args_preview = Some(ffmpeg_args_preview);
        self.ffmpeg_stdout_preview.clear();
        self.output_eof = false;
        Ok(())
    }

    fn write_control_bytes(&mut self, data: &[u8]) -> SdkResult<()> {
        let Some(control) = self.write_control.as_ref() else {
            return Err(SdkError::internal(
                "ffmpeg encoder write control channel is not open",
            ));
        };
        let mut offset = 0usize;
        while offset < data.len() {
            let next = (offset + IO_CHUNK_BYTES as usize).min(data.len());
            let wrote = control
                .write(&data[offset..next])
                .map_err(map_sidecar_error)? as usize;
            if wrote == 0 {
                return Err(SdkError::io("ffmpeg sidecar stdin write returned 0"));
            }
            offset = offset.saturating_add(wrote);
        }
        Ok(())
    }

    fn finalize_process(&mut self) -> SdkResult<()> {
        if self.process_finalized {
            return Ok(());
        }
        if let Some(write_control) = self.write_control.take() {
            write_control.close();
        }
        let Some(process) = self.process.as_ref() else {
            self.process_finalized = true;
            self.process_exit_code = Some(0);
            return Ok(());
        };

        let exit_code = process
            .wait_exit(Some(FFMPEG_PROCESS_TIMEOUT_MS))
            .map_err(map_sidecar_error)?;
        self.process_exit_code = if let Some(code) = exit_code {
            Some(code)
        } else {
            let _ = process.terminate(300);
            None
        };
        self.process_finalized = true;
        Ok(())
    }

    fn ffmpeg_failure_error(&self) -> SdkError {
        let executable = self
            .ffmpeg_executable
            .clone()
            .unwrap_or_else(|| self.config.ffmpeg_executable());
        let args_preview = self
            .ffmpeg_args_preview
            .as_deref()
            .unwrap_or("<unknown-args>");
        let preview = preview_text(self.ffmpeg_stdout_preview.as_slice());
        SdkError::internal(format!(
            "ffmpeg command failed for `{}`: {}; args={}; preview={}{}",
            executable,
            describe_ffmpeg_exit_code(self.process_exit_code),
            args_preview,
            preview,
            stderr_capture_hint(preview.as_str())
        ))
    }
}

impl ConfigStateOps for FfmpegEncoderSession {
    fn apply_config_update_json(&mut self, new_config_json: &str) -> SdkResult<()> {
        let next =
            serde_json::from_str::<FfmpegPluginConfig>(new_config_json).map_err(|error| {
                SdkError::invalid_arg(format!("invalid ffmpeg encoder config: {error}"))
            })?;
        let probe_report = probe_sidecar_binaries(&next)?;
        self.config = next;
        self.probe_report = probe_report;
        Ok(())
    }
}

impl EncoderSession for FfmpegEncoderSession {
    fn input_spec(&self) -> AudioSpec {
        self.input
    }

    fn output_format(&self) -> SdkResult<EncodedAudioFormat> {
        Ok(self.target.format.clone())
    }

    fn write_pcm_f32(&mut self, chunk: PcmF32Chunk) -> SdkResult<u32> {
        if self.input_eof {
            return Err(SdkError::invalid_arg(
                "ffmpeg encoder does not accept pcm after eof=true chunk",
            ));
        }
        let frame_bytes = self.frame_bytes();
        if frame_bytes == 0 {
            return Err(SdkError::invalid_arg("ffmpeg encoder got zero frame size"));
        }
        if !chunk.interleaved_f32le.len().is_multiple_of(frame_bytes) {
            return Err(SdkError::invalid_arg(format!(
                "pcm bytes are misaligned: bytes={} frame_bytes={frame_bytes}",
                chunk.interleaved_f32le.len()
            )));
        }

        let actual_frames = chunk.interleaved_f32le.len() / frame_bytes;
        let actual_frames_u32 = u32::try_from(actual_frames).map_err(|_| {
            SdkError::invalid_arg(format!(
                "too many frames in one chunk: frames={} (max={})",
                actual_frames,
                u32::MAX
            ))
        })?;
        if actual_frames_u32 != chunk.frames {
            return Err(SdkError::invalid_arg(format!(
                "pcm chunk frame mismatch: declared={} actual={}",
                chunk.frames, actual_frames_u32
            )));
        }

        if !chunk.interleaved_f32le.is_empty() {
            self.ensure_process_started()?;
            self.write_control_bytes(chunk.interleaved_f32le.as_slice())?;
        }
        if chunk.eof {
            self.input_eof = true;
            self.finalize_process()?;
        }
        Ok(actual_frames_u32)
    }

    fn read_encoded(&mut self, max_bytes: u32) -> SdkResult<EncodedChunk> {
        if max_bytes == 0 {
            return Ok(EncodedChunk {
                bytes: Vec::new(),
                eof: self.output_eof,
            });
        }
        if !self.input_eof {
            return Ok(EncodedChunk {
                bytes: Vec::new(),
                eof: false,
            });
        }
        self.finalize_process()?;
        let Some(control) = self.read_control.as_ref() else {
            self.output_eof = true;
            return Ok(EncodedChunk {
                bytes: Vec::new(),
                eof: true,
            });
        };

        let bytes = control
            .read(max_bytes, Some(100))
            .map_err(map_sidecar_error)?;
        if bytes.is_empty() {
            self.output_eof = true;
            if self.process_exit_code != Some(0) {
                return Err(self.ffmpeg_failure_error());
            }
            return Ok(EncodedChunk {
                bytes: Vec::new(),
                eof: true,
            });
        }

        if self.ffmpeg_stdout_preview.len() < PROBE_MAX_CAPTURE_BYTES {
            let remaining = PROBE_MAX_CAPTURE_BYTES - self.ffmpeg_stdout_preview.len();
            let capture = bytes.len().min(remaining);
            self.ffmpeg_stdout_preview
                .extend_from_slice(&bytes[..capture]);
        }

        Ok(EncodedChunk { bytes, eof: false })
    }
}

impl Drop for FfmpegEncoderSession {
    fn drop(&mut self) {
        if let Some(control) = self.write_control.take() {
            control.close();
        }
        if let Some(control) = self.read_control.take() {
            control.close();
        }
        if let Some(process) = self.process.take() {
            let _ = process.terminate(300);
            let _ = process.wait_exit(Some(500));
        }
    }
}

pub fn probe_sidecar_binaries(config: &FfmpegPluginConfig) -> SdkResult<BinaryProbeReport> {
    let timeout_ms = config.clamped_probe_timeout_ms();
    let ffmpeg = run_probe_command(
        config.ffmpeg_executable(),
        compose_probe_args(config.normalized_ffmpeg_args()),
        timeout_ms,
    )?;
    let ffprobe = run_probe_command(
        config.ffprobe_executable(),
        compose_probe_args(config.normalized_ffprobe_args()),
        timeout_ms,
    )?;
    Ok(BinaryProbeReport { ffmpeg, ffprobe })
}

fn compose_encode_args(
    config: &FfmpegPluginConfig,
    input: &AudioSpec,
    target: &EncodeTarget,
    metadata: Option<&MediaMetadata>,
    artwork_path: Option<&str>,
    input_path: &str,
    output_path: &str,
) -> SdkResult<Vec<String>> {
    let target_ext = normalized_ext_hint(target.ext_hint.as_deref())
        .or_else(|| normalized_ext_hint(target.format.container.as_deref()))
        .unwrap_or_default();
    let ParsedEncoderOptions {
        ffmpeg_args: user_args,
        sample_rate,
        channels,
        flac_compression_level,
    } = parse_encoder_options(target.options_json.as_deref())?;
    let mut args = config.normalized_ffmpeg_args();
    args.extend([
        "-nostdin".to_string(),
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "f32le".to_string(),
        "-ar".to_string(),
        input.sample_rate.to_string(),
        "-ac".to_string(),
        input.channels.to_string(),
        "-i".to_string(),
        input_path.to_string(),
    ]);
    if let Some(artwork_path) = artwork_path {
        args.extend(["-i".to_string(), artwork_path.to_string()]);
    }
    args.extend(["-map".to_string(), "0:a:0".to_string()]);
    if artwork_path.is_some() {
        args.extend([
            "-map".to_string(),
            "1:v:0".to_string(),
            "-c:v".to_string(),
            "copy".to_string(),
            "-disposition:v:0".to_string(),
            "attached_pic".to_string(),
            "-metadata:s:v:0".to_string(),
            "title=Cover".to_string(),
            "-metadata:s:v:0".to_string(),
            "comment=Cover (front)".to_string(),
        ]);
    }
    if target_ext == "mp3" {
        // Keep MP3 tags broadly readable (Windows Explorer and older players).
        args.extend([
            "-id3v2_version".to_string(),
            "3".to_string(),
            "-write_id3v1".to_string(),
            "1".to_string(),
        ]);
    }

    if let Some(sample_rate) = sample_rate.or_else(|| normalized_sample_rate_for_ffmpeg(target)) {
        validate_sample_rate_for_target(target, sample_rate)?;
        args.extend(["-ar".to_string(), sample_rate.to_string()]);
    }
    if let Some(channels) = channels.or_else(|| target.format.channels.filter(|value| *value > 0)) {
        args.extend(["-ac".to_string(), channels.to_string()]);
    }
    if let Some(bitrate) = normalized_bitrate_for_ffmpeg(target) {
        args.extend(["-b:a".to_string(), format!("{bitrate}k")]);
    }
    if let Some(codec) =
        normalized_codec_for_ffmpeg(target).or_else(|| default_codec_for_target(target))
    {
        args.extend(["-c:a".to_string(), codec]);
    }
    if target_ext == "flac"
        && let Some(level) = flac_compression_level
    {
        args.extend(["-compression_level".to_string(), level.to_string()]);
    }
    append_metadata_args(&mut args, metadata);

    args.extend(user_args);
    if let Some(container_muxer) = container_muxer_for_ext(target_ext.as_str()) {
        if container_muxer == "mp4" && output_path.starts_with("pipe:") {
            args.extend([
                "-movflags".to_string(),
                "+frag_keyframe+empty_moov+default_base_moof".to_string(),
            ]);
        }
        args.extend(["-f".to_string(), container_muxer.to_string()]);
    }
    args.push(output_path.to_string());
    Ok(args)
}

fn append_metadata_args(args: &mut Vec<String>, metadata: Option<&MediaMetadata>) {
    let Some(metadata) = metadata else {
        return;
    };

    let tags = &metadata.tags;
    append_metadata_entry(args, "title", tags.title.as_deref());
    append_metadata_entry(args, "album", tags.album.as_deref());
    append_metadata_entry(
        args,
        "artist",
        joined_tag_values(tags.artists.as_slice()).as_deref(),
    );
    append_metadata_entry(
        args,
        "album_artist",
        joined_tag_values(tags.album_artists.as_slice()).as_deref(),
    );
    append_metadata_entry(
        args,
        "genre",
        joined_tag_values(tags.genres.as_slice()).as_deref(),
    );

    if let Some(track_number) = tags.track_number.filter(|value| *value > 0) {
        let value = match tags.track_total.filter(|value| *value > 0) {
            Some(track_total) => format!("{track_number}/{track_total}"),
            None => track_number.to_string(),
        };
        append_metadata_entry(args, "track", Some(value.as_str()));
    }
    if let Some(disc_number) = tags.disc_number.filter(|value| *value > 0) {
        let value = match tags.disc_total.filter(|value| *value > 0) {
            Some(disc_total) => format!("{disc_number}/{disc_total}"),
            None => disc_number.to_string(),
        };
        append_metadata_entry(args, "disc", Some(value.as_str()));
    }
    if let Some(year) = tags.year.filter(|value| *value > 0) {
        let value = year.to_string();
        append_metadata_entry(args, "date", Some(value.as_str()));
    }
    append_metadata_entry(args, "comment", tags.comment.as_deref());
}

fn append_metadata_entry(args: &mut Vec<String>, key: &str, value: Option<&str>) {
    let Some(value) = value.map(str::trim).filter(|item| !item.is_empty()) else {
        return;
    };
    args.push("-metadata".to_string());
    args.push(format!("{key}={value}"));
}

fn joined_tag_values(values: &[String]) -> Option<String> {
    let normalized = values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<&str>>();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.join("; "))
    }
}

fn prepare_artwork_input_path(
    config: &FfmpegPluginConfig,
    metadata: Option<&MediaMetadata>,
    target_ext: Option<&str>,
    temp_id: &str,
) -> Option<String> {
    let target_ext = target_ext?;
    if !target_supports_embedded_artwork(target_ext) {
        return None;
    }
    let artwork = select_cover_artwork(metadata?)?;
    if artwork.data.is_empty() || artwork.data.len() > MAX_EMBEDDED_ARTWORK_BYTES {
        return None;
    }

    let ext = artwork_file_extension(artwork).unwrap_or("png");
    let output_path = format!("{TEMP_FILE_PREFIX}_{temp_id}_cover.{ext}");
    match write_binary_file_via_ffmpeg(config, output_path.as_str(), artwork.data.as_slice()) {
        Ok(()) => Some(output_path),
        Err(_) => None,
    }
}

fn target_supports_embedded_artwork(ext: &str) -> bool {
    matches!(ext, "mp3" | "m4a" | "mp4" | "flac")
}

fn select_cover_artwork(metadata: &MediaMetadata) -> Option<&Artwork> {
    metadata
        .artworks
        .iter()
        .find(|artwork| artwork.kind == ArtworkKind::FrontCover && !artwork.data.is_empty())
        .or_else(|| {
            metadata
                .artworks
                .iter()
                .find(|artwork| !artwork.data.is_empty())
        })
}

fn artwork_file_extension(artwork: &Artwork) -> Option<&'static str> {
    let mime = artwork.mime.trim().to_ascii_lowercase();
    match mime.as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/bmp" => Some("bmp"),
        "image/gif" => Some("gif"),
        _ => sniff_image_extension(artwork.data.as_slice()),
    }
}

fn sniff_image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 8 && bytes[..8] == [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'] {
        return Some("png");
    }
    if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        return Some("jpg");
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    if bytes.len() >= 2 && bytes[..2] == [b'B', b'M'] {
        return Some("bmp");
    }
    if bytes.len() >= 6 && (&bytes[..6] == b"GIF87a" || &bytes[..6] == b"GIF89a") {
        return Some("gif");
    }
    None
}

#[derive(Debug, Clone, Default)]
struct ParsedEncoderOptions {
    ffmpeg_args: Vec<String>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    flac_compression_level: Option<u8>,
}

fn parse_encoder_options(raw_options_json: Option<&str>) -> SdkResult<ParsedEncoderOptions> {
    let Some(raw) = raw_options_json
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(ParsedEncoderOptions::default());
    };

    if let Ok(args) = serde_json::from_str::<Vec<String>>(raw) {
        return Ok(ParsedEncoderOptions {
            ffmpeg_args: normalized_arg_list(args.as_slice()),
            ..ParsedEncoderOptions::default()
        });
    }

    #[derive(Debug, Deserialize)]
    struct EncoderJsonOptions {
        #[serde(default)]
        ffmpeg_args: Vec<String>,
        #[serde(default)]
        sample_rate: Option<u32>,
        #[serde(default)]
        channels: Option<u16>,
        #[serde(default)]
        flac_compression_level: Option<u8>,
    }

    let parsed = serde_json::from_str::<EncoderJsonOptions>(raw).map_err(|error| {
        SdkError::invalid_arg(format!(
            "invalid ffmpeg encoder options_json, expected [\"...\"] or object keys {{ffmpeg_args,sample_rate,channels,flac_compression_level}}: {error}"
        ))
    })?;
    if parsed.sample_rate == Some(0) {
        return Err(SdkError::invalid_arg(
            "ffmpeg encoder options sample_rate must be > 0",
        ));
    }
    if parsed.channels == Some(0) {
        return Err(SdkError::invalid_arg(
            "ffmpeg encoder options channels must be > 0",
        ));
    }
    if let Some(level) = parsed.flac_compression_level
        && level > FLAC_MAX_COMPRESSION_LEVEL
    {
        return Err(SdkError::invalid_arg(format!(
            "ffmpeg encoder options flac_compression_level must be between 0 and {FLAC_MAX_COMPRESSION_LEVEL}"
        )));
    }
    Ok(ParsedEncoderOptions {
        ffmpeg_args: normalized_arg_list(parsed.ffmpeg_args.as_slice()),
        sample_rate: parsed.sample_rate,
        channels: parsed.channels,
        flac_compression_level: parsed.flac_compression_level,
    })
}

fn normalized_bitrate_for_ffmpeg(target: &EncodeTarget) -> Option<u32> {
    target.format.bitrate_kbps.filter(|value| *value > 0)
}

fn normalized_sample_rate_for_ffmpeg(target: &EncodeTarget) -> Option<u32> {
    let sample_rate = target.format.sample_rate.filter(|value| *value > 0)?;
    if !target_uses_opus(target) {
        return Some(sample_rate);
    }
    match sample_rate {
        8_000 | 12_000 | 16_000 | 24_000 | 48_000 => Some(sample_rate),
        _ => Some(48_000),
    }
}

fn validate_sample_rate_for_target(target: &EncodeTarget, sample_rate: u32) -> SdkResult<()> {
    if target_uses_mp3(target) {
        return validate_sample_rate(
            sample_rate,
            MP3_SUPPORTED_SAMPLE_RATES,
            "mp3 sample_rate",
            "mp3 sample rates",
        );
    }
    if target_uses_opus(target) {
        return validate_sample_rate(
            sample_rate,
            OPUS_SUPPORTED_SAMPLE_RATES,
            "opus sample_rate",
            "opus sample rates",
        );
    }
    Ok(())
}

fn validate_sample_rate(
    sample_rate: u32,
    allowed: &[u32],
    field_label: &str,
    type_label: &str,
) -> SdkResult<()> {
    if allowed.contains(&sample_rate) {
        return Ok(());
    }
    let allowed_text = allowed
        .iter()
        .map(u32::to_string)
        .collect::<Vec<String>>()
        .join(", ");
    Err(SdkError::invalid_arg(format!(
        "unsupported {field_label}={sample_rate}; supported {type_label}: {allowed_text}"
    )))
}

fn normalized_arg_list(raw: &[String]) -> Vec<String> {
    raw.iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalized_codec_for_ffmpeg(target: &EncodeTarget) -> Option<String> {
    let codec = target.format.codec.trim();
    if codec.is_empty() {
        return None;
    }
    let normalized_codec = codec.to_ascii_lowercase();
    let ext_hint = normalized_ext_hint(target.ext_hint.as_deref());
    let container = normalized_ext_hint(target.format.container.as_deref());
    if ext_hint.as_ref() == Some(&normalized_codec) || container.as_ref() == Some(&normalized_codec)
    {
        return None;
    }
    Some(codec.to_string())
}

fn default_codec_for_target(target: &EncodeTarget) -> Option<String> {
    let ext = normalized_ext_hint(target.ext_hint.as_deref())
        .or_else(|| normalized_ext_hint(target.format.container.as_deref()))?;
    match ext.as_str() {
        "mp3" => Some("libmp3lame".to_string()),
        "m4a" | "mp4" => Some("aac".to_string()),
        "opus" => Some("libopus".to_string()),
        "ogg" => Some("libvorbis".to_string()),
        "flac" => Some("flac".to_string()),
        "wav" => Some("pcm_s16le".to_string()),
        _ => None,
    }
}

fn target_uses_opus(target: &EncodeTarget) -> bool {
    normalized_ext_hint(target.ext_hint.as_deref()).as_deref() == Some("opus")
        || normalized_ext_hint(target.format.container.as_deref()).as_deref() == Some("opus")
        || target.format.codec.trim().eq_ignore_ascii_case("opus")
        || target.format.codec.trim().eq_ignore_ascii_case("libopus")
}

fn target_uses_mp3(target: &EncodeTarget) -> bool {
    normalized_ext_hint(target.ext_hint.as_deref()).as_deref() == Some("mp3")
        || normalized_ext_hint(target.format.container.as_deref()).as_deref() == Some("mp3")
        || target.format.codec.trim().eq_ignore_ascii_case("mp3")
        || target
            .format
            .codec
            .trim()
            .eq_ignore_ascii_case("libmp3lame")
}

fn normalized_ext_hint(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn next_temp_id() -> String {
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{id:016x}")
}

fn compose_probe_args(extra_args: Vec<String>) -> Vec<String> {
    let mut args = vec!["-hide_banner".to_string(), "-version".to_string()];
    args.extend(extra_args);
    args
}

fn run_probe_command(
    executable: String,
    args: Vec<String>,
    timeout_ms: u32,
) -> SdkResult<ProgramProbeReport> {
    let args_preview = preview_args(args.as_slice());
    let spec = sidecar::LaunchSpec {
        scope: sidecar::LaunchScope::Instance,
        executable: executable.clone(),
        args,
        preferred_control: vec![sidecar::TransportOption {
            kind: sidecar::TransportKind::Stdio,
            priority: 10,
            max_frame_bytes: None,
        }],
        preferred_data: Vec::new(),
        env: Vec::new(),
    };

    let process = sidecar::launch(&spec).map_err(map_sidecar_error)?;
    let control = process.open_control().map_err(map_sidecar_error)?;
    let exit_code = process
        .wait_exit(Some(timeout_ms))
        .map_err(map_sidecar_error)?;
    if exit_code.is_none() {
        let _ = process.terminate(300);
        return Err(SdkError::timeout(format!(
            "ffmpeg sidecar probe timed out for `{}`",
            executable
        )));
    }

    let mut output = Vec::<u8>::new();
    loop {
        let chunk = control
            .read(IO_CHUNK_BYTES, Some(25))
            .map_err(map_sidecar_error)?;
        if chunk.is_empty() {
            break;
        }
        let remaining = PROBE_MAX_CAPTURE_BYTES.saturating_sub(output.len());
        if remaining == 0 {
            continue;
        }
        if chunk.len() <= remaining {
            output.extend_from_slice(chunk.as_slice());
        } else {
            output.extend_from_slice(&chunk[..remaining]);
        }
    }
    control.close();

    if exit_code != Some(0) {
        let preview = preview_text(output.as_slice());
        return Err(SdkError::internal(format!(
            "probe command failed for `{}`: {}; args={}; preview={}{}",
            executable,
            describe_ffmpeg_exit_code(exit_code),
            args_preview,
            preview,
            stderr_capture_hint(preview.as_str())
        )));
    }

    Ok(ProgramProbeReport {
        executable,
        exit_code,
        stdout_preview: preview_text(output.as_slice()),
    })
}

fn write_binary_file_via_ffmpeg(
    config: &FfmpegPluginConfig,
    output_path: &str,
    data: &[u8],
) -> SdkResult<()> {
    let mut args = config.normalized_ffmpeg_args();
    args.extend([
        "-nostdin".to_string(),
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "data".to_string(),
        "-i".to_string(),
        "pipe:0".to_string(),
        "-map".to_string(),
        "0:0".to_string(),
        "-c".to_string(),
        "copy".to_string(),
        "-f".to_string(),
        "data".to_string(),
        output_path.to_string(),
    ]);
    run_command_with_stdin_and_quiet_stdout(config.ffmpeg_executable(), args, data)
}

fn container_muxer_for_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "mp3" => Some("mp3"),
        "m4a" | "mp4" => Some("mp4"),
        "opus" => Some("opus"),
        "ogg" => Some("ogg"),
        "flac" => Some("flac"),
        "wav" => Some("wav"),
        _ => None,
    }
}

fn run_command_with_stdin_and_quiet_stdout(
    executable: String,
    args: Vec<String>,
    input: &[u8],
) -> SdkResult<()> {
    let executable_preview = executable.clone();
    let args_preview = preview_args(args.as_slice());
    let spec = sidecar::LaunchSpec {
        scope: sidecar::LaunchScope::Instance,
        executable,
        args,
        preferred_control: vec![sidecar::TransportOption {
            kind: sidecar::TransportKind::Stdio,
            priority: 10,
            max_frame_bytes: None,
        }],
        preferred_data: Vec::new(),
        env: Vec::new(),
    };

    let process = sidecar::launch(&spec).map_err(map_sidecar_error)?;
    let write_control = process.open_control().map_err(map_sidecar_error)?;

    let mut offset = 0usize;
    while offset < input.len() {
        let next = (offset + IO_CHUNK_BYTES as usize).min(input.len());
        let wrote = write_control
            .write(&input[offset..next])
            .map_err(map_sidecar_error)? as usize;
        if wrote == 0 {
            return Err(SdkError::io("ffmpeg sidecar stdin write returned 0"));
        }
        offset = offset.saturating_add(wrote);
    }

    write_control.close();
    let exit_code = process
        .wait_exit(Some(FFMPEG_PROCESS_TIMEOUT_MS))
        .map_err(map_sidecar_error)?;
    if exit_code != Some(0) {
        if exit_code.is_none() {
            let _ = process.terminate(300);
        }
        return Err(SdkError::internal(format!(
            "ffmpeg command failed for `{}`: {}; args={}; preview=<no-output>; note=ffmpeg stderr is not captured by current sidecar transport",
            executable_preview,
            describe_ffmpeg_exit_code(exit_code),
            args_preview,
        )));
    }
    Ok(())
}

fn preview_text(bytes: &[u8]) -> String {
    let used = if bytes.len() <= PROBE_MAX_CAPTURE_BYTES {
        bytes
    } else {
        &bytes[..PROBE_MAX_CAPTURE_BYTES]
    };
    let text = String::from_utf8_lossy(used);
    let first_line = text.lines().next().unwrap_or_default().trim().to_string();
    if first_line.is_empty() {
        "<no-output>".to_string()
    } else {
        first_line
    }
}

fn preview_args(args: &[String]) -> String {
    const MAX_ARGS: usize = 24;
    if args.is_empty() {
        return "<none>".to_string();
    }
    let mut items = args
        .iter()
        .take(MAX_ARGS)
        .map(|arg| {
            if arg.is_empty() || arg.chars().any(char::is_whitespace) {
                format!("\"{arg}\"")
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<String>>();
    if args.len() > MAX_ARGS {
        items.push("...".to_string());
    }
    items.join(" ")
}

fn describe_ffmpeg_exit_code(exit_code: Option<i32>) -> String {
    match exit_code {
        Some(0) => "success (0)".to_string(),
        Some(-2) => "no such file or directory (-2)".to_string(),
        Some(-12) => "out of memory (-12)".to_string(),
        Some(-13) => "permission denied (-13)".to_string(),
        Some(-22) => "invalid argument (-22)".to_string(),
        Some(-38) => "function not implemented (-38)".to_string(),
        Some(code) if code < 0 => format!("negative status ({code})"),
        Some(code) => format!("process exit code {code}"),
        None => "no exit code (timeout or process did not report status)".to_string(),
    }
}

fn stderr_capture_hint(preview: &str) -> &'static str {
    if preview == "<no-output>" {
        "; note=ffmpeg stderr is not captured by current sidecar transport"
    } else {
        ""
    }
}

fn map_sidecar_error(error: sidecar::PluginError) -> SdkError {
    match error {
        sidecar::PluginError::InvalidArg(message) => SdkError::invalid_arg(message),
        sidecar::PluginError::NotFound(message) => SdkError::not_found(message),
        sidecar::PluginError::Io(message) => SdkError::io(message),
        sidecar::PluginError::Timeout(message) => SdkError::timeout(message),
        sidecar::PluginError::Unsupported(message) => SdkError::unsupported(message),
        sidecar::PluginError::Denied(message) => SdkError::denied(message),
        sidecar::PluginError::Internal(message) => SdkError::internal(message),
    }
}
