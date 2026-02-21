use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::manifest::{AbilityKind, ComponentSpec, DecoderAbilitySpec, WasmPluginManifest};
use crate::runtime::model::{
    DesiredPluginState, RuntimeCapabilityDescriptor, RuntimeDecoderExtScore, RuntimePluginInfo,
    RuntimePluginLifecycleState, RuntimePluginStatus,
};

#[derive(Debug, Clone)]
pub(crate) struct ActivePlugin {
    pub(crate) info: RuntimePluginInfo,
    pub(crate) capabilities: Vec<RuntimeCapabilityDescriptor>,
    pub(crate) signature: String,
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeRegistry {
    pub(crate) revision: u64,
    pub(crate) active_plugins: BTreeMap<String, ActivePlugin>,
    pub(crate) desired_states: BTreeMap<String, DesiredPluginState>,
    pub(crate) last_discovered_plugin_ids: BTreeSet<String>,
    pub(crate) last_errors_by_plugin: BTreeMap<String, String>,
}

pub(crate) fn build_plugin_statuses(
    active_plugins: &BTreeMap<String, ActivePlugin>,
    discovered_plugin_ids: &BTreeSet<String>,
    desired_states: &BTreeMap<String, DesiredPluginState>,
    errors_by_plugin: &BTreeMap<String, String>,
) -> Vec<RuntimePluginStatus> {
    let mut plugin_ids = BTreeSet::<String>::new();
    plugin_ids.extend(active_plugins.keys().cloned());
    plugin_ids.extend(discovered_plugin_ids.iter().cloned());
    plugin_ids.extend(desired_states.keys().cloned());
    plugin_ids.extend(errors_by_plugin.keys().cloned());

    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let desired_state = desired_states
            .get(&plugin_id)
            .copied()
            .unwrap_or(DesiredPluginState::Enabled);
        let lifecycle_state = if active_plugins.contains_key(&plugin_id) {
            RuntimePluginLifecycleState::Active
        } else if !discovered_plugin_ids.contains(&plugin_id) {
            RuntimePluginLifecycleState::Missing
        } else if desired_state == DesiredPluginState::Disabled {
            RuntimePluginLifecycleState::Disabled
        } else if discovered_plugin_ids.contains(&plugin_id) {
            RuntimePluginLifecycleState::Failed
        } else {
            RuntimePluginLifecycleState::Missing
        };
        let last_error = errors_by_plugin.get(&plugin_id).cloned();
        out.push(RuntimePluginStatus {
            plugin_id,
            desired_state,
            lifecycle_state,
            last_error,
        });
    }
    out
}

pub(crate) fn active_plugin_from_manifest(
    root_dir: PathBuf,
    manifest_path: PathBuf,
    manifest: WasmPluginManifest,
) -> ActivePlugin {
    let info = RuntimePluginInfo {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        root_dir,
        manifest_path,
        component_count: manifest.components.len(),
    };
    let capabilities = manifest
        .components
        .iter()
        .flat_map(|component| capabilities_from_component(&manifest.id, component))
        .collect::<Vec<_>>();
    let signature = plugin_signature(&manifest);
    ActivePlugin {
        info,
        capabilities,
        signature,
    }
}

fn capabilities_from_component(
    plugin_id: &str,
    component: &ComponentSpec,
) -> Vec<RuntimeCapabilityDescriptor> {
    component
        .abilities
        .iter()
        .map(|ability| {
            let (decoder_ext_scores, decoder_wildcard_score) = decoder_rules_for_ability(ability);
            RuntimeCapabilityDescriptor {
                plugin_id: plugin_id.to_string(),
                component_id: component.id.clone(),
                component_rel_path: component.path.clone(),
                world: component.world.clone(),
                kind: ability.kind,
                type_id: ability.type_id.clone(),
                display_name: ability
                    .display_name
                    .clone()
                    .unwrap_or_else(|| ability.type_id.clone()),
                config_schema_json: ability
                    .config_schema_json
                    .clone()
                    .unwrap_or_else(|| "{}".to_string()),
                default_config_json: ability
                    .default_config_json
                    .clone()
                    .unwrap_or_else(|| "{}".to_string()),
                decoder_ext_scores,
                decoder_wildcard_score,
            }
        })
        .collect()
}

fn decoder_rules_for_ability(
    ability: &crate::manifest::AbilitySpec,
) -> (Vec<RuntimeDecoderExtScore>, u16) {
    if ability.kind != AbilityKind::Decoder {
        return (Vec::new(), 0);
    }
    let Some(DecoderAbilitySpec {
        ext_scores,
        wildcard_score,
    }) = ability.decoder.as_ref()
    else {
        return (Vec::new(), 0);
    };

    let mut dedup = BTreeMap::<String, u16>::new();
    for rule in ext_scores {
        let ext = rule.ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if ext.is_empty() || ext == "*" {
            continue;
        }
        dedup
            .entry(ext)
            .and_modify(|score| *score = (*score).max(rule.score))
            .or_insert(rule.score);
    }
    let ext_scores = dedup
        .into_iter()
        .map(|(ext, score)| RuntimeDecoderExtScore { ext, score })
        .collect::<Vec<_>>();
    let wildcard = wildcard_score.unwrap_or(0);
    (ext_scores, wildcard)
}

fn plugin_signature(manifest: &WasmPluginManifest) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    manifest.hash(&mut hasher);
    format!("stdhash64:{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{AbilitySpec, DecoderAbilitySpec, DecoderExtScoreSpec};

    #[test]
    fn decoder_rules_default_to_zero_when_not_configured() {
        let ability = AbilitySpec {
            kind: AbilityKind::Decoder,
            type_id: "demo".to_string(),
            display_name: None,
            config_schema_json: None,
            default_config_json: None,
            decoder: None,
        };
        let (ext_scores, wildcard) = decoder_rules_for_ability(&ability);
        assert!(ext_scores.is_empty());
        assert_eq!(wildcard, 0);
    }

    #[test]
    fn decoder_rules_use_manifest_values() {
        let ability = AbilitySpec {
            kind: AbilityKind::Decoder,
            type_id: "demo".to_string(),
            display_name: None,
            config_schema_json: None,
            default_config_json: None,
            decoder: Some(DecoderAbilitySpec {
                ext_scores: vec![
                    DecoderExtScoreSpec {
                        ext: "ncm".to_string(),
                        score: 70,
                    },
                    DecoderExtScoreSpec {
                        ext: ".NCM".to_string(),
                        score: 100,
                    },
                ],
                wildcard_score: Some(9),
            }),
        };
        let (ext_scores, wildcard) = decoder_rules_for_ability(&ability);
        assert_eq!(wildcard, 9);
        assert_eq!(ext_scores.len(), 1);
        assert_eq!(ext_scores[0].ext, "ncm");
        assert_eq!(ext_scores[0].score, 100);
    }

    fn test_manifest(id: &str, version: &str) -> WasmPluginManifest {
        WasmPluginManifest {
            schema_version: 1,
            id: id.to_string(),
            name: "Test Plugin".to_string(),
            version: version.to_string(),
            api_version: 1,
            components: vec![ComponentSpec {
                id: "main".to_string(),
                path: "plugin.wasm".to_string(),
                world: "decoder-plugin".to_string(),
                abilities: vec![AbilitySpec {
                    kind: AbilityKind::Decoder,
                    type_id: "test.decoder".to_string(),
                    display_name: Some("Test Decoder".to_string()),
                    config_schema_json: Some("{}".to_string()),
                    default_config_json: Some("{}".to_string()),
                    decoder: Some(DecoderAbilitySpec {
                        ext_scores: vec![DecoderExtScoreSpec {
                            ext: "ncm".to_string(),
                            score: 100,
                        }],
                        wildcard_score: Some(1),
                    }),
                }],
            }],
        }
    }

    #[test]
    fn plugin_signature_is_stable_for_same_manifest() {
        let manifest = test_manifest("dev.stellatune.test", "1.0.0");
        let a = plugin_signature(&manifest);
        let b = plugin_signature(&manifest);
        assert_eq!(a, b);
        assert!(a.starts_with("stdhash64:"));
    }

    #[test]
    fn plugin_signature_changes_when_manifest_changes() {
        let a = plugin_signature(&test_manifest("dev.stellatune.test", "1.0.0"));
        let b = plugin_signature(&test_manifest("dev.stellatune.test", "1.0.1"));
        assert_ne!(a, b);
    }
}
