use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow};

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
    let mut d = stellatune_decode::Decoder::open(&input)
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
