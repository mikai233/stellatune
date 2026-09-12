use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BitrateKind {
    Fixed,
    Average,
    Nominal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BitrateMode {
    Cbr,
    Vbr,
    Abr,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitrateInfo {
    pub bps: u32,
    pub kind: BitrateKind,
    pub estimated: bool,
    pub mode: Option<BitrateMode>,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AudioProperties {
    pub format: Option<String>,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub bits_per_sample: Option<u32>,
    pub floating_point: bool,
    pub channels: Option<u32>,
    pub bitrate: Option<BitrateInfo>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProbeStatus {
    Ready,
    Unsupported,
    Invalid,
    BudgetExceeded,
    Cancelled,
    IoError,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub properties: Option<AudioProperties>,
    pub status: ProbeStatus,
    pub bytes_read: u64,
}
