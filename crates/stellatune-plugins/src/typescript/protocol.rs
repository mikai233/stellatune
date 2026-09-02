use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY_RPC_PROTOCOL: &str = "stellatune-capability-rpc/1";
pub const DEFAULT_MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RpcRequest<'a> {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub protocol: &'a str,
    pub generation: u64,
    pub deadline_ms: u64,
    pub method: &'a str,
    pub params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub generation: u64,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<PluginError>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default)]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SourceLocatorDto {
    File {
        path: String,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMediaDto {
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub codec_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapabilitiesDto {
    pub seekable: bool,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRequirementsDto {
    #[serde(default)]
    pub decoder_capability_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePlanDto {
    pub source: SourceLocatorDto,
    #[serde(default)]
    pub media: SourceMediaDto,
    pub capabilities: SourceCapabilitiesDto,
    #[serde(default)]
    pub requirements: SourceRequirementsDto,
}
