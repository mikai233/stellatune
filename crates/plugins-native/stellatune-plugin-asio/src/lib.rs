mod client;
mod config;
mod session;

use stellatune_plugin_sdk::prelude::*;

use crate::session::AsioWasmSession;

pub struct AsioWasmPlugin;

impl PluginLifecycle for AsioWasmPlugin {
    fn on_enable(&mut self) -> SdkResult<()> {
        crate::client::lifecycle_on_enable()
    }

    fn on_disable(&mut self, _reason: DisableReason) -> SdkResult<()> {
        crate::client::lifecycle_on_disable()
    }
}

impl OutputSinkPlugin for AsioWasmPlugin {
    type Session = AsioWasmSession;

    const TYPE_ID: &'static str = crate::config::OUTPUT_SINK_TYPE_ID;
    const DISPLAY_NAME: &'static str = crate::config::OUTPUT_SINK_DISPLAY_NAME;
    const CONFIG_SCHEMA_JSON: &'static str = crate::config::CONFIG_SCHEMA_JSON;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn create_session(&mut self) -> SdkResult<Self::Session> {
        AsioWasmSession::new()
    }
}

fn create_plugin() -> SdkResult<AsioWasmPlugin> {
    Ok(AsioWasmPlugin)
}

stellatune_plugin_sdk::export_output_sink_component! {
    plugin_type: crate::AsioWasmPlugin,
    create: crate::create_plugin,
}
