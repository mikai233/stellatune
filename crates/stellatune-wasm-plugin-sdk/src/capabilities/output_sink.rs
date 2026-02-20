use crate::capabilities::{AbilityDescriptor, AbilityKind, ConfigStateOps};
use crate::common::{AudioSpec, CoreModuleSpec, NegotiatedSpec, OutputSinkStatus};
use crate::error::SdkResult;
use crate::lifecycle::PluginLifecycle;

pub trait OutputSinkSession: ConfigStateOps + Send {
    fn list_targets_json(&mut self) -> SdkResult<String>;
    fn negotiate_spec_json(
        &mut self,
        target_json: &str,
        desired: AudioSpec,
    ) -> SdkResult<NegotiatedSpec>;
    fn describe_hot_path(&mut self, spec: AudioSpec) -> SdkResult<Option<CoreModuleSpec>>;
    fn open_json(&mut self, target_json: &str, spec: AudioSpec) -> SdkResult<()>;
    fn write_interleaved_f32(&mut self, channels: u16, interleaved_f32le: &[u8]) -> SdkResult<u32>;
    fn query_status(&mut self) -> SdkResult<OutputSinkStatus>;
    fn flush(&mut self) -> SdkResult<()>;
    fn reset(&mut self) -> SdkResult<()>;
    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait OutputSinkPlugin: PluginLifecycle + Send + 'static {
    type Session: OutputSinkSession;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn descriptor() -> AbilityDescriptor {
        AbilityDescriptor {
            kind: AbilityKind::OutputSink,
            type_id: Self::TYPE_ID,
            display_name: Self::DISPLAY_NAME,
            config_schema_json: Self::CONFIG_SCHEMA_JSON,
            default_config_json: Self::DEFAULT_CONFIG_JSON,
        }
    }

    fn create_session(&mut self) -> SdkResult<Self::Session>;
}
