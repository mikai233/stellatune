use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(String);

impl RequestId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for RequestId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for RequestId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMetadataVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl PluginMetadataVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

#[cfg_attr(feature = "frb", flutter_rust_bridge::frb(ignore))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub api_version: u32,
    pub version: PluginMetadataVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<Map<String, Value>>,
}

impl PluginMetadata {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        api_version: u32,
        version: PluginMetadataVersion,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            api_version,
            version,
            info: None,
        }
    }

    pub fn with_info(mut self, info: Option<Map<String, Value>>) -> Self {
        self.info = info;
        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}
