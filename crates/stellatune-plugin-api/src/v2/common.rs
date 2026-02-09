use crate::StStr;

/// V2 plugin API version.
pub const STELLATUNE_PLUGIN_API_VERSION_V2: u32 = 4;
pub const STELLATUNE_PLUGIN_ENTRY_SYMBOL_V2: &str = "stellatune_plugin_entry_v2";

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StCapabilityKindV2 {
    Decoder = 1,
    Dsp = 2,
    SourceCatalog = 3,
    LyricsProvider = 4,
    OutputSink = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StConfigUpdateModeV2 {
    HotApply = 1,
    Recreate = 2,
    Reject = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StConfigUpdatePlanV2 {
    pub mode: StConfigUpdateModeV2,
    /// Optional plugin-provided diagnostic reason.
    /// If this points to plugin-owned bytes, host must free via module `plugin_free`.
    pub reason_utf8: StStr,
}
