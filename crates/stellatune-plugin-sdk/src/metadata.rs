#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PluginMetadataVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub api_version: u32,
    pub version: PluginMetadataVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

impl PluginMetadata {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

pub fn build_plugin_metadata(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
) -> PluginMetadata {
    PluginMetadata {
        id: id.into(),
        name: name.into(),
        api_version: crate::STELLATUNE_PLUGIN_API_VERSION_V1,
        version: PluginMetadataVersion {
            major,
            minor,
            patch,
        },
        info: None,
    }
}

pub fn build_plugin_metadata_with_info(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
    info: Option<serde_json::Value>,
) -> PluginMetadata {
    let mut meta = build_plugin_metadata(id, name, major, minor, patch);
    meta.info = info;
    meta
}

pub fn build_plugin_metadata_json(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
) -> String {
    let meta = build_plugin_metadata(id, name, major, minor, patch);
    match meta.to_json() {
        Ok(s) => s,
        Err(_) => {
            let id = meta.id.replace('\\', "\\\\").replace('"', "\\\"");
            let name = meta.name.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                r#"{{"id":"{id}","name":"{name}","api_version":{},"version":{{"major":{},"minor":{},"patch":{}}}}}"#,
                meta.api_version, meta.version.major, meta.version.minor, meta.version.patch
            )
        }
    }
}

pub fn build_plugin_metadata_json_with_info_json(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
    info_json: Option<&str>,
) -> String {
    let info = info_json.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(v) => Some(v),
            Err(_) => Some(serde_json::Value::String(trimmed.to_string())),
        }
    });
    let meta = build_plugin_metadata_with_info(id, name, major, minor, patch, info);
    match meta.to_json() {
        Ok(s) => s,
        Err(_) => build_plugin_metadata_json(
            meta.id,
            meta.name,
            meta.version.major,
            meta.version.minor,
            meta.version.patch,
        ),
    }
}
