use serde::Deserialize;
use serde_json::Value;
use stellatune_plugin_ffmpeg_common::FfmpegPluginConfig;
use stellatune_plugin_sdk::__private::stellatune_world_decoder::stellatune::plugin::sidecar;
use stellatune_plugin_sdk::prelude::*;

const READ_STREAM_CHUNK_BYTES: u32 = 64 * 1024;
const READ_OUTPUT_CHUNK_BYTES: u32 = 64 * 1024;
const PROBE_MAX_CAPTURE_BYTES: usize = 16 * 1024;
const FFMPEG_PROCESS_TIMEOUT_MS: u32 = 30_000;
const MAX_EMBEDDED_ARTWORK_BYTES: usize = 12 * 1024 * 1024;

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

pub struct FfmpegDecoderSession {
    info: DecoderInfo,
    metadata: MediaMetadata,
    pcm_f32le: Vec<u8>,
    cursor_bytes: usize,
    config: FfmpegPluginConfig,
    probe_report: BinaryProbeReport,
}

impl FfmpegDecoderSession {
    pub fn open(input: DecoderInput) -> SdkResult<Self> {
        let DecoderInput {
            mut stream,
            ext_hint,
        } = input;
        let config = FfmpegPluginConfig::default();
        let probe_report = probe_sidecar_binaries(&config)?;

        let input_bytes = read_all_input_bytes(stream.as_mut())?;
        if input_bytes.is_empty() {
            return Err(SdkError::invalid_arg(
                "ffmpeg decoder input stream is empty",
            ));
        }

        let ext = normalized_ext_hint(ext_hint.as_deref()).unwrap_or_else(|| "bin".to_string());
        let audio_probe = probe_audio_stream_from_bytes(&config, input_bytes.as_slice())?;
        let artworks = extract_embedded_artworks_from_bytes(&config, input_bytes.as_slice())
            .unwrap_or_default();
        let pcm_f32le = decode_bytes_to_pcm_f32le(&config, input_bytes.as_slice())?;

        let frame_bytes = (audio_probe.channels as usize).saturating_mul(4);
        if frame_bytes == 0 {
            return Err(SdkError::internal("ffprobe returned invalid channel count"));
        }
        if !pcm_f32le.len().is_multiple_of(frame_bytes) {
            return Err(SdkError::internal(format!(
                "decoded pcm size is misaligned: bytes={} frame_bytes={frame_bytes}",
                pcm_f32le.len()
            )));
        }

        let decoded_frames = (pcm_f32le.len() / frame_bytes) as u64;
        let fallback_duration_ms = if audio_probe.sample_rate == 0 {
            None
        } else {
            Some(
                decoded_frames
                    .saturating_mul(1000)
                    .saturating_div(audio_probe.sample_rate as u64),
            )
        };

        let duration_ms = audio_probe.duration_ms.or(fallback_duration_ms);
        let codec = audio_probe
            .codec_name
            .clone()
            .unwrap_or_else(|| ext.clone());
        let container = audio_probe.container_name.clone();

        let metadata = MediaMetadata {
            tags: AudioTags {
                title: audio_probe.title.clone(),
                album: audio_probe.album.clone(),
                artists: audio_probe.artist.clone().into_iter().collect(),
                ..AudioTags::default()
            },
            duration_ms,
            format: EncodedAudioFormat {
                codec,
                sample_rate: Some(audio_probe.sample_rate),
                channels: Some(audio_probe.channels),
                bitrate_kbps: audio_probe.bitrate_kbps,
                container,
            },
            artworks,
            extras: vec![MetadataEntry {
                key: "probe".to_string(),
                value: MetadataValue::Text(format!(
                    "ffmpeg={} exit={:?}; ffprobe={} exit={:?}",
                    probe_report.ffmpeg.stdout_preview,
                    probe_report.ffmpeg.exit_code,
                    probe_report.ffprobe.stdout_preview,
                    probe_report.ffprobe.exit_code
                )),
            }],
        };

        let info = DecoderInfo {
            sample_rate: audio_probe.sample_rate,
            channels: audio_probe.channels,
            duration_ms,
            seekable: true,
            encoder_delay_frames: 0,
            encoder_padding_frames: 0,
        };

        Ok(Self {
            info,
            metadata,
            pcm_f32le,
            cursor_bytes: 0,
            config,
            probe_report,
        })
    }

    fn frame_bytes(&self) -> usize {
        (self.info.channels.max(1) as usize).saturating_mul(4)
    }
}

impl ConfigStateOps for FfmpegDecoderSession {
    fn apply_config_update_json(&mut self, new_config_json: &str) -> SdkResult<()> {
        let next =
            serde_json::from_str::<FfmpegPluginConfig>(new_config_json).map_err(|error| {
                SdkError::invalid_arg(format!("invalid ffmpeg decoder config: {error}"))
            })?;
        let probe_report = probe_sidecar_binaries(&next)?;
        self.config = next;
        self.probe_report = probe_report;
        Ok(())
    }
}

impl DecoderSession for FfmpegDecoderSession {
    fn info(&self) -> SdkResult<DecoderInfo> {
        Ok(self.info.clone())
    }

    fn metadata(&self) -> SdkResult<MediaMetadata> {
        Ok(self.metadata.clone())
    }

    fn read_pcm_f32(&mut self, max_frames: u32) -> SdkResult<PcmF32Chunk> {
        let frame_bytes = self.frame_bytes();
        if max_frames == 0 || frame_bytes == 0 {
            return Ok(PcmF32Chunk {
                interleaved_f32le: Vec::new(),
                frames: 0,
                eof: self.cursor_bytes >= self.pcm_f32le.len(),
            });
        }

        let remaining = self.pcm_f32le.len().saturating_sub(self.cursor_bytes);
        if remaining == 0 {
            return Ok(PcmF32Chunk {
                interleaved_f32le: Vec::new(),
                frames: 0,
                eof: true,
            });
        }

        let want_bytes = (max_frames as usize).saturating_mul(frame_bytes);
        let take_bytes = want_bytes.min(remaining);
        let take_frames = take_bytes / frame_bytes;
        let exact_take_bytes = take_frames.saturating_mul(frame_bytes);

        let start = self.cursor_bytes;
        let end = start.saturating_add(exact_take_bytes);
        let interleaved_f32le = self.pcm_f32le[start..end].to_vec();
        self.cursor_bytes = end;

        Ok(PcmF32Chunk {
            interleaved_f32le,
            frames: take_frames as u32,
            eof: self.cursor_bytes >= self.pcm_f32le.len(),
        })
    }

    fn seek_ms(&mut self, position_ms: u64) -> SdkResult<()> {
        let frame_bytes = self.frame_bytes();
        if frame_bytes == 0 || self.info.sample_rate == 0 {
            self.cursor_bytes = 0;
            return Ok(());
        }

        let target_frames = position_ms
            .saturating_mul(self.info.sample_rate as u64)
            .saturating_div(1000);
        let total_frames = (self.pcm_f32le.len() / frame_bytes) as u64;
        let clamped_frames = target_frames.min(total_frames);
        self.cursor_bytes = (clamped_frames as usize).saturating_mul(frame_bytes);
        Ok(())
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

fn normalized_ext_hint(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn read_all_input_bytes(stream: &mut dyn DecoderInputStream) -> SdkResult<Vec<u8>> {
    let mut out = Vec::<u8>::new();
    loop {
        let chunk = stream.read(READ_STREAM_CHUNK_BYTES)?;
        if chunk.is_empty() {
            break;
        }
        out.extend_from_slice(chunk.as_slice());
    }
    Ok(out)
}

fn decode_bytes_to_pcm_f32le(
    config: &FfmpegPluginConfig,
    input_bytes: &[u8],
) -> SdkResult<Vec<u8>> {
    let mut args = config.normalized_ffmpeg_args();
    args.extend([
        "-nostdin".to_string(),
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        "pipe:0".to_string(),
        "-vn".to_string(),
        "-map".to_string(),
        "0:a:0".to_string(),
        "-f".to_string(),
        "f32le".to_string(),
        "-acodec".to_string(),
        "pcm_f32le".to_string(),
        "pipe:1".to_string(),
    ]);
    run_command_with_stdin_capture_stdout_with_timeout(
        config.ffmpeg_executable(),
        args,
        input_bytes,
        FFMPEG_PROCESS_TIMEOUT_MS,
    )
}

fn probe_audio_stream_from_bytes(
    config: &FfmpegPluginConfig,
    input_bytes: &[u8],
) -> SdkResult<AudioProbe> {
    let mut args = config.normalized_ffprobe_args();
    args.extend([
        "-v".to_string(),
        "error".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_streams".to_string(),
        "-show_format".to_string(),
        "-select_streams".to_string(),
        "a:0".to_string(),
        "-i".to_string(),
        "pipe:0".to_string(),
    ]);

    let payload = run_command_with_stdin_capture_stdout_small(
        config.ffprobe_executable(),
        args,
        input_bytes,
        FFMPEG_PROCESS_TIMEOUT_MS,
    )?;
    let parsed: FfprobeOutput = serde_json::from_slice(payload.as_slice())
        .map_err(|error| SdkError::internal(format!("decode ffprobe json failed: {error}")))?;
    let stream = parsed
        .streams
        .into_iter()
        .next()
        .ok_or_else(|| SdkError::unsupported("ffprobe did not return an audio stream"))?;

    let sample_rate = stream
        .sample_rate
        .as_deref()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| SdkError::unsupported("ffprobe missing sample_rate"))?;
    let channels = stream
        .channels
        .filter(|value| *value > 0)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| SdkError::unsupported("ffprobe missing channels"))?;

    let duration_ms = stream
        .duration
        .as_deref()
        .and_then(parse_duration_seconds_to_ms)
        .or_else(|| {
            parsed
                .format
                .as_ref()
                .and_then(|format| format.duration.as_deref())
                .and_then(parse_duration_seconds_to_ms)
        });

    let bitrate_kbps = stream
        .bit_rate
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value / 1000)
        .or_else(|| {
            parsed
                .format
                .as_ref()
                .and_then(|format| format.bit_rate.as_deref())
                .and_then(|value| value.parse::<u64>().ok())
                .map(|value| value / 1000)
        })
        .and_then(|value| u32::try_from(value).ok());

    let codec_name = stream.codec_name;
    let container_name = parsed
        .format
        .as_ref()
        .and_then(|format| format.format_name.clone());

    let stream_tags = stream.tags.as_ref();
    let format_tags = parsed
        .format
        .as_ref()
        .and_then(|format| format.tags.as_ref());
    let title =
        extract_tag_text(stream_tags, "title").or_else(|| extract_tag_text(format_tags, "title"));
    let album =
        extract_tag_text(stream_tags, "album").or_else(|| extract_tag_text(format_tags, "album"));
    let artist =
        extract_tag_text(stream_tags, "artist").or_else(|| extract_tag_text(format_tags, "artist"));

    Ok(AudioProbe {
        sample_rate,
        channels,
        duration_ms,
        bitrate_kbps,
        codec_name,
        container_name,
        title,
        album,
        artist,
    })
}

fn extract_embedded_artworks_from_bytes(
    config: &FfmpegPluginConfig,
    input_bytes: &[u8],
) -> SdkResult<Vec<Artwork>> {
    let mut args = config.normalized_ffprobe_args();
    args.extend([
        "-v".to_string(),
        "error".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_streams".to_string(),
        "-i".to_string(),
        "pipe:0".to_string(),
    ]);
    let payload = run_command_with_stdin_capture_stdout_small(
        config.ffprobe_executable(),
        args,
        input_bytes,
        FFMPEG_PROCESS_TIMEOUT_MS,
    )?;
    let parsed: FfprobeOutput = serde_json::from_slice(payload.as_slice())
        .map_err(|error| SdkError::internal(format!("decode ffprobe json failed: {error}")))?;

    let mut artworks = Vec::<Artwork>::new();
    for stream in parsed.streams {
        if !stream.is_attached_picture() {
            continue;
        }
        let Some(index) = stream.index else {
            continue;
        };
        let width = stream.width.filter(|value| *value > 0);
        let height = stream.height.filter(|value| *value > 0);
        let Some(data) = extract_artwork_bytes_as_png_from_bytes(config, input_bytes, index)?
        else {
            continue;
        };
        artworks.push(Artwork {
            kind: ArtworkKind::FrontCover,
            mime: "image/png".to_string(),
            description: stream
                .tags
                .as_ref()
                .and_then(|tags| extract_tag_text(Some(tags), "comment")),
            width,
            height,
            data,
        });
    }
    Ok(artworks)
}

fn extract_artwork_bytes_as_png_from_bytes(
    config: &FfmpegPluginConfig,
    input_bytes: &[u8],
    stream_index: u32,
) -> SdkResult<Option<Vec<u8>>> {
    let mut args = config.normalized_ffmpeg_args();
    args.extend([
        "-nostdin".to_string(),
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-i".to_string(),
        "pipe:0".to_string(),
        "-map".to_string(),
        format!("0:{stream_index}"),
        "-frames:v".to_string(),
        "1".to_string(),
        "-f".to_string(),
        "image2pipe".to_string(),
        "-vcodec".to_string(),
        "png".to_string(),
        "pipe:1".to_string(),
    ]);
    let bytes = run_command_with_stdin_capture_stdout_small(
        config.ffmpeg_executable(),
        args,
        input_bytes,
        FFMPEG_PROCESS_TIMEOUT_MS,
    )?;
    if bytes.is_empty() || bytes.len() > MAX_EMBEDDED_ARTWORK_BYTES {
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn parse_duration_seconds_to_ms(raw: &str) -> Option<u64> {
    let seconds = raw.trim().parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    Some((seconds * 1000.0).round() as u64)
}

fn extract_tag_text(tags: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<String> {
    let tags = tags?;
    tags.get(key)
        .and_then(Value::as_str)
        .or_else(|| {
            tags.iter().find_map(|(k, v)| {
                if k.eq_ignore_ascii_case(key) {
                    v.as_str()
                } else {
                    None
                }
            })
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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
            .read(READ_OUTPUT_CHUNK_BYTES, Some(25))
            .map_err(map_sidecar_error)?;
        if chunk.is_empty() {
            break;
        }
        if output.len() >= PROBE_MAX_CAPTURE_BYTES {
            continue;
        }
        let remaining = PROBE_MAX_CAPTURE_BYTES - output.len();
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

fn run_command_with_stdin_capture_stdout_small(
    executable: String,
    args: Vec<String>,
    input: &[u8],
    timeout_ms: u32,
) -> SdkResult<Vec<u8>> {
    run_command_with_stdin_capture_stdout_with_timeout(executable, args, input, timeout_ms)
}

fn run_command_with_stdin_capture_stdout_with_timeout(
    executable: String,
    args: Vec<String>,
    input: &[u8],
    timeout_ms: u32,
) -> SdkResult<Vec<u8>> {
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
    let read_control = process.open_control().map_err(map_sidecar_error)?;

    let mut offset = 0usize;
    while offset < input.len() {
        let next = (offset + READ_OUTPUT_CHUNK_BYTES as usize).min(input.len());
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
        .wait_exit(Some(timeout_ms))
        .map_err(map_sidecar_error)?;
    if exit_code.is_none() {
        let _ = process.terminate(300);
        return Err(SdkError::timeout(format!(
            "ffmpeg sidecar process timed out for `{}`",
            executable_preview
        )));
    }

    let mut output = Vec::<u8>::new();
    loop {
        let chunk = read_control
            .read(READ_OUTPUT_CHUNK_BYTES, Some(25))
            .map_err(map_sidecar_error)?;
        if chunk.is_empty() {
            break;
        }
        output.extend_from_slice(chunk.as_slice());
    }

    if exit_code != Some(0) {
        let preview = preview_text_limited(output.as_slice(), PROBE_MAX_CAPTURE_BYTES);
        return Err(SdkError::internal(format!(
            "ffmpeg command failed for `{}`: {}; args={}; preview={}{}",
            executable_preview,
            describe_ffmpeg_exit_code(exit_code),
            args_preview,
            preview,
            stderr_capture_hint(preview.as_str())
        )));
    }

    read_control.close();
    Ok(output)
}

fn preview_text(bytes: &[u8]) -> String {
    preview_text_limited(bytes, PROBE_MAX_CAPTURE_BYTES)
}

fn preview_text_limited(bytes: &[u8], max_bytes: usize) -> String {
    let used = if bytes.len() <= max_bytes {
        bytes
    } else {
        &bytes[..max_bytes]
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

#[derive(Debug, Clone)]
struct AudioProbe {
    sample_rate: u32,
    channels: u16,
    duration_ms: Option<u64>,
    bitrate_kbps: Option<u32>,
    codec_name: Option<String>,
    container_name: Option<String>,
    title: Option<String>,
    album: Option<String>,
    artist: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    #[serde(default)]
    format: Option<FfprobeFormat>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    #[serde(default)]
    index: Option<u32>,
    #[serde(default)]
    codec_type: Option<String>,
    #[serde(default)]
    codec_name: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    sample_rate: Option<String>,
    #[serde(default)]
    channels: Option<u32>,
    #[serde(default)]
    duration: Option<String>,
    #[serde(default)]
    bit_rate: Option<String>,
    #[serde(default)]
    tags: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    disposition: Option<FfprobeDisposition>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    #[serde(default)]
    format_name: Option<String>,
    #[serde(default)]
    duration: Option<String>,
    #[serde(default)]
    bit_rate: Option<String>,
    #[serde(default)]
    tags: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, Deserialize)]
struct FfprobeDisposition {
    #[serde(default)]
    attached_pic: Option<u32>,
}

impl FfprobeStream {
    fn is_attached_picture(&self) -> bool {
        self.codec_type.as_deref() == Some("video")
            && self
                .disposition
                .as_ref()
                .and_then(|item| item.attached_pic)
                .is_some_and(|value| value > 0)
    }
}
