use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::audio::{AudioBufferRef, SignalSpec};
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::formats::{SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unsupported channel count: {channels} (only mono/stereo supported)")]
    UnsupportedChannels { channels: u16 },

    #[error("missing audio track")]
    MissingTrack,

    #[error("missing sample rate in codec parameters")]
    MissingSampleRate,

    #[error("decoder error: {0}")]
    Symphonia(#[from] SymphoniaError),
}

pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    spec: TrackSpec,
    sample_buf: Option<SampleBuffer<f32>>,
    pending: Vec<f32>,
}

impl Decoder {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DecodeError> {
        let path = path.as_ref();
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            hint.with_extension(ext);
        }

        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;

        let format = probed.format;
        let track = format.default_track().ok_or(DecodeError::MissingTrack)?;
        let track_id = track.id;
        let params = track.codec_params.clone();

        let sample_rate = params.sample_rate.ok_or(DecodeError::MissingSampleRate)?;
        let channels = params
            .channels
            .as_ref()
            .map(|c| c.count() as u16)
            .unwrap_or(0);

        let decoder = symphonia::default::get_codecs().make(&params, &DecoderOptions::default())?;

        Ok(Self {
            format,
            decoder,
            track_id,
            spec: TrackSpec {
                sample_rate,
                channels,
            },
            sample_buf: None,
            pending: Vec::new(),
        })
    }

    pub fn spec(&self) -> TrackSpec {
        self.spec
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), DecodeError> {
        let secs = position_ms / 1000;
        let frac = (position_ms % 1000) as f64 / 1000.0;
        let time = Time::new(secs, frac);
        // Some formats/codecs require a reset after seek.
        let _ = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;
        self.decoder.reset();
        self.pending.clear();
        Ok(())
    }

    /// Decode up to `frames` frames (interleaved stereo f32).
    ///
    /// Returns `Ok(None)` on end-of-stream.
    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, DecodeError> {
        let want_samples = frames.saturating_mul(self.spec.channels as usize);

        while self.pending.len() < want_samples {
            match self.format.next_packet() {
                Ok(packet) => {
                    if packet.track_id() != self.track_id {
                        continue;
                    }

                    match self.decoder.decode(&packet) {
                        Ok(audio_buf) => {
                            append_decoded(&mut self.sample_buf, &mut self.pending, audio_buf)
                        }
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => return Err(e.into()),
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

fn append_decoded(
    sample_buf: &mut Option<SampleBuffer<f32>>,
    pending: &mut Vec<f32>,
    audio_buf: AudioBufferRef<'_>,
) {
    let spec = SignalSpec::new(audio_buf.spec().rate, audio_buf.spec().channels.clone());
    let duration = audio_buf.capacity() as u64;

    let needs_realloc = match sample_buf.as_ref() {
        None => true,
        Some(buf) => buf.capacity() < audio_buf.capacity(),
    };
    if needs_realloc {
        *sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
    }

    let sb = sample_buf.as_mut().expect("just initialized");
    sb.copy_interleaved_ref(audio_buf);
    pending.extend_from_slice(sb.samples());
}
