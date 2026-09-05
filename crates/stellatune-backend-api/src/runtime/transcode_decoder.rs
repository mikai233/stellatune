use std::path::Path;

use stellatune_audio_builtin_adapters::builtin_decoder::BuiltinDecoder;

#[derive(Debug, Clone, Default)]
pub struct MediaMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PcmF32Chunk {
    pub interleaved_f32le: Vec<u8>,
    pub frames: u32,
    pub eof: bool,
}

#[derive(Debug, Clone)]
pub struct TranscodeDecoderInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub metadata: Option<MediaMetadata>,
    pub decoder_plugin_id: Option<String>,
    pub decoder_type_id: Option<String>,
}

pub trait TranscodeDecoderSession: Send {
    fn info(&self) -> TranscodeDecoderInfo;
    fn read_pcm_f32(&mut self, max_frames: u32) -> Result<PcmF32Chunk, String>;
    fn close(&mut self) -> Result<(), String>;
}

pub async fn open_local_transcode_decoder(
    path: &str,
) -> Result<Box<dyn TranscodeDecoderSession>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("source path is empty".to_string());
    }
    let source = Path::new(path);
    if !source.is_file() {
        return Err(format!("source file does not exist: {}", source.display()));
    }
    let opened = super::local_decoder::open_local_decoder(source).await?;
    Ok(Box::new(NativeTranscodeDecoder {
        decoder: opened.decoder,
        plugin_id: opened.plugin_id,
        capability_id: opened.capability_id,
    }))
}

struct NativeTranscodeDecoder {
    decoder: BuiltinDecoder,
    plugin_id: Option<String>,
    capability_id: Option<String>,
}

impl TranscodeDecoderSession for NativeTranscodeDecoder {
    fn info(&self) -> TranscodeDecoderInfo {
        let spec = self.decoder.spec();
        TranscodeDecoderInfo {
            sample_rate: spec.sample_rate,
            channels: spec.channel_layout.channel_count(),
            duration_ms: self.decoder.effective_duration_ms_hint(),
            metadata: None,
            decoder_plugin_id: self.plugin_id.clone(),
            decoder_type_id: self.capability_id.clone(),
        }
    }

    fn read_pcm_f32(&mut self, max_frames: u32) -> Result<PcmF32Chunk, String> {
        let channels = usize::from(self.decoder.spec().channel_layout.channel_count());
        let Some(samples) = self.decoder.next_block(max_frames.max(1) as usize)? else {
            return Ok(PcmF32Chunk {
                interleaved_f32le: Vec::new(),
                frames: 0,
                eof: true,
            });
        };
        if !samples.len().is_multiple_of(channels) {
            return Err(format!(
                "native decoder produced misaligned block: samples={} channels={channels}",
                samples.len()
            ));
        }
        let frames = (samples.len() / channels).min(u32::MAX as usize) as u32;
        let mut bytes = Vec::with_capacity(samples.len() * 4);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        Ok(PcmF32Chunk {
            interleaved_f32le: bytes,
            frames,
            eof: false,
        })
    }

    fn close(&mut self) -> Result<(), String> {
        Ok(())
    }
}
