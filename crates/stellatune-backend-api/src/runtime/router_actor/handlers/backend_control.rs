use std::time::Instant;

use stellatune_core::{PluginRuntimeEvent, PluginRuntimeKind};
use stellatune_plugin_protocol::PluginControlRequest;
use stellatune_plugins::runtime::backend_control::{BackendControlRequest, BackendControlResponse};
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::RuntimeRouterActor;
use crate::runtime::bus::{
    ControlFinishedArgs, build_control_result_payload, emit_control_finished, emit_runtime_event,
    push_plugin_host_event_json,
};
use crate::runtime::control::{control_wait_kind, route_plugin_control_request};
use crate::runtime::types::{CONTROL_FINISH_TIMEOUT, ControlWaitKind, PendingControlFinish};

pub(crate) struct BackendControlMessage {
    pub(crate) request: BackendControlRequest,
}

impl Message for BackendControlMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<BackendControlMessage> for RuntimeRouterActor {
    async fn handle(
        &mut self,
        message: BackendControlMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        let engine = self.engine.clone();
        let library = self.library.clone();

        let parsed_request =
            match serde_json::from_str::<PluginControlRequest>(&message.request.request_json) {
                Ok(parsed) => Some(parsed),
                Err(err) => {
                    let error = format!("invalid json: {err}");
                    let _ = message
                        .request
                        .response_tx
                        .send(BackendControlResponse::error(-1, error));
                    None
                }
            };

        let route_result = match parsed_request.as_ref() {
            Some(parsed) => {
                route_plugin_control_request(parsed, engine.as_ref(), library.as_ref()).await
            }
            None => Err("invalid control request json".to_string()),
        };

        let payload = build_control_result_payload(
            parsed_request.as_ref(),
            route_result.as_ref().err().map(String::as_str),
        );
        let response_json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());

        let response = match &route_result {
            Ok(()) => BackendControlResponse::ok(response_json.clone()),
            Err(err) => BackendControlResponse::error(-1, err.clone()),
        };
        let _ = message.request.response_tx.send(response);

        push_plugin_host_event_json(&message.request.plugin_id, response_json).await;

        let runtime_event = PluginRuntimeEvent::from_payload(
            message.request.plugin_id.clone(),
            PluginRuntimeKind::ControlResult,
            &payload,
        )
        .unwrap_or_else(|_| PluginRuntimeEvent {
            plugin_id: message.request.plugin_id.clone(),
            kind: PluginRuntimeKind::ControlResult,
            payload_json: "{}".to_string(),
        });
        emit_runtime_event(self.router.runtime_hub.as_ref(), runtime_event);

        match route_result {
            Ok(()) => {
                let request_id = parsed_request
                    .as_ref()
                    .and_then(PluginControlRequest::request_id)
                    .cloned();
                let scope = parsed_request
                    .as_ref()
                    .map(PluginControlRequest::scope)
                    .unwrap_or(stellatune_core::ControlScope::Player);
                let command = parsed_request
                    .as_ref()
                    .map(PluginControlRequest::control_command);
                let wait = parsed_request
                    .as_ref()
                    .map(control_wait_kind)
                    .unwrap_or(ControlWaitKind::Immediate);

                if wait == ControlWaitKind::Immediate {
                    emit_control_finished(
                        self.router.runtime_hub.as_ref(),
                        ControlFinishedArgs {
                            plugin_id: &message.request.plugin_id,
                            request_id,
                            scope,
                            command,
                            error: None,
                        },
                    )
                    .await;
                } else {
                    self.pending_finishes.push(PendingControlFinish {
                        plugin_id: message.request.plugin_id,
                        request_id,
                        scope,
                        command,
                        wait,
                        deadline: Instant::now() + CONTROL_FINISH_TIMEOUT,
                    });
                }
            }
            Err(err) => {
                tracing::warn!(
                    plugin_id = %message.request.plugin_id,
                    payload = %message.request.request_json,
                    error = %err,
                    "failed to route plugin control"
                );

                let request_id = parsed_request
                    .as_ref()
                    .and_then(PluginControlRequest::request_id)
                    .cloned();
                let scope = parsed_request
                    .as_ref()
                    .map(PluginControlRequest::scope)
                    .unwrap_or(stellatune_core::ControlScope::Player);
                let command = parsed_request
                    .as_ref()
                    .map(PluginControlRequest::control_command);

                emit_control_finished(
                    self.router.runtime_hub.as_ref(),
                    ControlFinishedArgs {
                        plugin_id: &message.request.plugin_id,
                        request_id,
                        scope,
                        command,
                        error: Some(&err),
                    },
                )
                .await;
            }
        }
    }
}
