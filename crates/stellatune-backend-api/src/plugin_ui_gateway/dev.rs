use std::collections::HashMap;

use anyhow::{Result, anyhow};
use url::Url;

pub(super) const DEV_UI_ORIGINS_ENV: &str = "STELLATUNE_PLUGIN_UI_DEV_ORIGINS";

pub(super) fn merge_dev_ui_overrides(
    from_options: HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let mut merged = load_env_dev_ui_overrides()?;
    merged.extend(from_options);
    normalize_dev_ui_overrides(merged)
}

pub(super) fn normalize_dev_ui_overrides(
    input: HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let mut out = HashMap::<String, String>::new();
    for (plugin_id_raw, origin_raw) in input {
        let plugin_id = plugin_id_raw.trim();
        if plugin_id.is_empty() {
            continue;
        }
        if !is_safe_plugin_id(plugin_id) {
            return Err(anyhow!(
                "invalid plugin id `{plugin_id}` for dev ui override"
            ));
        }
        let origin = normalize_dev_origin(origin_raw.as_str())?;
        out.insert(plugin_id.to_string(), origin);
    }
    Ok(out)
}

pub(super) fn normalize_dev_origin(raw: &str) -> Result<String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(anyhow!("dev origin is empty"));
    }
    let mut url =
        Url::parse(value).map_err(|error| anyhow!("invalid dev ui origin `{value}`: {error}"))?;
    let scheme = url.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(anyhow!(
            "dev ui origin `{value}` must use http/https scheme"
        ));
    }
    let Some(host) = url.host_str() else {
        return Err(anyhow!("dev ui origin `{value}` is missing host"));
    };
    if !is_local_dev_host(host) {
        return Err(anyhow!(
            "dev ui origin host `{host}` is not allowed (only localhost/loopback)"
        ));
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub(super) fn build_dev_ui_redirect_url(
    dev_origin: &str,
    path: Option<&str>,
    plugin_id: &str,
    token: &str,
    gateway_origin: &str,
) -> Result<String> {
    let mut target = Url::parse(dev_origin)
        .map_err(|error| anyhow!("parse dev ui origin `{dev_origin}` failed: {error}"))?;
    if let Some(path) = path {
        let trimmed = path.trim().trim_start_matches('/');
        if !trimmed.is_empty() {
            target.set_path(trimmed);
        } else {
            target.set_path("/");
        }
    } else {
        target.set_path("/");
    }

    let should_inject_ctx = path.is_none();
    if should_inject_ctx {
        target.query_pairs_mut().append_pair("token", token);
        target
            .query_pairs_mut()
            .append_pair("plugin_id", plugin_id.trim());
        target
            .query_pairs_mut()
            .append_pair("gateway_origin", gateway_origin);
    }
    Ok(target.to_string())
}

pub(super) fn is_safe_plugin_id(plugin_id: &str) -> bool {
    !plugin_id.is_empty()
        && plugin_id.len() <= 200
        && plugin_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_')
}

fn load_env_dev_ui_overrides() -> Result<HashMap<String, String>> {
    let Ok(raw) = std::env::var(DEV_UI_ORIGINS_ENV) else {
        return Ok(HashMap::new());
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(HashMap::new());
    }
    serde_json::from_str::<HashMap<String, String>>(raw).map_err(|error| {
        anyhow!(
            "parse {} failed: expected JSON object string->string ({error})",
            DEV_UI_ORIGINS_ENV
        )
    })
}

fn is_local_dev_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host == "[::1]"
        || host.starts_with("127.")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{build_dev_ui_redirect_url, normalize_dev_ui_overrides};

    #[test]
    fn dev_overrides_are_validated() {
        let mut map = HashMap::<String, String>::new();
        map.insert(
            "dev.stellatune.source.netease".to_string(),
            "http://127.0.0.1:5173".to_string(),
        );
        let normalized = normalize_dev_ui_overrides(map).expect("normalize");
        assert_eq!(
            normalized
                .get("dev.stellatune.source.netease")
                .map(String::as_str),
            Some("http://127.0.0.1:5173")
        );
    }

    #[test]
    fn dev_redirect_injects_context_for_entry() {
        let url = build_dev_ui_redirect_url(
            "http://127.0.0.1:5173",
            None,
            "dev.stellatune.source.netease",
            "abc",
            "http://127.0.0.1:19000",
        )
        .expect("build");
        assert!(url.contains("token=abc"));
        assert!(url.contains("plugin_id=dev.stellatune.source.netease"));
    }
}
