use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use stellatune_plugin_api::{StPluginModule, StStr};

use crate::runtime::introspection::{
    CapabilityDescriptor, CapabilityKind, DecoderCandidate, RuntimeIntrospectionReadCache,
};
use crate::runtime::model::ModuleLease;
use crate::runtime::registry::PluginModuleLeaseSlotState;

use super::{PluginRuntimeActor, lease_id_of};

impl PluginRuntimeActor {
    pub(crate) fn introspection_cache_snapshot(&self) -> Arc<RuntimeIntrospectionReadCache> {
        self.maybe_refresh_introspection_cache();
        self.introspection_cache_local.load_full()
    }

    pub(super) fn mark_introspection_cache_dirty(&self) {
        self.introspection_cache_dirty
            .store(true, Ordering::Release);
    }

    pub(super) fn maybe_refresh_introspection_cache(&self) {
        if !self.introspection_cache_dirty.swap(false, Ordering::AcqRel) {
            return;
        }
        let cache = RuntimeIntrospectionReadCache::build(&self.modules);
        self.introspection_cache_local.store(Arc::new(cache));
    }
}

#[derive(Debug, Default)]
struct DecoderScoreRules {
    exact_by_ext: HashMap<String, u16>,
    wildcard: u16,
}

impl RuntimeIntrospectionReadCache {
    fn build(modules: &HashMap<String, PluginModuleLeaseSlotState>) -> Self {
        let mut capabilities_by_plugin: HashMap<String, Vec<CapabilityDescriptor>> = HashMap::new();
        let mut capability_index: HashMap<
            String,
            HashMap<CapabilityKind, HashMap<String, CapabilityDescriptor>>,
        > = HashMap::new();
        let mut decoder_exact_candidates_by_ext: HashMap<String, Vec<DecoderCandidate>> =
            HashMap::new();
        let mut decoder_candidates_wildcard: Vec<DecoderCandidate> = Vec::new();

        let mut plugin_ids = modules.keys().cloned().collect::<Vec<_>>();
        plugin_ids.sort();
        for plugin_id in plugin_ids {
            let Some(slot) = modules.get(&plugin_id) else {
                continue;
            };
            let Some(lease) = slot.current.as_ref() else {
                continue;
            };

            let capabilities = collect_capabilities_from_lease(lease);
            for capability in &capabilities {
                capability_index
                    .entry(plugin_id.clone())
                    .or_default()
                    .entry(capability.kind)
                    .or_default()
                    .insert(capability.type_id.clone(), capability.clone());

                if capability.kind != CapabilityKind::Decoder {
                    continue;
                }

                let rules = decoder_score_rules_for_capability(
                    &lease.loaded.module,
                    capability.type_id.as_str(),
                );
                for (ext, score) in rules.exact_by_ext {
                    decoder_exact_candidates_by_ext
                        .entry(ext)
                        .or_default()
                        .push(DecoderCandidate {
                            plugin_id: plugin_id.clone(),
                            type_id: capability.type_id.clone(),
                            score,
                        });
                }
                if rules.wildcard > 0 {
                    decoder_candidates_wildcard.push(DecoderCandidate {
                        plugin_id: plugin_id.clone(),
                        type_id: capability.type_id.clone(),
                        score: rules.wildcard,
                    });
                }
            }

            capabilities_by_plugin.insert(plugin_id, capabilities);
        }

        sort_decoder_candidates(&mut decoder_candidates_wildcard);

        let mut decoder_candidates_by_ext = HashMap::new();
        for (ext, exact_candidates) in decoder_exact_candidates_by_ext {
            let mut merged = exact_candidates;
            for wildcard in &decoder_candidates_wildcard {
                let already_covered = merged.iter().any(|item| {
                    item.plugin_id == wildcard.plugin_id && item.type_id == wildcard.type_id
                });
                if already_covered {
                    continue;
                }
                merged.push(wildcard.clone());
            }
            sort_decoder_candidates(&mut merged);
            decoder_candidates_by_ext.insert(ext, merged);
        }

        Self {
            capabilities_by_plugin,
            capability_index,
            decoder_candidates_by_ext,
            decoder_candidates_wildcard,
        }
    }
}

fn collect_capabilities_from_lease(lease: &Arc<ModuleLease>) -> Vec<CapabilityDescriptor> {
    let lease_id = lease_id_of(lease);
    let mut out = Vec::new();
    let cap_count = (lease.loaded.module.capability_count)();
    for index in 0..cap_count {
        let desc_ptr = (lease.loaded.module.capability_get)(index);
        if desc_ptr.is_null() {
            continue;
        }
        let descriptor = unsafe { *desc_ptr };
        let type_id = unsafe { crate::util::ststr_to_string_lossy(descriptor.type_id_utf8) };
        if type_id.is_empty() {
            continue;
        }
        out.push(CapabilityDescriptor {
            lease_id,
            kind: CapabilityKind::from_st(descriptor.kind),
            type_id,
            display_name: unsafe {
                crate::util::ststr_to_string_lossy(descriptor.display_name_utf8)
            },
            config_schema_json: unsafe {
                crate::util::ststr_to_string_lossy(descriptor.config_schema_json_utf8)
            },
            default_config_json: unsafe {
                crate::util::ststr_to_string_lossy(descriptor.default_config_json_utf8)
            },
        });
    }
    out
}

fn decoder_score_rules_for_capability(module: &StPluginModule, type_id: &str) -> DecoderScoreRules {
    let Some(count_fn) = module.decoder_ext_score_count else {
        return DecoderScoreRules {
            exact_by_ext: HashMap::new(),
            wildcard: 1,
        };
    };
    let Some(get_fn) = module.decoder_ext_score_get else {
        return DecoderScoreRules {
            exact_by_ext: HashMap::new(),
            wildcard: 1,
        };
    };

    let type_id_st = ststr_from_str(type_id);
    let count = (count_fn)(type_id_st);
    let mut rules = DecoderScoreRules::default();
    for index in 0..count {
        let score_ptr = (get_fn)(type_id_st, index);
        if score_ptr.is_null() {
            continue;
        }
        let item = unsafe { *score_ptr };
        let rule = unsafe { crate::util::ststr_to_string_lossy(item.ext_utf8) };
        let rule = rule.trim().trim_start_matches('.').to_ascii_lowercase();
        if rule == "*" {
            rules.wildcard = rules.wildcard.max(item.score);
            continue;
        }
        if rule.is_empty() {
            continue;
        }
        rules
            .exact_by_ext
            .entry(rule)
            .and_modify(|score| *score = (*score).max(item.score))
            .or_insert(item.score);
    }
    rules
}

fn sort_decoder_candidates(candidates: &mut [DecoderCandidate]) {
    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.plugin_id.cmp(&b.plugin_id))
            .then_with(|| a.type_id.cmp(&b.type_id))
    });
}

fn ststr_from_str(s: &str) -> StStr {
    StStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}
