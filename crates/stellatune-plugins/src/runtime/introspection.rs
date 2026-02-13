use serde::{Deserialize, Serialize};

use stellatune_plugin_api::StCapabilityKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Decoder,
    Dsp,
    SourceCatalog,
    LyricsProvider,
    OutputSink,
}

impl CapabilityKind {
    pub(crate) fn from_st(kind: StCapabilityKind) -> Self {
        match kind {
            StCapabilityKind::Decoder => Self::Decoder,
            StCapabilityKind::Dsp => Self::Dsp,
            StCapabilityKind::SourceCatalog => Self::SourceCatalog,
            StCapabilityKind::LyricsProvider => Self::LyricsProvider,
            StCapabilityKind::OutputSink => Self::OutputSink,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub lease_id: u64,
    pub kind: CapabilityKind,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderCandidate {
    pub plugin_id: String,
    pub type_id: String,
    pub score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLeaseInfo {
    pub lease_id: u64,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLeaseState {
    pub current: Option<PluginLeaseInfo>,
    pub retired_lease_ids: Vec<u64>,
}
