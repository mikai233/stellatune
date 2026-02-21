use crate::capabilities::{AbilityDescriptor, AbilityKind, ConfigStateOps};
use crate::common::{AudioSpec, CoreModuleSpec};
use crate::error::SdkResult;
use crate::lifecycle::PluginLifecycle;

pub trait DspProcessor: ConfigStateOps + Send {
    fn describe_hot_path(&mut self, spec: AudioSpec) -> SdkResult<Option<CoreModuleSpec>>;
    fn process_interleaved_f32(
        &mut self,
        channels: u16,
        interleaved_f32le: &[u8],
    ) -> SdkResult<Vec<u8>>;
    fn supported_layouts(&self) -> u32;
    fn output_channels(&self) -> u16;
    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait DspPlugin: PluginLifecycle + Send + 'static {
    type Processor: DspProcessor;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn descriptor() -> AbilityDescriptor {
        AbilityDescriptor {
            kind: AbilityKind::Dsp,
            type_id: Self::TYPE_ID,
            display_name: Self::DISPLAY_NAME,
            config_schema_json: Self::CONFIG_SCHEMA_JSON,
            default_config_json: Self::DEFAULT_CONFIG_JSON,
        }
    }

    fn create_processor(&mut self, spec: AudioSpec) -> SdkResult<Self::Processor>;
}
