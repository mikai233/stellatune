use stellatune_plugin_sdk::prelude::*;

pub struct ExampleDecoder;
pub struct ExampleDecoderSession;

impl PluginLifecycle for ExampleDecoder {}
impl ConfigStateOps for ExampleDecoderSession {}

impl DecoderSession for ExampleDecoderSession {
    fn info(&self) -> SdkResult<DecoderInfo> {
        Ok(DecoderInfo {
            sample_rate: 44_100,
            channels: 2,
            duration_ms: None,
            seekable: true,
            encoder_delay_frames: 0,
            encoder_padding_frames: 0,
        })
    }

    fn metadata(&self) -> SdkResult<MediaMetadata> {
        Ok(MediaMetadata {
            tags: AudioTags::default(),
            duration_ms: None,
            format: EncodedAudioFormat {
                codec: "unknown".to_string(),
                sample_rate: Some(44_100),
                channels: Some(2),
                bitrate_kbps: None,
                container: None,
            },
            extras: Vec::new(),
        })
    }

    fn read_pcm_f32(&mut self, _max_frames: u32) -> SdkResult<PcmF32Chunk> {
        Ok(PcmF32Chunk {
            interleaved_f32le: Vec::new(),
            frames: 0,
            eof: true,
        })
    }

    fn seek_ms(&mut self, _position_ms: u64) -> SdkResult<()> {
        Ok(())
    }
}

impl DecoderPlugin for ExampleDecoder {
    type Session = ExampleDecoderSession;

    const TYPE_ID: &'static str = "example-decoder";
    const DISPLAY_NAME: &'static str = "Example Decoder";

    fn open(&mut self, _input: DecoderInput) -> SdkResult<Self::Session> {
        Ok(ExampleDecoderSession)
    }
}

fn create_decoder() -> SdkResult<ExampleDecoder> {
    Ok(ExampleDecoder)
}

stellatune_plugin_sdk::export_decoder_plugin! {
    export: decoder_export,
    plugin_type: crate::ExampleDecoder,
    create: crate::create_decoder,
    plugin_id: "dev.stellatune.example.decoder",
    component_id: "decoder-main",
    type_id: "example-decoder",
    display_name: "Example Decoder",
}

pub struct ExampleSource;
pub struct ExampleCatalog;
pub struct ExampleSourceStream;

impl PluginLifecycle for ExampleSource {}
impl ConfigStateOps for ExampleCatalog {}

impl SourceStream for ExampleSourceStream {
    fn metadata(&self) -> SdkResult<MediaMetadata> {
        Ok(MediaMetadata {
            tags: AudioTags::default(),
            duration_ms: None,
            format: EncodedAudioFormat {
                codec: "flac".to_string(),
                sample_rate: None,
                channels: None,
                bitrate_kbps: None,
                container: None,
            },
            extras: Vec::new(),
        })
    }

    fn read(&mut self, _max_bytes: u32) -> SdkResult<EncodedChunk> {
        Ok(EncodedChunk {
            bytes: Vec::new(),
            eof: true,
        })
    }
}

impl SourceCatalog for ExampleCatalog {
    type Stream = ExampleSourceStream;

    fn list_items_json(&mut self, _request_json: &str) -> SdkResult<String> {
        Ok("[]".to_string())
    }

    fn open_stream_json(&mut self, _track_json: &str) -> SdkResult<Self::Stream> {
        Ok(ExampleSourceStream)
    }
}

impl SourcePlugin for ExampleSource {
    type Catalog = ExampleCatalog;

    const TYPE_ID: &'static str = "example-source";
    const DISPLAY_NAME: &'static str = "Example Source";

    fn create_catalog(&mut self) -> SdkResult<Self::Catalog> {
        Ok(ExampleCatalog)
    }
}

fn create_source() -> SdkResult<ExampleSource> {
    Ok(ExampleSource)
}

stellatune_plugin_sdk::export_source_plugin! {
    export: source_export,
    plugin_type: crate::ExampleSource,
    create: crate::create_source,
    plugin_id: "dev.stellatune.example.source",
    component_id: "source-main",
    type_id: "example-source",
    display_name: "Example Source",
}

pub struct ExampleLyrics;
pub struct ExampleProvider;

impl PluginLifecycle for ExampleLyrics {}
impl ConfigStateOps for ExampleProvider {}

impl LyricsProvider for ExampleProvider {
    fn search(&mut self, _keyword: &str) -> SdkResult<Vec<LyricCandidate>> {
        Ok(Vec::new())
    }

    fn fetch(&mut self, _id: &str) -> SdkResult<String> {
        Ok(String::new())
    }
}

impl LyricsPlugin for ExampleLyrics {
    type Provider = ExampleProvider;

    const TYPE_ID: &'static str = "example-lyrics";
    const DISPLAY_NAME: &'static str = "Example Lyrics";

    fn create_provider(&mut self) -> SdkResult<Self::Provider> {
        Ok(ExampleProvider)
    }
}

fn create_lyrics() -> SdkResult<ExampleLyrics> {
    Ok(ExampleLyrics)
}

stellatune_plugin_sdk::export_lyrics_plugin! {
    export: lyrics_export,
    plugin_type: crate::ExampleLyrics,
    create: crate::create_lyrics,
    plugin_id: "dev.stellatune.example.lyrics",
    component_id: "lyrics-main",
    type_id: "example-lyrics",
    display_name: "Example Lyrics",
}

pub struct ExampleOutputSink;
pub struct ExampleOutputSession;

impl PluginLifecycle for ExampleOutputSink {}
impl ConfigStateOps for ExampleOutputSession {}

impl OutputSinkSession for ExampleOutputSession {
    fn list_targets_json(&mut self) -> SdkResult<String> {
        Ok("[]".to_string())
    }

    fn negotiate_spec_json(
        &mut self,
        _target_json: &str,
        desired: AudioSpec,
    ) -> SdkResult<NegotiatedSpec> {
        Ok(NegotiatedSpec {
            spec: desired,
            preferred_chunk_frames: 1024,
            prefer_track_rate: true,
        })
    }

    fn describe_hot_path(&mut self, _spec: AudioSpec) -> SdkResult<Option<CoreModuleSpec>> {
        Ok(None)
    }

    fn open_json(&mut self, _target_json: &str, _spec: AudioSpec) -> SdkResult<()> {
        Ok(())
    }

    fn write_interleaved_f32(&mut self, channels: u16, interleaved_f32le: &[u8]) -> SdkResult<u32> {
        if channels == 0 {
            return Ok(0);
        }
        let bytes_per_frame = channels as usize * 4;
        Ok((interleaved_f32le.len() / bytes_per_frame) as u32)
    }

    fn query_status(&mut self) -> SdkResult<OutputSinkStatus> {
        Ok(OutputSinkStatus {
            queued_samples: 0,
            running: true,
        })
    }

    fn flush(&mut self) -> SdkResult<()> {
        Ok(())
    }

    fn reset(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

impl OutputSinkPlugin for ExampleOutputSink {
    type Session = ExampleOutputSession;

    const TYPE_ID: &'static str = "example-output";
    const DISPLAY_NAME: &'static str = "Example Output";

    fn create_session(&mut self) -> SdkResult<Self::Session> {
        Ok(ExampleOutputSession)
    }
}

fn create_output_sink() -> SdkResult<ExampleOutputSink> {
    Ok(ExampleOutputSink)
}

stellatune_plugin_sdk::export_output_sink_plugin! {
    export: output_export,
    plugin_type: crate::ExampleOutputSink,
    create: crate::create_output_sink,
    plugin_id: "dev.stellatune.example.output",
    component_id: "output-main",
    type_id: "example-output",
    display_name: "Example Output",
}

pub struct ExampleDsp;
pub struct ExampleProcessor;

impl PluginLifecycle for ExampleDsp {}
impl ConfigStateOps for ExampleProcessor {}

impl DspProcessor for ExampleProcessor {
    fn describe_hot_path(&mut self, _spec: AudioSpec) -> SdkResult<Option<CoreModuleSpec>> {
        Ok(None)
    }

    fn process_interleaved_f32(
        &mut self,
        _channels: u16,
        interleaved_f32le: &[u8],
    ) -> SdkResult<Vec<u8>> {
        Ok(interleaved_f32le.to_vec())
    }

    fn supported_layouts(&self) -> u32 {
        0
    }

    fn output_channels(&self) -> u16 {
        0
    }
}

impl DspPlugin for ExampleDsp {
    type Processor = ExampleProcessor;

    const TYPE_ID: &'static str = "example-dsp";
    const DISPLAY_NAME: &'static str = "Example DSP";

    fn create_processor(&mut self, _spec: AudioSpec) -> SdkResult<Self::Processor> {
        Ok(ExampleProcessor)
    }
}

fn create_dsp() -> SdkResult<ExampleDsp> {
    Ok(ExampleDsp)
}

stellatune_plugin_sdk::export_dsp_plugin! {
    export: dsp_export,
    plugin_type: crate::ExampleDsp,
    create: crate::create_dsp,
    plugin_id: "dev.stellatune.example.dsp",
    component_id: "dsp-main",
    type_id: "example-dsp",
    display_name: "Example DSP",
}

fn main() {}
