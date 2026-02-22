use std::collections::HashSet;

use stellatune_plugins::manifest::WasmPluginManifest;

const ACTION_CONFIG_GET: &str = "config.get";
const ACTION_CONFIG_APPLY: &str = "config.apply";
const PERMISSION_ACTION_ANY: &str = "action.any";
const PERMISSION_CONFIG_READ: &str = "config.read";
const PERMISSION_CONFIG_WRITE: &str = "config.write";

pub(super) fn ensure_action_allowed(
    manifest: &WasmPluginManifest,
    action: &str,
) -> Result<(), String> {
    let action = action.trim();
    if action.is_empty() {
        return Err("action must not be empty".to_string());
    }
    if is_action_allowed(manifest, action) {
        return Ok(());
    }

    let required = required_permission_hint(action);
    Err(format!(
        "action `{action}` is forbidden by plugin ui permission whitelist (need `{required}`)"
    ))
}

fn is_action_allowed(manifest: &WasmPluginManifest, action: &str) -> bool {
    let permissions = normalized_permissions(manifest);
    if permissions.is_empty() {
        return matches!(action, ACTION_CONFIG_GET | ACTION_CONFIG_APPLY);
    }

    if permissions.contains(PERMISSION_ACTION_ANY) {
        return true;
    }

    let action_perm = format!("action.{action}");
    if permissions.contains(action_perm.as_str()) {
        return true;
    }

    match action {
        ACTION_CONFIG_GET => permissions.contains(PERMISSION_CONFIG_READ),
        ACTION_CONFIG_APPLY => permissions.contains(PERMISSION_CONFIG_WRITE),
        _ => false,
    }
}

fn normalized_permissions(manifest: &WasmPluginManifest) -> HashSet<String> {
    manifest
        .ui
        .as_ref()
        .map(|spec| {
            spec.permissions
                .iter()
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default()
}

fn required_permission_hint(action: &str) -> &'static str {
    match action {
        ACTION_CONFIG_GET => "config.read or action.config.get",
        ACTION_CONFIG_APPLY => "config.write or action.config.apply",
        _ => "action.any or action.<name>",
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_action_allowed;
    use stellatune_plugins::manifest::{
        ComponentSpec, PluginUiMobileSupport, PluginUiMode, PluginUiSpec, WasmPluginManifest,
    };

    fn base_manifest(permissions: Vec<&str>) -> WasmPluginManifest {
        WasmPluginManifest {
            schema_version: 1,
            id: "demo".to_string(),
            name: "demo".to_string(),
            version: "0.1.0".to_string(),
            api_version: 1,
            components: vec![ComponentSpec {
                id: "component".to_string(),
                path: "plugin.wasm".to_string(),
                world: "world".to_string(),
                abilities: Vec::new(),
            }],
            ui: Some(PluginUiSpec {
                mode: PluginUiMode::Web,
                entry: "ui/index.html".to_string(),
                permissions: permissions.into_iter().map(str::to_string).collect(),
                mobile_support: PluginUiMobileSupport::Limited,
            }),
        }
    }

    #[test]
    fn defaults_allow_builtin_actions_when_permissions_empty() {
        let manifest = base_manifest(Vec::new());
        assert!(ensure_action_allowed(&manifest, "config.get").is_ok());
        assert!(ensure_action_allowed(&manifest, "config.apply").is_ok());
    }

    #[test]
    fn action_whitelist_blocks_unlisted_action() {
        let manifest = base_manifest(vec!["config.read"]);
        assert!(ensure_action_allowed(&manifest, "config.get").is_ok());
        assert!(ensure_action_allowed(&manifest, "config.apply").is_err());
    }
}
