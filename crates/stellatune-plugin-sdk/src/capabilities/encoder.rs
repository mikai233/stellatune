use crate::capabilities::{AbilityDescriptor, AbilityKind, ConfigStateOps};
use crate::common::{AudioSpec, EncodedAudioFormat, EncodedChunk, MediaMetadata, PcmF32Chunk};
use crate::error::SdkResult;
use crate::lifecycle::PluginLifecycle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeTarget {
    pub format: EncodedAudioFormat,
    pub ext_hint: Option<String>,
    pub options_json: Option<String>,
}

pub trait EncoderSession: ConfigStateOps + Send {
    fn input_spec(&self) -> AudioSpec;
    fn output_format(&self) -> SdkResult<EncodedAudioFormat>;
    fn write_pcm_f32(&mut self, chunk: PcmF32Chunk) -> SdkResult<u32>;
    fn read_encoded(&mut self, max_bytes: u32) -> SdkResult<EncodedChunk>;
    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait EncoderPlugin: PluginLifecycle + Send + 'static {
    type Session: EncoderSession;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn descriptor() -> AbilityDescriptor {
        AbilityDescriptor {
            kind: AbilityKind::Encoder,
            type_id: Self::TYPE_ID,
            display_name: Self::DISPLAY_NAME,
            config_schema_json: Self::CONFIG_SCHEMA_JSON,
            default_config_json: Self::DEFAULT_CONFIG_JSON,
        }
    }

    fn create_session(
        &mut self,
        input: AudioSpec,
        target: EncodeTarget,
        metadata: Option<MediaMetadata>,
    ) -> SdkResult<Self::Session>;
}
