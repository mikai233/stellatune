use stellatune_plugin_sdk::instance::OutputSinkDescriptor;
use stellatune_plugin_sdk::{
    OutputSinkDescriptor as LegacyOutputSinkDescriptor, SdkResult, StAudioSpec,
    StOutputSinkNegotiatedSpec,
};

use crate::client::{ensure_windows, sidecar_get_device_caps, sidecar_list_devices};
use crate::config::{
    AsioOutputConfig, AsioOutputTarget, CONFIG_SCHEMA_JSON, OUTPUT_SINK_DISPLAY_NAME,
    OUTPUT_SINK_TYPE_ID, build_negotiated_spec,
};
use crate::instance::{AsioOutputSinkInstance, create_instance};
use crate::sink::AsioOutputSink;

impl LegacyOutputSinkDescriptor for AsioOutputSink {
    type Config = AsioOutputConfig;
    type Target = AsioOutputTarget;

    const TYPE_ID: &'static str = OUTPUT_SINK_TYPE_ID;
    const DISPLAY_NAME: &'static str = OUTPUT_SINK_DISPLAY_NAME;
    const CONFIG_SCHEMA_JSON: &'static str = CONFIG_SCHEMA_JSON;

    fn default_config() -> Self::Config {
        AsioOutputConfig::default()
    }

    fn list_targets(config: &Self::Config) -> SdkResult<Vec<Self::Target>> {
        ensure_windows()?;
        let devices = sidecar_list_devices(config)?;
        Ok(devices
            .into_iter()
            .map(|d| AsioOutputTarget {
                id: d.id,
                name: Some(d.name),
                selection_session_id: Some(d.selection_session_id),
            })
            .collect())
    }

    fn negotiate_spec(
        desired_spec: StAudioSpec,
        config: &Self::Config,
        target: &Self::Target,
    ) -> SdkResult<StOutputSinkNegotiatedSpec> {
        ensure_windows()?;
        let selection_session_id = target.required_selection_session_id()?;
        let caps = sidecar_get_device_caps(config, selection_session_id, &target.id)?;
        Ok(build_negotiated_spec(desired_spec, &caps, config))
    }

    fn open(spec: StAudioSpec, config: &Self::Config, target: &Self::Target) -> SdkResult<Self> {
        ensure_windows()?;
        let selection_session_id = target.required_selection_session_id()?.to_string();
        AsioOutputSink::open(spec, config, target.id.clone(), selection_session_id)
    }
}

impl OutputSinkDescriptor for AsioOutputSinkInstance {
    type Config = AsioOutputConfig;
    type Instance = AsioOutputSinkInstance;

    const TYPE_ID: &'static str = <AsioOutputSink as LegacyOutputSinkDescriptor>::TYPE_ID;
    const DISPLAY_NAME: &'static str = <AsioOutputSink as LegacyOutputSinkDescriptor>::DISPLAY_NAME;
    const CONFIG_SCHEMA_JSON: &'static str =
        <AsioOutputSink as LegacyOutputSinkDescriptor>::CONFIG_SCHEMA_JSON;

    fn default_config() -> Self::Config {
        AsioOutputConfig::default()
    }

    fn create(config: Self::Config) -> SdkResult<Self::Instance> {
        create_instance(config)
    }
}
