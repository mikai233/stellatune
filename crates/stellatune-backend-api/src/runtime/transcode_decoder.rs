use std::path::Path;

use stellatune_audio_builtin_adapters::builtin_decoder::{BuiltinDecoder, extension_from_path};
use stellatune_audio_builtin_adapters::ncm_decoder::NcmDecoder;

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

pub fn open_local_transcode_decoder(
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
    if extension_from_path(path).eq_ignore_ascii_case("ncm") {
        return Ok(Box::new(NativeTranscodeDecoder::Ncm(NcmDecoder::open(
            path,
        )?)));
    }
    Ok(Box::new(NativeTranscodeDecoder::Builtin(
        BuiltinDecoder::open(path)?,
    )))
}

enum NativeTranscodeDecoder {
    Builtin(BuiltinDecoder),
    Ncm(NcmDecoder),
}

impl NativeTranscodeDecoder {
    fn spec(&self) -> stellatune_audio_core::AudioFormat {
        match self {
            Self::Builtin(decoder) => decoder.spec(),
            Self::Ncm(decoder) => decoder.spec(),
        }
    }

    fn duration_ms(&self) -> Option<u64> {
        match self {
            Self::Builtin(decoder) => decoder.effective_duration_ms_hint(),
            Self::Ncm(decoder) => decoder.duration_ms_hint(),
        }
    }

    fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        match self {
            Self::Builtin(decoder) => decoder.next_block(frames),
            Self::Ncm(decoder) => decoder.next_block(frames),
        }
    }
}

impl TranscodeDecoderSession for NativeTranscodeDecoder {
    fn info(&self) -> TranscodeDecoderInfo {
        let spec = self.spec();
        TranscodeDecoderInfo {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            duration_ms: self.duration_ms(),
            metadata: None,
            decoder_plugin_id: None,
            decoder_type_id: None,
        }
    }

    fn read_pcm_f32(&mut self, max_frames: u32) -> Result<PcmF32Chunk, String> {
        let channels = self.spec().channels.max(1) as usize;
        let Some(samples) = self.next_block(max_frames.max(1) as usize)? else {
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
