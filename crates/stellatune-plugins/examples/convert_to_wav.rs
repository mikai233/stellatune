use std::fs::File;
use std::io;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::audio::{AudioBufferRef, SignalSpec};
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};

struct Wav16Writer {
    file: File,
    channels: u16,
    data_bytes: u32,
}

impl Wav16Writer {
    fn create(path: impl AsRef<Path>, sample_rate: u32, channels: u16) -> Result<Self> {
        if channels == 0 {
            return Err(anyhow!("channels must be > 0"));
        }
        if sample_rate == 0 {
            return Err(anyhow!("sample_rate must be > 0"));
        }
        let mut file = File::create(path.as_ref())
            .with_context(|| format!("failed to create {}", path.as_ref().display()))?;

        // RIFF header with placeholder sizes; patch at finish.
        // PCM 16-bit little-endian.
        let bits_per_sample: u16 = 16;
        let block_align: u16 = channels.saturating_mul(bits_per_sample / 8);
        let byte_rate: u32 = sample_rate.saturating_mul(block_align as u32);

        file.write_all(b"RIFF")?;
        file.write_all(&0u32.to_le_bytes())?; // chunk size placeholder
        file.write_all(b"WAVE")?;

        file.write_all(b"fmt ")?;
        file.write_all(&16u32.to_le_bytes())?; // fmt chunk size
        file.write_all(&1u16.to_le_bytes())?; // audio format = PCM
        file.write_all(&channels.to_le_bytes())?;
        file.write_all(&sample_rate.to_le_bytes())?;
        file.write_all(&byte_rate.to_le_bytes())?;
        file.write_all(&block_align.to_le_bytes())?;
        file.write_all(&bits_per_sample.to_le_bytes())?;

        file.write_all(b"data")?;
        file.write_all(&0u32.to_le_bytes())?; // data size placeholder

        Ok(Self {
            file,
            channels,
            data_bytes: 0,
        })
    }

    fn write_f32_interleaved(&mut self, samples: &[f32]) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let channels = self.channels.max(1) as usize;
        if !samples.len().is_multiple_of(channels) {
            return Err(anyhow!(
                "interleaved sample count {} not divisible by channels {}",
                samples.len(),
                channels
            ));
        }

        // Convert to i16 PCM.
        // Note: This is a simple conversion; future work can add dither.
        let mut buf = Vec::<u8>::with_capacity(samples.len() * 2);
        for &s in samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            buf.extend_from_slice(&v.to_le_bytes());
        }

        self.file.write_all(&buf)?;
        self.data_bytes = self.data_bytes.saturating_add(buf.len() as u32);
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        // Patch RIFF chunk size at offset 4: 36 + data_bytes.
        let riff_size = 36u32.saturating_add(self.data_bytes);
        self.file.seek(SeekFrom::Start(4))?;
        self.file.write_all(&riff_size.to_le_bytes())?;

        // Patch data chunk size at offset 40.
        self.file.seek(SeekFrom::Start(40))?;
        self.file.write_all(&self.data_bytes.to_le_bytes())?;

        self.file.flush()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrackSpec {
    sample_rate: u32,
    channels: u16,
}

struct BuiltinDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    spec: TrackSpec,
    sample_buf: Option<SampleBuffer<f32>>,
    pending: Vec<f32>,
}

fn supports_extension(ext: &str) -> bool {
    matches!(ext, "mp3" | "flac" | "wav")
}

impl BuiltinDecoder {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !supports_extension(&ext) {
            bail!("unsupported file extension for built-in decoder: `{ext}`");
        }

        let mut hint = Hint::new();
        if !ext.is_empty() {
            hint.with_extension(&ext);
        }

        let file = File::open(path)
            .with_context(|| format!("failed to open input file {}", path.display()))?;
        let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
        let probed = get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;

        let format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| anyhow!("missing audio track"))?;
        let track_id = track.id;
        let params = track.codec_params.clone();
        let sample_rate = params
            .sample_rate
            .ok_or_else(|| anyhow!("missing sample rate in codec parameters"))?;
        let channels = params
            .channels
            .as_ref()
            .map(|c| c.count() as u16)
            .unwrap_or(0);
        let decoder = get_codecs().make(&params, &DecoderOptions::default())?;

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

    fn spec(&self) -> TrackSpec {
        self.spec
    }

    fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>> {
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
                        },
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        },
                        Err(e) => return Err(e.into()),
                    }
                },
                Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    break;
                },
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
    let spec = SignalSpec::new(audio_buf.spec().rate, audio_buf.spec().channels);
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

fn main() -> Result<()> {
    // Usage:
    //   convert_to_wav <plugin_dir> <input> <output.wav>
    let mut args = std::env::args().skip(1);
    let _plugin_dir = args.next().ok_or_else(|| anyhow!("missing <plugin_dir>"))?;
    let input = args.next().ok_or_else(|| anyhow!("missing <input>"))?;
    let output = args.next().ok_or_else(|| anyhow!("missing <output.wav>"))?;
    if args.next().is_some() {
        return Err(anyhow!("unexpected extra arguments"));
    }

    // Built-in decode example. Plugin decode flow has moved to host runtime.
    let mut d = BuiltinDecoder::open(&input)
        .map_err(|e| anyhow!("failed to open built-in decoder: {e}"))?;
    let spec = d.spec();
    eprintln!(
        "decoder: builtin sr={} ch={}",
        spec.sample_rate, spec.channels
    );
    let mut out = Wav16Writer::create(&output, spec.sample_rate, spec.channels)?;
    while let Some(samples) = d
        .next_block(4096)
        .map_err(|e| anyhow!("decode failed: {e}"))?
    {
        out.write_f32_interleaved(&samples)?;
    }
    out.finish()?;
    Ok(())
}
