use std::convert::Infallible;
use std::time::Duration;

use axum::Json;
use axum::extract::{OriginalUri, Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Redirect, Response, Sse};
use futures_util::stream::{self, StreamExt};
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::plugin_ui_gateway::auth;
use crate::plugin_ui_gateway::dev;
use crate::plugin_ui_gateway::model::{ActionInvokeResponse, HealthResponse, PluginConfigResponse};
use crate::plugin_ui_gateway::permissions;
use crate::plugin_ui_gateway::runtime_apply;
use crate::plugin_ui_gateway::state::{GatewayState, HttpResult};
use crate::plugin_ui_gateway::storage;

pub(super) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

pub(super) async fn serve_plugin_ui_index(
    AxPath(plugin_id): AxPath<String>,
    State(state): State<GatewayState>,
) -> HttpResult<Response> {
    let _plugin_root = storage::resolve_plugin_root(&state, &plugin_id).await?;
    if let Some(dev_origin) = state.dev_origin_for_plugin(plugin_id.as_str()) {
        let redirect_url = dev::build_dev_ui_redirect_url(
            dev_origin,
            None,
            plugin_id.as_str(),
            state.session_token(),
            state.gateway_origin(),
        )
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        return Ok(Redirect::temporary(redirect_url.as_str()).into_response());
    }
    storage::serve_plugin_ui_index(&state, &plugin_id).await
}

pub(super) async fn redirect_plugin_ui_index(
    AxPath(plugin_id): AxPath<String>,
    State(state): State<GatewayState>,
    OriginalUri(original_uri): OriginalUri,
) -> HttpResult<Response> {
    let _plugin_root = storage::resolve_plugin_root(&state, &plugin_id).await?;
    let mut target = format!("/ui/{plugin_id}/");
    if let Some(query) = original_uri.query() {
        target.push('?');
        target.push_str(query);
    }
    Ok(Redirect::temporary(target.as_str()).into_response())
}

pub(super) async fn serve_plugin_ui_asset(
    AxPath((plugin_id, path)): AxPath<(String, String)>,
    State(state): State<GatewayState>,
) -> HttpResult<Response> {
    let _plugin_root = storage::resolve_plugin_root(&state, &plugin_id).await?;
    if let Some(dev_origin) = state.dev_origin_for_plugin(plugin_id.as_str()) {
        let redirect_url = dev::build_dev_ui_redirect_url(
            dev_origin,
            Some(path.as_str()),
            plugin_id.as_str(),
            state.session_token(),
            state.gateway_origin(),
        )
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        return Ok(Redirect::temporary(redirect_url.as_str()).into_response());
    }
    storage::serve_plugin_ui_asset(&state, &plugin_id, &path).await
}

pub(super) async fn api_preflight(
    State(state): State<GatewayState>,
    headers: HeaderMap,
) -> HttpResult<Response> {
    let origin = auth::validate_origin_header(&state, &headers)?;
    Ok(auth::preflight_response(origin.as_deref()))
}

pub(super) async fn get_plugin_config(
    AxPath(plugin_id): AxPath<String>,
    State(state): State<GatewayState>,
) -> HttpResult<Json<PluginConfigResponse>> {
    let plugin_root = storage::resolve_plugin_root(&state, &plugin_id).await?;
    let config = storage::read_plugin_ui_config(&plugin_root).await?;
    Ok(Json(PluginConfigResponse {
        plugin_id,
        config,
        apply_report: None,
    }))
}

pub(super) async fn put_plugin_config(
    AxPath(plugin_id): AxPath<String>,
    State(state): State<GatewayState>,
    Json(config): Json<Value>,
) -> HttpResult<Json<PluginConfigResponse>> {
    if !config.is_object() {
        return Err((
            StatusCode::BAD_REQUEST,
            "config payload must be a JSON object".to_string(),
        ));
    }
    runtime_apply::validate_config_payload(&config).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid plugin ui config payload: {error}"),
        )
    })?;

    let plugin_root = storage::resolve_plugin_root(&state, &plugin_id).await?;
    let manifest = storage::read_plugin_manifest(&plugin_root).await?;
    permissions::ensure_action_allowed(&manifest, "config.apply")
        .map_err(|error| (StatusCode::FORBIDDEN, error))?;
    storage::write_plugin_ui_config(&plugin_root, &config).await?;
    let apply_report = runtime_apply::apply_config_best_effort(&plugin_id, &config)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to apply plugin config at runtime: {error}"),
            )
        })?;
    let report_json = runtime_apply::build_apply_report_json(&apply_report);
    state
        .publish_plugin_event(
            &plugin_id,
            "config.updated",
            json!({ "apply_report": report_json }),
        )
        .await;

    Ok(Json(PluginConfigResponse {
        plugin_id,
        config,
        apply_report: Some(apply_report),
    }))
}

pub(super) async fn invoke_plugin_action(
    AxPath((plugin_id, action)): AxPath<(String, String)>,
    State(state): State<GatewayState>,
    Json(payload): Json<Value>,
) -> HttpResult<Json<ActionInvokeResponse>> {
    let plugin_root = storage::resolve_plugin_root(&state, &plugin_id).await?;
    let manifest = storage::read_plugin_manifest(&plugin_root).await?;
    let action = action.trim().to_string();
    if action.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "action must not be empty".to_string(),
        ));
    }
    let payload = runtime_apply::normalize_action_payload(payload);
    permissions::ensure_action_allowed(&manifest, action.as_str())
        .map_err(|error| (StatusCode::FORBIDDEN, error))?;

    match action.as_str() {
        "config.get" => {
            let config = storage::read_plugin_ui_config(&plugin_root).await?;
            Ok(Json(ActionInvokeResponse {
                plugin_id,
                action,
                accepted: true,
                message: "config fetched".to_string(),
                data: json!({ "config": config }),
            }))
        },
        "config.apply" => {
            let mut config = storage::read_plugin_ui_config(&plugin_root).await?;
            if let Some(provided) = runtime_apply::action_payload_config(&payload) {
                config = provided.clone();
            }
            runtime_apply::validate_config_payload(&config).map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("invalid config for action `config.apply`: {error}"),
                )
            })?;

            let persist = runtime_apply::action_payload_persist(&payload);
            if persist {
                storage::write_plugin_ui_config(&plugin_root, &config).await?;
            }
            let apply_report = runtime_apply::apply_config_best_effort(&plugin_id, &config)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("failed to apply config via action: {error}"),
                    )
                })?;
            let report_json = runtime_apply::build_apply_report_json(&apply_report);
            state
                .publish_plugin_event(
                    &plugin_id,
                    "config.applied",
                    json!({ "persist": persist, "apply_report": report_json }),
                )
                .await;
            Ok(Json(ActionInvokeResponse {
                plugin_id,
                action,
                accepted: true,
                message: "config applied".to_string(),
                data: json!({
                    "persist": persist,
                    "apply_report": report_json,
                    "config": config,
                }),
            }))
        },
        _ => {
            if let Some(data) =
                runtime_apply::invoke_action_via_host(action.as_str(), &payload).await?
            {
                state
                    .publish_plugin_event(
                        &plugin_id,
                        format!("action.{action}.done"),
                        json!({ "data": data.clone() }),
                    )
                    .await;
                return Ok(Json(ActionInvokeResponse {
                    plugin_id,
                    action,
                    accepted: true,
                    message: "action dispatched to host playback runtime".to_string(),
                    data,
                }));
            }

            let config = storage::read_plugin_ui_config(&plugin_root).await?;
            let Some(data) = runtime_apply::invoke_action_via_source(
                &plugin_id,
                action.as_str(),
                &payload,
                &config,
            )
            .await
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?
            else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    runtime_apply::build_unknown_action_error(action.as_str()),
                ));
            };

            state
                .publish_plugin_event(
                    &plugin_id,
                    format!("action.{action}.done"),
                    json!({ "data": data.clone() }),
                )
                .await;
            Ok(Json(ActionInvokeResponse {
                plugin_id,
                action,
                accepted: true,
                message: "action dispatched to source plugin".to_string(),
                data,
            }))
        },
    }
}

pub(super) async fn stream_plugin_events(
    AxPath(plugin_id): AxPath<String>,
    State(state): State<GatewayState>,
) -> HttpResult<Response> {
    let _plugin_root = storage::resolve_plugin_root(&state, &plugin_id).await?;
    let plugin_id_for_stream = plugin_id.clone();
    let rx = state.subscribe_plugin_events(&plugin_id).await;
    let bootstrap = json!({
        "plugin_id": plugin_id.clone(),
        "event": "ready",
        "message": "plugin ui event stream connected",
    })
    .to_string();
    let live_stream = stream::unfold(
        (rx, plugin_id_for_stream),
        |(mut rx, plugin_id)| async move {
            match rx.recv().await {
                Ok(event) => {
                    let payload =
                        serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
                    Some((
                        Ok::<Event, Infallible>(Event::default().event("event").data(payload)),
                        (rx, plugin_id),
                    ))
                },
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    let payload = json!({
                        "plugin_id": plugin_id.clone(),
                        "event": "lagged",
                        "skipped": skipped,
                    })
                    .to_string();
                    Some((
                        Ok::<Event, Infallible>(Event::default().event("lagged").data(payload)),
                        (rx, plugin_id),
                    ))
                },
                Err(broadcast::error::RecvError::Closed) => None,
            }
        },
    );

    let stream = stream::once(async move {
        Ok::<Event, Infallible>(Event::default().event("ready").data(bootstrap))
    })
    .chain(live_stream);
    let response = Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    );
    Ok(response.into_response())
}
