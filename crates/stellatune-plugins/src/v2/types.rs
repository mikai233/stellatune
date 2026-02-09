use crate::runtime::{CapabilityKind, GenerationId};
use stellatune_plugin_api::v2::{StCapabilityDescriptorV2, StCapabilityKindV2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityId(pub u64);

#[derive(Debug, Clone)]
pub struct CapabilityDescriptorInput {
    pub kind: CapabilityKind,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[derive(Debug, Clone)]
pub struct CapabilityDescriptorRecord {
    pub id: CapabilityId,
    pub plugin_id: String,
    pub generation: GenerationId,
    pub kind: CapabilityKind,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[derive(Debug, Clone)]
pub struct PluginGenerationInfo {
    pub id: GenerationId,
    pub metadata_json: String,
    pub activated_at_unix_ms: u64,
}

#[derive(Debug, Clone)]
pub struct PluginSlotSnapshot {
    pub plugin_id: String,
    pub active: Option<PluginGenerationInfo>,
    pub draining: Vec<PluginGenerationInfo>,
}

#[derive(Debug, Clone)]
pub struct ActivationReport {
    pub plugin_id: String,
    pub generation: PluginGenerationInfo,
    pub capabilities: Vec<CapabilityDescriptorRecord>,
}

pub fn capability_kind_from_api(kind: StCapabilityKindV2) -> CapabilityKind {
    match kind {
        StCapabilityKindV2::Decoder => CapabilityKind::Decoder,
        StCapabilityKindV2::Dsp => CapabilityKind::Dsp,
        StCapabilityKindV2::SourceCatalog => CapabilityKind::SourceCatalog,
        StCapabilityKindV2::LyricsProvider => CapabilityKind::LyricsProvider,
        StCapabilityKindV2::OutputSink => CapabilityKind::OutputSink,
    }
}

/// # Safety
/// Caller must ensure `desc` points to a valid descriptor from plugin ABI.
pub unsafe fn capability_input_from_ffi(
    desc: &StCapabilityDescriptorV2,
) -> CapabilityDescriptorInput {
    CapabilityDescriptorInput {
        kind: capability_kind_from_api(desc.kind),
        type_id: unsafe { crate::util::ststr_to_string_lossy(desc.type_id_utf8) },
        display_name: unsafe { crate::util::ststr_to_string_lossy(desc.display_name_utf8) },
        config_schema_json: unsafe {
            crate::util::ststr_to_string_lossy(desc.config_schema_json_utf8)
        },
        default_config_json: unsafe {
            crate::util::ststr_to_string_lossy(desc.default_config_json_utf8)
        },
    }
}
