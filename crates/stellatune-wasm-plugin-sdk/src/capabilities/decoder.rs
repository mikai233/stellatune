use crate::capabilities::{AbilityDescriptor, AbilityKind, ConfigStateOps};
use crate::common::{DecoderInfo, MediaMetadata, PcmF32Chunk, SeekWhence};
use crate::error::SdkResult;
use crate::lifecycle::PluginLifecycle;

pub trait DecoderInputStream: Send {
    fn read(&mut self, max_bytes: u32) -> SdkResult<Vec<u8>>;
    fn seek(&mut self, offset: i64, whence: SeekWhence) -> SdkResult<u64>;
    fn tell(&mut self) -> SdkResult<u64>;
    fn size(&mut self) -> SdkResult<u64>;
}

pub struct DecoderInput<'a> {
    pub stream: &'a mut dyn DecoderInputStream,
    pub ext_hint: Option<&'a str>,
}

pub trait DecoderSession: ConfigStateOps + Send {
    fn info(&self) -> SdkResult<DecoderInfo>;
    fn metadata(&self) -> SdkResult<MediaMetadata>;
    fn read_pcm_f32(&mut self, max_frames: u32) -> SdkResult<PcmF32Chunk>;
    fn seek_ms(&mut self, position_ms: u64) -> SdkResult<()>;
    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait DecoderPlugin: PluginLifecycle + Send + 'static {
    type Session: DecoderSession;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn descriptor() -> AbilityDescriptor {
        AbilityDescriptor {
            kind: AbilityKind::Decoder,
            type_id: Self::TYPE_ID,
            display_name: Self::DISPLAY_NAME,
            config_schema_json: Self::CONFIG_SCHEMA_JSON,
            default_config_json: Self::DEFAULT_CONFIG_JSON,
        }
    }

    fn open(&mut self, input: DecoderInput<'_>) -> SdkResult<Self::Session>;
}
