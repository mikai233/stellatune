use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::manifest::AbilityKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub root_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub component_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDecoderExtScore {
    pub ext: String,
    pub score: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCapabilityDescriptor {
    pub plugin_id: String,
    pub component_id: String,
    pub component_rel_path: String,
    pub world: String,
    pub kind: AbilityKind,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
    #[serde(default)]
    pub decoder_ext_scores: Vec<RuntimeDecoderExtScore>,
    #[serde(default)]
    pub decoder_wildcard_score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeLyricCandidate {
    pub id: String,
    pub title: String,
    pub artist: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuntimeDecoderSessionHandle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuntimeSourceCatalogHandle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuntimeSourceStreamHandle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuntimeOutputSinkSessionHandle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuntimeDspProcessorHandle(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMetadataValue {
    Text(String),
    Boolean(bool),
    Uint32(u32),
    Uint64(u64),
    Int64(i64),
    Float64(f64),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMetadataEntry {
    pub key: String,
    pub value: RuntimeMetadataValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEncodedAudioFormat {
    pub codec: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bitrate_kbps: Option<u32>,
    pub container: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuntimeAudioTags {
    pub title: Option<String>,
    pub album: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
    pub genres: Vec<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMediaMetadata {
    pub tags: RuntimeAudioTags,
    pub duration_ms: Option<u64>,
    pub format: RuntimeEncodedAudioFormat,
    pub extras: Vec<RuntimeMetadataEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEncodedChunk {
    pub bytes: Vec<u8>,
    pub eof: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePcmF32Chunk {
    pub interleaved_f32le: Vec<u8>,
    pub frames: u32,
    pub eof: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDecoderInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub seekable: bool,
    pub encoder_delay_frames: u32,
    pub encoder_padding_frames: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeConfigUpdateMode {
    HotApply,
    Recreate,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfigUpdatePlan {
    pub mode: RuntimeConfigUpdateMode,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeAudioSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeNegotiatedSpec {
    pub spec: RuntimeAudioSpec,
    pub preferred_chunk_frames: u32,
    pub prefer_track_rate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeOutputSinkStatus {
    pub queued_samples: u32,
    pub running: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSampleFormat {
    F32Le,
    I16Le,
    I32Le,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeHotPathRole {
    DspTransform,
    OutputSink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBufferLayout {
    pub in_offset: u32,
    pub out_offset: Option<u32>,
    pub max_frames: u32,
    pub channels: u16,
    pub sample_format: RuntimeSampleFormat,
    pub interleaved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCoreModuleSpec {
    pub role: RuntimeHotPathRole,
    pub wasm_rel_path: String,
    pub abi_version: u32,
    pub memory_export: String,
    pub init_export: String,
    pub process_export: String,
    pub reset_export: Option<String>,
    pub drop_export: Option<String>,
    pub buffer: RuntimeBufferLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredPluginState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDisableReason {
    HostDisable,
    Unload,
    Shutdown,
    Reload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePluginLifecycleState {
    Active,
    Disabled,
    Failed,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePluginStatus {
    pub plugin_id: String,
    pub desired_state: DesiredPluginState,
    pub lifecycle_state: RuntimePluginLifecycleState,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePluginTransitionTrigger {
    LoadNew,
    ReloadChanged,
    DisableRequested,
    RemovedFromDisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePluginTransitionOutcome {
    Applied,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePluginTransition {
    pub plugin_id: String,
    pub from: RuntimePluginLifecycleState,
    pub to: RuntimePluginLifecycleState,
    pub trigger: RuntimePluginTransitionTrigger,
    pub outcome: RuntimePluginTransitionOutcome,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimePluginDirective {
    Destroy { reason: PluginDisableReason },
    Rebuild,
    UpdateConfig { config_json: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeSyncReport {
    pub revision: u64,
    pub discovered_plugins: usize,
    pub active_plugins: Vec<RuntimePluginInfo>,
    pub plugin_statuses: Vec<RuntimePluginStatus>,
    pub transitions: Vec<RuntimePluginTransition>,
    pub errors: Vec<String>,
}
