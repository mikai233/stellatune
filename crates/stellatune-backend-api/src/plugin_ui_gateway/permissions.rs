use stellatune_plugins::typescript::manifest::TypeScriptPluginManifest;

/// Manifest v2 has no permission mini-language. The host exposes only its
/// fixed gateway API and validates each action at that boundary.
pub(super) fn ensure_action_allowed(
    _manifest: &TypeScriptPluginManifest,
    action: &str,
) -> Result<(), String> {
    if action.trim().is_empty() {
        Err("action must not be empty".to_string())
    } else {
        Ok(())
    }
}
