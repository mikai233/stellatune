use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::plugin_ui_gateway::state::{GatewayState, HttpResult};

pub(super) const TOKEN_HEADER_NAME: &str = "x-stellatune-plugin-ui-token";
const TOKEN_QUERY_KEY: &str = "token";

pub(super) fn generate_session_token() -> String {
    let part_a: u128 = rand::random();
    let part_b: u128 = rand::random();
    format!("{part_a:032x}{part_b:032x}")
}

pub(super) fn build_allowed_origins(local_addr: SocketAddr) -> HashSet<String> {
    let port = local_addr.port();
    let mut origins = HashSet::<String>::new();
    origins.insert(format!("http://localhost:{port}"));
    origins.insert(format!("http://127.0.0.1:{port}"));
    origins.insert(format!("http://[::1]:{port}"));

    match local_addr.ip() {
        IpAddr::V4(v4) => {
            if !v4.is_unspecified() {
                origins.insert(format!("http://{v4}:{port}"));
            }
        },
        IpAddr::V6(v6) => {
            if !v6.is_unspecified() {
                origins.insert(format!("http://[{v6}]:{port}"));
            }
        },
    }

    origins
}

pub(super) fn validate_origin_header(
    state: &GatewayState,
    headers: &HeaderMap,
) -> HttpResult<Option<String>> {
    let Some(origin) = read_origin_header(headers) else {
        return Ok(None);
    };
    if state.is_origin_allowed(origin) {
        return Ok(Some(origin.to_string()));
    }
    Err((
        StatusCode::FORBIDDEN,
        format!("origin `{origin}` is not allowed"),
    ))
}

pub(super) async fn require_api_access(
    State(state): State<GatewayState>,
    request: Request,
    next: Next,
) -> Response {
    let request_origin = read_origin_header(request.headers()).map(str::to_string);
    let validation = validate_origin_header(&state, request.headers()).and_then(|_| {
        if request.method() == Method::OPTIONS {
            return Ok(());
        }
        validate_access_token(&state, request.headers(), request.uri())
    });

    match validation {
        Ok(()) => {
            let mut response = next.run(request).await;
            let cors_origin = request_origin
                .as_deref()
                .filter(|origin| state.is_origin_allowed(origin));
            apply_cors_headers(response.headers_mut(), cors_origin);
            response
        },
        Err((status, message)) => {
            let mut response = (status, message).into_response();
            let cors_origin = request_origin
                .as_deref()
                .filter(|origin| state.is_origin_allowed(origin));
            apply_cors_headers(response.headers_mut(), cors_origin);
            response
        },
    }
}

pub(super) fn preflight_response(origin: Option<&str>) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    apply_cors_headers(response.headers_mut(), origin);
    response
}

fn validate_access_token(state: &GatewayState, headers: &HeaderMap, uri: &Uri) -> HttpResult<()> {
    let Some(provided_token) = read_token_header(headers)
        .map(str::to_string)
        .or_else(|| read_token_query(uri))
    else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "missing plugin ui api session token".to_string(),
        ));
    };
    if provided_token == state.session_token() {
        return Ok(());
    }
    Err((
        StatusCode::UNAUTHORIZED,
        "invalid plugin ui api session token".to_string(),
    ))
}

fn read_origin_header(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn read_token_header(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(TOKEN_HEADER_NAME)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn read_token_query(uri: &Uri) -> Option<String> {
    let query = uri.query()?;
    url::form_urlencoded::parse(query.as_bytes())
        .find_map(|(key, value)| (key == TOKEN_QUERY_KEY).then(|| value.into_owned()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn apply_cors_headers(headers: &mut HeaderMap, origin: Option<&str>) {
    if let Some(origin) = origin
        && let Ok(value) = HeaderValue::from_str(origin)
    {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
        headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET,POST,PUT,OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type,x-stellatune-plugin-ui-token"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    headers.insert(
        HeaderName::from_static("x-stellatune-plugin-ui-token-header"),
        HeaderValue::from_static(TOKEN_HEADER_NAME),
    );
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::read_token_query;

    #[test]
    fn token_query_is_read_from_url() {
        let uri: Uri = "/api/plugins/demo/config?token=abc123"
            .parse()
            .expect("parse uri");
        assert_eq!(read_token_query(&uri).as_deref(), Some("abc123"));
    }
}
