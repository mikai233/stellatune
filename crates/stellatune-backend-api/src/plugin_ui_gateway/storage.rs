use std::path::{Component, Path, PathBuf};

use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use stellatune_plugins::typescript::manifest::{
    TYPESCRIPT_MANIFEST_FILE_NAME, TypeScriptPluginManifest,
};

use crate::plugin_ui_gateway::state::{GatewayState, HttpResult};

pub(super) async fn resolve_plugin_root(
    state: &GatewayState,
    plugin_id_raw: &str,
) -> HttpResult<PathBuf> {
    let plugin_id = sanitize_plugin_id(plugin_id_raw)?;
    let plugin_root = state.plugins_dir.join(plugin_id);
    let manifest_path = plugin_root.join(TYPESCRIPT_MANIFEST_FILE_NAME);
    let exists = tokio::fs::try_exists(&manifest_path)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to check plugin manifest existence: {error}"),
            )
        })?;
    if !exists {
        return Err((
            StatusCode::NOT_FOUND,
            format!("plugin `{plugin_id}` is not installed"),
        ));
    }
    Ok(plugin_root)
}

pub(super) async fn serve_plugin_ui_index(
    state: &GatewayState,
    plugin_id_raw: &str,
) -> HttpResult<Response> {
    let plugin_root = resolve_plugin_root(state, plugin_id_raw).await?;
    let entry_path = resolve_ui_entry_path(&plugin_root).await?;
    let bytes = tokio::fs::read(&entry_path).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            format!("ui entry not found for plugin `{plugin_id_raw}`"),
        )
    })?;
    let mut response = Body::from(bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header_value(guess_content_type(&entry_path)),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header_value("no-store, no-cache, must-revalidate"),
    );
    Ok(response)
}

pub(super) async fn serve_plugin_ui_asset(
    state: &GatewayState,
    plugin_id_raw: &str,
    requested_path: &str,
) -> HttpResult<Response> {
    let plugin_root = resolve_plugin_root(state, plugin_id_raw).await?;
    let rel_path = sanitize_relative_path(requested_path)?;
    let ui_base_dir = resolve_ui_base_dir(&plugin_root).await?;
    let full_path = ui_base_dir.join(rel_path);
    let bytes = tokio::fs::read(&full_path).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            format!("ui asset not found for plugin `{plugin_id_raw}`"),
        )
    })?;

    let mut response = Body::from(bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header_value(guess_content_type(&full_path)),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header_value("no-store, no-cache, must-revalidate"),
    );
    Ok(response)
}

async fn resolve_ui_entry_path(plugin_root: &Path) -> HttpResult<PathBuf> {
    let entry_rel = read_manifest_ui_entry(plugin_root)
        .await?
        .unwrap_or_else(|| "ui/index.html".to_string());
    let entry_path = plugin_root.join(Path::new(entry_rel.as_str()));
    Ok(entry_path)
}

async fn resolve_ui_base_dir(plugin_root: &Path) -> HttpResult<PathBuf> {
    let entry_path = resolve_ui_entry_path(plugin_root).await?;
    let parent = entry_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| plugin_root.to_path_buf());
    Ok(parent)
}

async fn read_manifest_ui_entry(plugin_root: &Path) -> HttpResult<Option<String>> {
    let manifest = read_plugin_manifest(plugin_root).await?;
    Ok(manifest.ui.map(|ui| ui.entry))
}

pub(super) async fn read_plugin_manifest(
    plugin_root: &Path,
) -> HttpResult<TypeScriptPluginManifest> {
    let manifest_path = plugin_root.join(TYPESCRIPT_MANIFEST_FILE_NAME);
    let raw = tokio::fs::read_to_string(&manifest_path)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read plugin manifest: {error}"),
            )
        })?;
    serde_json::from_str::<TypeScriptPluginManifest>(&raw).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to parse plugin manifest: {error}"),
        )
    })
}

pub(super) async fn read_plugin_ui_config(plugin_root: &Path) -> HttpResult<Value> {
    let config_path = plugin_root.join(".ui-config.json");
    let exists = tokio::fs::try_exists(&config_path).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to check config file: {error}"),
        )
    })?;
    if !exists {
        return Ok(json!({}));
    }
    let raw = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read config file: {error}"),
            )
        })?;
    serde_json::from_str::<Value>(&raw).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("invalid config json for plugin ui: {error}"),
        )
    })
}

pub(super) async fn write_plugin_ui_config(plugin_root: &Path, config: &Value) -> HttpResult<()> {
    let config_path = plugin_root.join(".ui-config.json");
    let text = serde_json::to_string_pretty(config).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode config json: {error}"),
        )
    })?;
    tokio::fs::write(&config_path, text).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to write config file: {error}"),
        )
    })
}

fn sanitize_plugin_id(plugin_id_raw: &str) -> HttpResult<&str> {
    let plugin_id = plugin_id_raw.trim();
    if plugin_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "plugin_id is empty".to_string()));
    }
    if plugin_id.len() > 200 {
        return Err((StatusCode::BAD_REQUEST, "plugin_id is too long".to_string()));
    }
    if plugin_id
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_'))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "plugin_id contains unsupported characters".to_string(),
        ));
    }
    Ok(plugin_id)
}

fn sanitize_relative_path(raw_path: &str) -> HttpResult<PathBuf> {
    let trimmed = raw_path.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Ok(PathBuf::from("index.html"));
    }
    let path = Path::new(trimmed);
    if !is_safe_relative_path(path) {
        return Err((
            StatusCode::BAD_REQUEST,
            "path must be a safe relative path".to_string(),
        ));
    }
    Ok(path.to_path_buf())
}

fn is_safe_relative_path(path: &Path) -> bool {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }
    path.components()
        .all(|part| matches!(part, Component::Normal(_)))
}

fn guess_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn header_value(value: &'static str) -> axum::http::HeaderValue {
    axum::http::HeaderValue::from_static(value)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::guess_content_type;
    use super::is_safe_relative_path;

    #[test]
    fn relative_paths_are_sanitized() {
        assert!(is_safe_relative_path(Path::new("index.html")));
        assert!(is_safe_relative_path(Path::new("assets/main.js")));
        assert!(!is_safe_relative_path(Path::new("")));
        assert!(!is_safe_relative_path(Path::new("../escape.js")));
        assert!(!is_safe_relative_path(Path::new("/etc/passwd")));
    }

    #[test]
    fn content_type_guess_matches_common_web_assets() {
        assert_eq!(
            guess_content_type(Path::new("index.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            guess_content_type(Path::new("assets/app.js")),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(
            guess_content_type(Path::new("assets/icon.svg")),
            "image/svg+xml"
        );
        assert_eq!(
            guess_content_type(Path::new("assets/data.bin")),
            "application/octet-stream"
        );
    }
}
