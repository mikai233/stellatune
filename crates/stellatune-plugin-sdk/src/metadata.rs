pub use stellatune_plugin_api::{PluginMetadata, PluginMetadataVersion};

pub fn build_plugin_metadata(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
) -> PluginMetadata {
    PluginMetadata::new(
        id,
        name,
        crate::STELLATUNE_PLUGIN_API_VERSION,
        PluginMetadataVersion::new(major, minor, patch),
    )
}

pub fn build_plugin_metadata_with_info(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
    info: Option<serde_json::Map<String, serde_json::Value>>,
) -> PluginMetadata {
    build_plugin_metadata(id, name, major, minor, patch).with_info(info)
}
