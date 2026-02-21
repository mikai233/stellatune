mod source;

use stellatune_plugin_sdk::prelude::*;

use crate::source::NeteaseSourceCatalog;

pub struct NeteaseSourcePlugin;

impl PluginLifecycle for NeteaseSourcePlugin {
    fn on_disable(&mut self, _reason: DisableReason) -> SdkResult<()> {
        // Sidecar lifetime is managed by host plugin enabled/disabled state.
        Ok(())
    }
}

impl SourcePlugin for NeteaseSourcePlugin {
    type Catalog = NeteaseSourceCatalog;

    const TYPE_ID: &'static str = source::SOURCE_TYPE_ID;
    const DISPLAY_NAME: &'static str = source::SOURCE_DISPLAY_NAME;
    const CONFIG_SCHEMA_JSON: &'static str = source::CONFIG_SCHEMA_JSON;
    const DEFAULT_CONFIG_JSON: &'static str = source::DEFAULT_CONFIG_JSON;

    fn create_catalog(&mut self) -> SdkResult<Self::Catalog> {
        Ok(NeteaseSourceCatalog::new())
    }
}

fn create_plugin() -> SdkResult<NeteaseSourcePlugin> {
    Ok(NeteaseSourcePlugin)
}

stellatune_plugin_sdk::export_source_component! {
    plugin_type: crate::NeteaseSourcePlugin,
    create: crate::create_plugin,
}
