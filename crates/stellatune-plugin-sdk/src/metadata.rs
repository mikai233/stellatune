pub use stellatune_plugin_protocol::{PluginMetadata, PluginMetadataVersion};

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
        crate::STELLATUNE_PLUGIN_API_VERSION_V1,
        PluginMetadataVersion::new(major, minor, patch),
    )
}

pub fn build_plugin_metadata_v2(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
) -> PluginMetadata {
    PluginMetadata::new(
        id,
        name,
        stellatune_plugin_api::v2::STELLATUNE_PLUGIN_API_VERSION_V2,
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

pub fn build_plugin_metadata_with_info_v2(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
    info: Option<serde_json::Map<String, serde_json::Value>>,
) -> PluginMetadata {
    build_plugin_metadata_v2(id, name, major, minor, patch).with_info(info)
}
