use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};

use super::transcode_decoder::{MediaMetadata, PcmF32Chunk};

const NATIVE_PLUGIN_ID: &str = "builtin.native";
const WAV_ENCODER_ID: &str = "wav-f32";

#[derive(Debug, Clone)]
pub struct TranscodeEncoderDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

pub trait TranscodeEncoderSession: Send {
    fn write_pcm_f32(&mut self, chunk: PcmF32Chunk) -> Result<u32, String>;
    fn finish(&mut self) -> Result<(), String>;
    fn close(&mut self) -> Result<(), String>;
    fn written_bytes(&self) -> u64;
}

pub fn list_local_transcode_encoders() -> Vec<TranscodeEncoderDescriptor> {
    vec![TranscodeEncoderDescriptor {
        plugin_id: NATIVE_PLUGIN_ID.to_string(),
        plugin_name: "Stellatune Native".to_string(),
        type_id: WAV_ENCODER_ID.to_string(),
        display_name: "WAV (32-bit float)".to_string(),
        config_schema_json: r#"{"type":"object","additionalProperties":false}"#.to_string(),
        default_config_json: "{}".to_string(),
    }]
}

#[allow(clippy::too_many_arguments)]
pub fn open_local_transcode_encoder(
    output_path: &str,
    encoder_plugin_id: &str,
    encoder_type_id: &str,
    sample_rate: u32,
    channels: u16,
    _metadata: Option<MediaMetadata>,
    _encoder_config_json: &str,
    _encoder_options_json: Option<&str>,
) -> Result<Box<dyn TranscodeEncoderSession>, String> {
    if encoder_plugin_id.trim() != NATIVE_PLUGIN_ID || encoder_type_id.trim() != WAV_ENCODER_ID {
        return Err(format!(
            "unsupported native encoder: {}::{}",
            encoder_plugin_id.trim(),
            encoder_type_id.trim()
        ));
    }
    if sample_rate == 0 || channels == 0 {
        return Err(format!(
            "invalid encoder stream spec: sample_rate={sample_rate} channels={channels}"
        ));
    }
    let file = File::create(output_path.trim())
        .map_err(|error| format!("create output `{}`: {error}", output_path.trim()))?;
    let mut encoder = WavF32Encoder {
        writer: BufWriter::new(file),
        sample_rate,
        channels,
        data_bytes: 0,
        closed: false,
    };
    encoder.write_header()?;
    Ok(Box::new(encoder))
}

struct WavF32Encoder {
    writer: BufWriter<File>,
    sample_rate: u32,
    channels: u16,
    data_bytes: u64,
    closed: bool,
}

impl WavF32Encoder {
    fn write_header(&mut self) -> Result<(), String> {
        let block_align = self.channels.saturating_mul(4);
        let byte_rate = self.sample_rate.saturating_mul(block_align as u32);
        let mut header = Vec::with_capacity(44);
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&36_u32.to_le_bytes());
        header.extend_from_slice(b"WAVEfmt ");
        header.extend_from_slice(&16_u32.to_le_bytes());
        header.extend_from_slice(&3_u16.to_le_bytes());
        header.extend_from_slice(&self.channels.to_le_bytes());
        header.extend_from_slice(&self.sample_rate.to_le_bytes());
        header.extend_from_slice(&byte_rate.to_le_bytes());
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&32_u16.to_le_bytes());
        header.extend_from_slice(b"data");
        header.extend_from_slice(&0_u32.to_le_bytes());
        self.writer
            .write_all(&header)
            .map_err(|error| format!("write WAV header: {error}"))
    }

    fn finalize_header(&mut self) -> Result<(), String> {
        self.writer
            .flush()
            .map_err(|error| format!("flush WAV: {error}"))?;
        let data_size = self.data_bytes.min(u32::MAX as u64) as u32;
        let riff_size = 36_u32.saturating_add(data_size);
        self.writer
            .seek(SeekFrom::Start(4))
            .and_then(|_| self.writer.write_all(&riff_size.to_le_bytes()))
            .and_then(|_| self.writer.seek(SeekFrom::Start(40)).map(|_| ()))
            .and_then(|_| self.writer.write_all(&data_size.to_le_bytes()))
            .and_then(|_| self.writer.flush())
            .map_err(|error| format!("finalize WAV header: {error}"))
    }
}

impl TranscodeEncoderSession for WavF32Encoder {
    fn write_pcm_f32(&mut self, chunk: PcmF32Chunk) -> Result<u32, String> {
        if self.closed {
            return Err("native encoder is closed".to_string());
        }
        let frame_bytes = self.channels.max(1) as usize * 4;
        if !chunk.interleaved_f32le.len().is_multiple_of(frame_bytes) {
            return Err("PCM chunk is not frame-aligned".to_string());
        }
        self.writer
            .write_all(&chunk.interleaved_f32le)
            .map_err(|error| format!("write WAV samples: {error}"))?;
        self.data_bytes = self
            .data_bytes
            .saturating_add(chunk.interleaved_f32le.len() as u64);
        Ok((chunk.interleaved_f32le.len() / frame_bytes).min(u32::MAX as usize) as u32)
    }

    fn finish(&mut self) -> Result<(), String> {
        self.finalize_header()
    }

    fn close(&mut self) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        self.finalize_header()?;
        self.closed = true;
        Ok(())
    }

    fn written_bytes(&self) -> u64 {
        self.data_bytes.saturating_add(44)
    }
}

impl Drop for WavF32Encoder {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
