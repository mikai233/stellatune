use core::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use stellatune_core::{
    HostControlFinishedPayload, HostControlResultPayload, HostEventTopic, HostLibraryEventEnvelope,
    HostPlayerEventEnvelope, HostPlayerTickPayload,
};
pub use stellatune_plugin_protocol::{
    HostControlAck, LibraryControl, LibraryListPlaylistTracksQuery, LibraryListTracksQuery,
    LibrarySearchQuery, PlayerControl, PluginControlRequest, RequestId,
};

use crate::{SdkResult, host_poll_event_json, host_send_control_json};

static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
pub enum PluginHostEvent {
    PlayerTick(HostPlayerTickPayload),
    PlayerEvent(HostPlayerEventEnvelope),
    LibraryEvent(HostLibraryEventEnvelope),
    ControlResult(HostControlResultPayload),
    ControlFinished(HostControlFinishedPayload),
    Custom(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerControlRequestBuilder {
    control: PlayerControl,
    request_id: Option<RequestId>,
}

impl PlayerControlRequestBuilder {
    pub fn request_id(mut self, request_id: RequestId) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn request_id_str(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(RequestId::new(request_id));
        self
    }

    pub fn send(&self) -> SdkResult<HostControlAck> {
        PlayerControlExt::send(&self.control, self.request_id.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibraryControlRequestBuilder {
    control: LibraryControl,
    request_id: Option<RequestId>,
}

impl LibraryControlRequestBuilder {
    pub fn request_id(mut self, request_id: RequestId) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn request_id_str(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(RequestId::new(request_id));
        self
    }

    pub fn send(&self) -> SdkResult<HostControlAck> {
        LibraryControlExt::send(&self.control, self.request_id.clone())
    }
}

pub trait PlayerControlExt {
    fn request(self) -> PlayerControlRequestBuilder;

    fn send(&self, request_id: Option<RequestId>) -> SdkResult<HostControlAck>;

    #[cfg(test)]
    fn to_request_json(&self, request_id: Option<RequestId>) -> SdkResult<String>;
}

impl PlayerControlExt for PlayerControl {
    fn request(self) -> PlayerControlRequestBuilder {
        PlayerControlRequestBuilder {
            control: self,
            request_id: None,
        }
    }

    fn send(&self, request_id: Option<RequestId>) -> SdkResult<HostControlAck> {
        let req = PluginControlRequest::player(self.clone(), request_id);
        let request_json = serde_json::to_string(&req).map_err(crate::SdkError::from)?;
        let response_json = host_send_control_json(&request_json)?;
        parse_control_ack_json(&response_json)
    }

    #[cfg(test)]
    fn to_request_json(&self, request_id: Option<RequestId>) -> SdkResult<String> {
        let req = PluginControlRequest::player(self.clone(), request_id);
        serde_json::to_string(&req).map_err(Into::into)
    }
}

pub trait LibraryControlExt {
    fn request(self) -> LibraryControlRequestBuilder;

    fn send(&self, request_id: Option<RequestId>) -> SdkResult<HostControlAck>;

    #[cfg(test)]
    fn to_request_json(&self, request_id: Option<RequestId>) -> SdkResult<String>;
}

impl LibraryControlExt for LibraryControl {
    fn request(self) -> LibraryControlRequestBuilder {
        LibraryControlRequestBuilder {
            control: self,
            request_id: None,
        }
    }

    fn send(&self, request_id: Option<RequestId>) -> SdkResult<HostControlAck> {
        let req = PluginControlRequest::library(self.clone(), request_id);
        let request_json = serde_json::to_string(&req).map_err(crate::SdkError::from)?;
        let response_json = host_send_control_json(&request_json)?;
        parse_control_ack_json(&response_json)
    }

    #[cfg(test)]
    fn to_request_json(&self, request_id: Option<RequestId>) -> SdkResult<String> {
        let req = PluginControlRequest::library(self.clone(), request_id);
        serde_json::to_string(&req).map_err(Into::into)
    }
}

pub fn next_request_id() -> RequestId {
    let seq = REQUEST_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    RequestId::new(format!("req-{ts_ms}-{seq}"))
}

pub fn as_control_result(event: &PluginHostEvent) -> Option<&HostControlResultPayload> {
    match event {
        PluginHostEvent::ControlResult(payload) => Some(payload),
        _ => None,
    }
}

pub fn as_control_finished(event: &PluginHostEvent) -> Option<&HostControlFinishedPayload> {
    match event {
        PluginHostEvent::ControlFinished(payload) => Some(payload),
        _ => None,
    }
}

pub fn control_event_request_id(event: &PluginHostEvent) -> Option<RequestId> {
    match event {
        PluginHostEvent::ControlResult(payload) => payload.request_id.clone(),
        PluginHostEvent::ControlFinished(payload) => payload.request_id.clone(),
        _ => None,
    }
}

pub fn control_event_matches_request_id(event: &PluginHostEvent, request_id: &RequestId) -> bool {
    matches!(control_event_request_id(event), Some(v) if v == *request_id)
}

fn parse_control_ack_json(response_json: &str) -> SdkResult<HostControlAck> {
    serde_json::from_str(response_json).map_err(Into::into)
}

pub(crate) fn parse_host_event_json(event_json: &str) -> SdkResult<PluginHostEvent> {
    let root: Value = serde_json::from_str(event_json).map_err(crate::SdkError::from)?;
    let topic = root
        .get("topic")
        .and_then(|v| v.as_str())
        .and_then(HostEventTopic::from_str);
    let Some(topic) = topic else {
        return Ok(PluginHostEvent::Custom(root));
    };
    match topic {
        HostEventTopic::PlayerTick => serde_json::from_value::<HostPlayerTickPayload>(root)
            .map(PluginHostEvent::PlayerTick)
            .map_err(crate::SdkError::from),
        HostEventTopic::PlayerEvent => serde_json::from_value::<HostPlayerEventEnvelope>(root)
            .map(PluginHostEvent::PlayerEvent)
            .map_err(crate::SdkError::from),
        HostEventTopic::LibraryEvent => serde_json::from_value::<HostLibraryEventEnvelope>(root)
            .map(PluginHostEvent::LibraryEvent)
            .map_err(crate::SdkError::from),
        HostEventTopic::HostControlResult => {
            serde_json::from_value::<HostControlResultPayload>(root)
                .map(PluginHostEvent::ControlResult)
                .map_err(crate::SdkError::from)
        }
        HostEventTopic::HostControlFinished => {
            serde_json::from_value::<HostControlFinishedPayload>(root)
                .map(PluginHostEvent::ControlFinished)
                .map_err(crate::SdkError::from)
        }
    }
}

pub fn host_poll_event() -> SdkResult<Option<PluginHostEvent>> {
    let Some(raw) = host_poll_event_json()? else {
        return Ok(None);
    };
    parse_host_event_json(&raw).map(Some)
}
