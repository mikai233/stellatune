use stellatune_plugin_sdk::instance::{DecoderDescriptor, DecoderInstance};
use stellatune_plugin_sdk::update::ConfigUpdatable;
use stellatune_plugin_sdk::{
    ST_DECODER_INFO_FLAG_HAS_DURATION, ST_DECODER_INFO_FLAG_SEEKABLE, SdkResult, StAudioSpec,
    StDecoderInfo,
};

pub struct NoopDecoder {
    build: &'static str,
}

impl ConfigUpdatable for NoopDecoder {}

impl DecoderInstance for NoopDecoder {
    fn get_info(&self) -> StDecoderInfo {
        StDecoderInfo {
            spec: StAudioSpec {
                sample_rate: 48_000,
                channels: 2,
                reserved: 0,
            },
            duration_ms: 0,
            encoder_delay_frames: 0,
            encoder_padding_frames: 0,
            flags: ST_DECODER_INFO_FLAG_SEEKABLE | ST_DECODER_INFO_FLAG_HAS_DURATION,
            reserved: 0,
        }
    }

    fn get_metadata_json(&self) -> SdkResult<Option<String>> {
        Ok(Some(serde_json::json!({ "build": self.build }).to_string()))
    }

    fn read_interleaved_f32(
        &mut self,
        _frames: u32,
        _out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)> {
        Ok((0, true))
    }
}

impl DecoderDescriptor for NoopDecoder {
    type Config = serde_json::Value;
    type Instance = NoopDecoder;

    const TYPE_ID: &'static str = "noop";
    const DISPLAY_NAME: &'static str = "Noop Decoder";
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn default_config() -> Self::Config {
        serde_json::json!({})
    }

    fn create(_config: Self::Config) -> SdkResult<Self::Instance> {
        Ok(NoopDecoder { build: "v2" })
    }
}

stellatune_plugin_sdk::export_plugin! {
    id: "dev.stellatune.test.lifecycle",
    name: "Lifecycle Fixture V2",
    version: (0, 2, 0),
    decoders: [
        noop => NoopDecoder,
    ],
    dsps: [],
    source_catalogs: [],
    lyrics_providers: [],
    output_sinks: [],
}
