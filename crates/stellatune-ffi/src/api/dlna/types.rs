use serde::{Deserialize, Serialize};

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaSsdpDevice {
    pub usn: String,
    pub st: String,
    pub location: String,
    pub server: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaRenderer {
    pub usn: String,
    pub location: String,
    pub friendly_name: String,
    pub av_transport_control_url: Option<String>,
    pub av_transport_service_type: Option<String>,
    pub rendering_control_url: Option<String>,
    pub rendering_control_service_type: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaHttpServerInfo {
    pub listen_addr: String,
    pub base_url: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaTransportInfo {
    pub current_transport_state: String,
    pub current_transport_status: Option<String>,
    pub current_speed: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaPositionInfo {
    pub rel_time_ms: u64,
    pub track_duration_ms: Option<u64>,
}
