use std::collections::HashMap;

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

#[derive(Debug, Clone, Default)]
pub struct RuntimeIntrospectionReadCache {
    pub(crate) capabilities_by_plugin: HashMap<String, Vec<CapabilityDescriptor>>,
    pub(crate) capability_index:
        HashMap<String, HashMap<CapabilityKind, HashMap<String, CapabilityDescriptor>>>,
    pub(crate) decoder_candidates_by_ext: HashMap<String, Vec<DecoderCandidate>>,
    pub(crate) decoder_candidates_wildcard: Vec<DecoderCandidate>,
}

impl RuntimeIntrospectionReadCache {
    pub fn capability_plugin_ids(&self) -> Vec<String> {
        self.capabilities_by_plugin.keys().cloned().collect()
    }

    pub fn list_capabilities(&self, plugin_id: &str) -> Vec<CapabilityDescriptor> {
        self.capabilities_by_plugin
            .get(plugin_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn find_capability(
        &self,
        plugin_id: &str,
        kind: CapabilityKind,
        type_id: &str,
    ) -> Option<CapabilityDescriptor> {
        self.capability_index
            .get(plugin_id)
            .and_then(|by_kind| by_kind.get(&kind))
            .and_then(|by_type| by_type.get(type_id))
            .cloned()
    }

    pub fn list_decoder_candidates_for_ext(&self, ext: &str) -> Vec<DecoderCandidate> {
        let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if ext.is_empty() {
            return Vec::new();
        }
        self.decoder_candidates_by_ext
            .get(ext.as_str())
            .cloned()
            .unwrap_or_else(|| self.decoder_candidates_wildcard.clone())
    }

    pub fn decoder_supported_extensions(&self) -> Vec<String> {
        let mut out: Vec<String> = self.decoder_candidates_by_ext.keys().cloned().collect();
        out.sort();
        out
    }

    pub fn decoder_has_wildcard_candidate(&self) -> bool {
        !self.decoder_candidates_wildcard.is_empty()
    }
}
