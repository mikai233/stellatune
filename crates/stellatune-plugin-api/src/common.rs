use crate::StStr;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StCapabilityKind {
    Decoder = 1,
    Dsp = 2,
    SourceCatalog = 3,
    LyricsProvider = 4,
    OutputSink = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StConfigUpdateMode {
    HotApply = 1,
    Recreate = 2,
    Reject = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StConfigUpdatePlan {
    pub mode: StConfigUpdateMode,
    /// Optional plugin-provided diagnostic reason.
    /// If this points to plugin-owned bytes, host must free via module `plugin_free`.
    pub reason_utf8: StStr,
}
