use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use stellatune_audio::gapless::gapless_trimmed_duration_ms;
use stellatune_audio_core::pipeline::context::{GaplessTrimSpec, StreamSpec};
use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::{
    AudioDecoder as SymphoniaDecoder, AudioDecoderOptions as DecoderOptions,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinDecoderScoreRule {
    pub ext: &'static str,
    pub score: u16,
}

pub const BUILTIN_DECODER_SCORE_RULES: &[BuiltinDecoderScoreRule] = &[
    BuiltinDecoderScoreRule {
        ext: "mp1",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mp2",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mp3",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mpa",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "aac",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "alac",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "m4a",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "m4b",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "m4r",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "m4p",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mp4",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mov",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "3gp",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "3g2",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "caf",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "flac",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "wav",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "wave",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "aif",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "aiff",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "aifc",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "ogg",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "oga",
        score: 90,
    },
];

pub fn normalize_extension(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn locator_path_for_extension(locator: &str) -> &str {
    let trimmed = locator.trim();
    let without_query_or_fragment = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    let Some((scheme, remainder)) = without_query_or_fragment.split_once("://") else {
        return without_query_or_fragment;
    };

    if !matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "file"
    ) {
        return without_query_or_fragment;
    }

    let slash_index = remainder.find('/').unwrap_or(remainder.len());
    &remainder[slash_index..]
}

pub fn extension_from_path(path: &str) -> String {
    Path::new(locator_path_for_extension(path))
        .extension()
        .and_then(|value| value.to_str())
        .map(normalize_extension)
        .unwrap_or_default()
}

pub fn builtin_decoder_score_for_ext(ext: &str) -> Option<u16> {
    let ext = normalize_extension(ext);
    if ext.is_empty() {
        return None;
    }
    BUILTIN_DECODER_SCORE_RULES
        .iter()
        .find(|rule| rule.ext == ext)
        .map(|rule| rule.score)
}

pub fn builtin_decoder_supported_extensions() -> Vec<String> {
    let mut out = BUILTIN_DECODER_SCORE_RULES
        .iter()
        .map(|rule| rule.ext.to_string())
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

pub struct BuiltinDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    spec: StreamSpec,
    duration_ms_hint: Option<u64>,
    encoder_delay_frames: u32,
    encoder_padding_frames: u32,
    pending: Vec<f32>,
}

impl BuiltinDecoder {
    pub fn open(path: &str) -> Result<Self, String> {
        let opened = open_media_input(path)?;
        let ext = opened.hint_extension;
        if !ext.is_empty() && builtin_decoder_score_for_ext(ext.as_str()).is_none() {
            return Err(format!(
                "builtin decoder does not support extension `{}`",
                if ext.is_empty() {
                    "<none>"
                } else {
                    ext.as_str()
                }
            ));
        }

        let mut hint = Hint::new();
        if !ext.is_empty() {
            hint.with_extension(ext.as_str());
        }

        let mss = MediaSourceStream::new(opened.source, MediaSourceStreamOptions::default());

        let mut format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| format!("symphonia probe failed: {e}"))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| "missing default audio track".to_string())?;
        let track_id = track.id;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .cloned()
            .ok_or_else(|| "missing audio codec parameters".to_string())?;
        let time_base = track.time_base;
        let num_frames = track.num_frames;
        let encoder_delay_frames = track.delay.unwrap_or(0);
        let encoder_padding_frames = track.padding.unwrap_or(0);

        let mut sample_rate = params.sample_rate.unwrap_or(0);
        let mut channels = params
            .channels
            .as_ref()
            .map(|v| v.count() as u16)
            .unwrap_or(0);

        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &DecoderOptions::default())
            .map_err(|e| format!("decoder init failed: {e}"))?;

        let mut duration_ms_hint = duration_ms_from_track_params(time_base, num_frames);
        if duration_ms_hint.is_none() {
            // TODO: Re-evaluate whether this seek-based duration fallback should be removed.
            duration_ms_hint = estimate_duration_ms_by_seek(format.as_mut(), track_id, time_base);
            // Restore start position after duration probing.
            let _ = format.seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::ZERO,
                    track_id: Some(track_id),
                },
            );
            decoder.reset();
        }

        let mut pending = Vec::new();
        if sample_rate == 0 || channels == 0 {
            while sample_rate == 0 || channels == 0 {
                match format.next_packet() {
                    Ok(Some(packet)) => {
                        if packet.track_id != track_id {
                            continue;
                        }
                        match decoder.decode(&packet) {
                            Ok(audio_buf) => {
                                if sample_rate == 0 {
                                    sample_rate = audio_buf.spec().rate();
                                }
                                if channels == 0 {
                                    channels = audio_buf.spec().channels().count() as u16;
                                }
                                append_decoded(&mut pending, audio_buf);
                            },
                            Err(SymphoniaError::DecodeError(_)) => continue,
                            Err(SymphoniaError::ResetRequired) => {
                                decoder.reset();
                                continue;
                            },
                            Err(e) => {
                                return Err(format!(
                                    "decode failed while probing stream spec: {e}"
                                ));
                            },
                        }
                    },
                    Ok(None) => break,
                    Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                        break;
                    },
                    Err(e) => {
                        return Err(format!("read packet failed while probing stream spec: {e}"));
                    },
                }
            }
        }
        if sample_rate == 0 || channels == 0 {
            return Err(format!(
                "missing stream spec after probe: sample_rate={sample_rate} channels={channels}"
            ));
        }

        Ok(Self {
            format,
            decoder,
            track_id,
            spec: StreamSpec {
                sample_rate,
                channels,
            },
            duration_ms_hint,
            encoder_delay_frames,
            encoder_padding_frames,
            pending,
        })
    }

    pub fn spec(&self) -> StreamSpec {
        self.spec
    }

    pub fn duration_ms_hint(&self) -> Option<u64> {
        self.duration_ms_hint
    }

    pub fn effective_duration_ms_hint(&self) -> Option<u64> {
        gapless_trimmed_duration_ms(
            self.duration_ms_hint,
            self.spec.sample_rate,
            self.gapless_trim_spec(),
        )
    }

    pub fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        let spec = GaplessTrimSpec {
            head_frames: self.encoder_delay_frames,
            tail_frames: self.encoder_padding_frames,
        };
        (!spec.is_disabled()).then_some(spec)
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::from_millis_u64(position_ms),
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| format!("seek failed: {e}"))?;
        self.decoder.reset();
        self.pending.clear();
        Ok(())
    }

    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        let channels = self.spec.channels.max(1) as usize;
        let want_samples = frames.saturating_mul(channels).max(channels);

        while self.pending.len() < want_samples {
            match self.format.next_packet() {
                Ok(Some(packet)) => {
                    if packet.track_id != self.track_id {
                        continue;
                    }
                    match self.decoder.decode(&packet) {
                        Ok(audio_buf) => {
                            append_decoded(&mut self.pending, audio_buf);
                        },
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        },
                        Err(e) => return Err(format!("decode failed: {e}")),
                    }
                },
                Ok(None) => break,
                Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    break;
                },
                Err(e) => return Err(format!("read packet failed: {e}")),
            }
        }

        if self.pending.is_empty() {
            return Ok(None);
        }
        let take = want_samples.min(self.pending.len());
        let out = self.pending.drain(..take).collect::<Vec<_>>();
        Ok(Some(out))
    }
}

fn append_decoded(pending: &mut Vec<f32>, audio_buf: GenericAudioBufferRef<'_>) {
    let mut samples = vec![0.0f32; audio_buf.samples_interleaved()];
    audio_buf.copy_to_slice_interleaved(&mut samples);
    pending.extend_from_slice(&samples);
}

fn duration_ms_from_track_params(
    time_base: Option<TimeBase>,
    n_frames: Option<u64>,
) -> Option<u64> {
    let tb = time_base?;
    let frames = n_frames?;
    Some(duration_ms_from_time_base(
        tb,
        Timestamp::new(frames.min(i64::MAX as u64) as i64),
    ))
}

fn duration_ms_from_time_base(tb: TimeBase, ts: Timestamp) -> u64 {
    tb.calc_time(ts)
        .map(|time| (time.as_secs_f64() * 1000.0).round() as u64)
        .unwrap_or(u64::MAX)
}

fn open_media_input(locator: &str) -> Result<OpenedMediaInput, String> {
    if is_http_locator(locator) {
        return open_http_media_input(locator);
    }

    let file = File::open(locator).map_err(|e| format!("failed to open `{locator}`: {e}"))?;
    Ok(OpenedMediaInput {
        source: Box::new(file),
        hint_extension: extension_from_path(locator),
    })
}

fn is_http_locator(locator: &str) -> bool {
    let trimmed = locator.trim();
    trimmed.len() >= 7
        && (trimmed[..7].eq_ignore_ascii_case("http://")
            || (trimmed.len() >= 8 && trimmed[..8].eq_ignore_ascii_case("https://")))
}

fn open_http_media_input(locator: &str) -> Result<OpenedMediaInput, String> {
    let (source, hint_extension) = HttpMediaSource::open(locator)?;

    Ok(OpenedMediaInput {
        source: Box::new(source),
        hint_extension,
    })
}

fn extension_from_content_type(content_type: &str) -> Option<String> {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase();
    let ext = match mime.as_str() {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/aac" | "audio/aacp" => "aac",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/wav" | "audio/wave" | "audio/x-wav" => "wav",
        "audio/aiff" | "audio/x-aiff" => "aiff",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/ogg" => "ogg",
        "audio/ogg; codecs=vorbis" => "ogg",
        "audio/vnd.wave" => "wav",
        _ => return None,
    };
    Some(ext.to_string())
}

fn estimate_duration_ms_by_seek(
    format: &mut dyn FormatReader,
    track_id: u32,
    time_base: Option<TimeBase>,
) -> Option<u64> {
    let tb = time_base?;
    let seeked = format
        .seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time: Time::MAX,
                track_id: Some(track_id),
            },
        )
        .ok()?;
    let end_ts = seeked.actual_ts.max(seeked.required_ts);
    Some(duration_ms_from_time_base(tb, end_ts))
}

struct OpenedMediaInput {
    source: Box<dyn MediaSource>,
    hint_extension: String,
}

struct HttpMediaSource {
    client: reqwest::blocking::Client,
    url: String,
    position: u64,
    total_size: Option<u64>,
    seekable: bool,
    response: Mutex<Option<reqwest::blocking::Response>>,
}

impl HttpMediaSource {
    fn open(locator: &str) -> Result<(Self, String), String> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("failed to build HTTP client for `{locator}`: {e}"))?;

        let response = open_http_response(&client, locator, 0)?;
        let total_size = resolve_total_size(&response, 0);
        let hint_extension = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(extension_from_content_type)
            .unwrap_or_else(|| extension_from_path(locator));
        let seekable = probe_http_range_support(&client, locator, total_size);

        Ok((
            Self {
                client,
                url: locator.trim().to_string(),
                position: 0,
                total_size,
                seekable,
                response: Mutex::new(Some(response)),
            },
            hint_extension,
        ))
    }

    fn reopen_at(&self, offset: u64) -> Result<reqwest::blocking::Response, io::Error> {
        open_http_response(&self.client, self.url.as_str(), offset).map_err(io::Error::other)
    }
}

impl MediaSource for HttpMediaSource {
    fn is_seekable(&self) -> bool {
        self.seekable
    }

    fn byte_len(&self) -> Option<u64> {
        self.total_size
    }
}

impl Read for HttpMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let position = self.position;
        let needs_open = {
            let response_slot = self
                .response
                .get_mut()
                .map_err(|_| io::Error::other("http response mutex poisoned"))?;
            response_slot.is_none()
        };
        if needs_open {
            let reopened = self.reopen_at(position)?;
            let response_slot = self
                .response
                .get_mut()
                .map_err(|_| io::Error::other("http response mutex poisoned"))?;
            *response_slot = Some(reopened);
        }

        let read = {
            let response_slot = self
                .response
                .get_mut()
                .map_err(|_| io::Error::other("http response mutex poisoned"))?;
            let Some(response) = response_slot.as_mut() else {
                return Ok(0);
            };
            response.read(buf)?
        };
        self.position = self.position.saturating_add(read as u64);
        if read == 0 {
            let response_slot = self
                .response
                .get_mut()
                .map_err(|_| io::Error::other("http response mutex poisoned"))?;
            *response_slot = None;
        }
        Ok(read)
    }
}

impl Seek for HttpMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(delta) => add_signed_offset(self.position, delta)?,
            SeekFrom::End(delta) => {
                let len = self.total_size.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Unsupported,
                        "http source length is unknown; SeekFrom::End is unavailable",
                    )
                })?;
                add_signed_offset(len, delta)?
            },
        };

        let target = if let Some(total_size) = self.total_size {
            target.min(total_size)
        } else {
            target
        };

        if target == self.position {
            return Ok(self.position);
        }

        if target != 0 && !self.seekable {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "http source does not support range seek",
            ));
        }

        let next_response = self.reopen_at(target)?;
        *self
            .response
            .get_mut()
            .map_err(|_| io::Error::other("http response mutex poisoned"))? = Some(next_response);
        self.position = target;
        Ok(self.position)
    }
}

fn open_http_response(
    client: &reqwest::blocking::Client,
    locator: &str,
    start_offset: u64,
) -> Result<reqwest::blocking::Response, String> {
    let mut builder = client.get(locator.trim());
    if start_offset > 0 {
        builder = builder.header(reqwest::header::RANGE, format!("bytes={start_offset}-"));
    }

    let response = builder
        .send()
        .map_err(|e| format!("failed to request `{locator}`: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP request failed for `{locator}`: {e}"))?;

    if start_offset > 0 && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(format!(
            "http server does not support range seek for `{locator}`"
        ));
    }

    Ok(response)
}

fn resolve_total_size(response: &reqwest::blocking::Response, start_offset: u64) -> Option<u64> {
    if let Some(content_range) = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        && let Some(total) = parse_content_range_total(content_range)
    {
        return Some(total);
    }

    if start_offset == 0 {
        return response.content_length();
    }

    response
        .content_length()
        .map(|remaining| start_offset.saturating_add(remaining))
}

fn parse_content_range_total(content_range: &str) -> Option<u64> {
    let (_, total) = content_range.split_once('/')?;
    let total = total.trim();
    if total == "*" {
        return None;
    }
    total.parse::<u64>().ok()
}

fn probe_http_range_support(
    client: &reqwest::blocking::Client,
    locator: &str,
    total_size: Option<u64>,
) -> bool {
    if total_size.is_none() {
        return false;
    }

    let response = client
        .get(locator.trim())
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send();
    let Ok(response) = response else {
        return false;
    };
    response.status() == reqwest::StatusCode::PARTIAL_CONTENT
}

fn add_signed_offset(base: u64, delta: i64) -> io::Result<u64> {
    if delta >= 0 {
        return Ok(base.saturating_add(delta as u64));
    }

    let magnitude = delta.unsigned_abs();
    base.checked_sub(magnitude).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "seek before start of http source",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{BuiltinDecoder, extension_from_path};
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use std::thread;

    #[test]
    fn extension_from_path_understands_urls() {
        assert_eq!(
            extension_from_path("https://example.com/music/Track.FLAC?token=abc#frag"),
            "flac"
        );
        assert_eq!(extension_from_path("http://example.com/stream"), "");
        assert_eq!(extension_from_path("C:/music/song.mp3"), "mp3");
    }

    #[test]
    fn builtin_decoder_opens_http_wav() {
        let wav_bytes = tiny_wav_bytes();
        let (addr, join) = serve_static_http(wav_bytes, 2);
        let url = format!("http://{addr}/test.wav?download=1");

        let decoder = BuiltinDecoder::open(url.as_str()).expect("http decoder open should work");
        assert_eq!(decoder.spec().sample_rate, 44_100);
        assert_eq!(decoder.spec().channels, 1);

        join.join().expect("server thread should exit cleanly");
    }

    #[test]
    fn builtin_decoder_seeks_http_wav() {
        let wav_bytes = tiny_wav_bytes();
        let (addr, join) = serve_static_http(wav_bytes, 3);
        let url = format!("http://{addr}/seek-test.wav");

        let mut decoder =
            BuiltinDecoder::open(url.as_str()).expect("http decoder open should work");
        decoder.seek_ms(500).expect("http wav seek should work");
        let block = decoder
            .next_block(256)
            .expect("next block after seek should succeed");
        assert!(block.is_some(), "seeked decoder should still produce audio");

        join.join().expect("server thread should exit cleanly");
    }

    fn serve_static_http(
        body: Vec<u8>,
        expected_requests: usize,
    ) -> (SocketAddr, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("read local addr");
        let join = thread::spawn(move || {
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().expect("accept request");
                let mut request_buf = [0_u8; 4096];
                let request_len = stream.read(&mut request_buf).expect("read request");
                let request = String::from_utf8_lossy(&request_buf[..request_len]);
                let range_header = request
                    .lines()
                    .find(|line| line.to_ascii_lowercase().starts_with("range:"))
                    .map(str::to_string);

                let (status_line, response_body, content_range_header) =
                    if let Some(range_header) = range_header {
                        let range_value = range_header
                            .split_once(':')
                            .map(|(_, value)| value.trim())
                            .expect("range header should contain colon");
                        let (start, end) = parse_range(range_value, body.len());
                        (
                            "HTTP/1.1 206 Partial Content\r\n".to_string(),
                            body[start..=end].to_vec(),
                            Some(format!(
                                "Content-Range: bytes {start}-{end}/{}\r\n",
                                body.len()
                            )),
                        )
                    } else {
                        ("HTTP/1.1 200 OK\r\n".to_string(), body.clone(), None)
                    };

                let mut response = String::new();
                response.push_str(status_line.as_str());
                response.push_str("Content-Type: audio/wav\r\n");
                response.push_str("Accept-Ranges: bytes\r\n");
                response.push_str(format!("Content-Length: {}\r\n", response_body.len()).as_str());
                if let Some(content_range_header) = content_range_header {
                    response.push_str(content_range_header.as_str());
                }
                response.push_str("Connection: close\r\n\r\n");
                stream
                    .write_all(response.as_bytes())
                    .expect("write response head");
                stream
                    .write_all(response_body.as_slice())
                    .expect("write response body");
            }
        });
        (addr, join)
    }

    fn parse_range(range_value: &str, total_len: usize) -> (usize, usize) {
        let value = range_value
            .strip_prefix("bytes=")
            .expect("range must start with bytes=");
        let (start_raw, end_raw) = value.split_once('-').expect("range must contain dash");
        let start = start_raw
            .parse::<usize>()
            .expect("range start must be a number");
        let end = if end_raw.trim().is_empty() {
            total_len.saturating_sub(1)
        } else {
            end_raw
                .parse::<usize>()
                .expect("range end must be a number")
        };
        (
            start.min(total_len.saturating_sub(1)),
            end.min(total_len.saturating_sub(1)),
        )
    }

    fn tiny_wav_bytes() -> Vec<u8> {
        let samples = (0..44_100)
            .map(|index| match index % 4 {
                0 => 0_i16,
                1 => 1200_i16,
                2 => -1200_i16,
                _ => 0_i16,
            })
            .collect::<Vec<_>>();
        let sample_rate = 44_100u32;
        let channels = 1u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * (bits_per_sample / 8);
        let data_size = (samples.len() * std::mem::size_of::<i16>()) as u32;
        let riff_size = 36 + data_size;

        let mut out = Vec::with_capacity((44 + data_size) as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&riff_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits_per_sample.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }
}
