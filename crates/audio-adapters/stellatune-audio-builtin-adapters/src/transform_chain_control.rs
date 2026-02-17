use serde::Deserialize;
use stellatune_audio::pipeline::assembly::{OpaqueTransformStageSpec, PipelineMutation};
use stellatune_audio::pipeline::graph::{
    TransformGraphMutation, TransformPosition, TransformSegment,
};

pub const BUILTIN_TRANSFORM_CHAIN_SCHEMA_ID: &str = "stellatune.audio.transform.chain";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformChainStage {
    PreMix,
    Main,
    PostMix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinTransformChainEntry {
    pub stage: TransformChainStage,
    pub type_id: String,
    pub config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinTransformStagePayload {
    pub type_id: String,
    pub config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedBuiltinTransform {
    pub stage_key: String,
    pub type_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct TransformChainApplyPlan {
    pub mutations: Vec<PipelineMutation>,
    pub managed_stages: Vec<ManagedBuiltinTransform>,
}

#[derive(Debug, Clone, Default)]
pub struct BuiltinTransformChainLifecycle {
    managed_stages: Vec<ManagedBuiltinTransform>,
}

impl BuiltinTransformChainLifecycle {
    pub fn managed_stages(&self) -> &[ManagedBuiltinTransform] {
        &self.managed_stages
    }

    pub fn replace_transform_chain(
        &mut self,
        next: &[BuiltinTransformChainEntry],
    ) -> Vec<PipelineMutation> {
        let plan = plan_replace_managed_transform_chain(&self.managed_stages, next);
        self.managed_stages = plan.managed_stages.clone();
        plan.mutations
    }

    pub fn clear_pipeline(&mut self) -> Vec<PipelineMutation> {
        let mut mutations = Vec::with_capacity(self.managed_stages.len());
        for stage in self.managed_stages.drain(..).rev() {
            mutations.push(PipelineMutation::MutateTransformGraph {
                mutation: TransformGraphMutation::Remove {
                    target_stage_key: stage.stage_key,
                },
            });
        }
        mutations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct TransformChainPayloadItem {
    type_id: String,
    config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
struct TransformChainPayload {
    #[serde(default)]
    pre_mix: Vec<TransformChainPayloadItem>,
    #[serde(default)]
    main: Vec<TransformChainPayloadItem>,
    #[serde(default)]
    post_mix: Vec<TransformChainPayloadItem>,
}

pub fn parse_stage_control_payload(
    schema_id: &str,
    _revision: u64,
    payload: &[u8],
) -> Result<Option<Vec<BuiltinTransformChainEntry>>, String> {
    if schema_id != BUILTIN_TRANSFORM_CHAIN_SCHEMA_ID {
        return Ok(None);
    }

    let parsed: TransformChainPayload = serde_json::from_slice(payload)
        .map_err(|e| format!("invalid builtin transform chain payload json: {e}"))?;
    let mut entries =
        Vec::with_capacity(parsed.pre_mix.len() + parsed.main.len() + parsed.post_mix.len());
    entries.extend(
        parsed
            .pre_mix
            .into_iter()
            .map(|item| BuiltinTransformChainEntry {
                stage: TransformChainStage::PreMix,
                type_id: item.type_id,
                config_json: item.config_json,
            }),
    );
    entries.extend(
        parsed
            .main
            .into_iter()
            .map(|item| BuiltinTransformChainEntry {
                stage: TransformChainStage::Main,
                type_id: item.type_id,
                config_json: item.config_json,
            }),
    );
    entries.extend(
        parsed
            .post_mix
            .into_iter()
            .map(|item| BuiltinTransformChainEntry {
                stage: TransformChainStage::PostMix,
                type_id: item.type_id,
                config_json: item.config_json,
            }),
    );
    Ok(Some(entries))
}

pub fn plan_replace_managed_transform_chain(
    previous: &[ManagedBuiltinTransform],
    next: &[BuiltinTransformChainEntry],
) -> TransformChainApplyPlan {
    let mut mutations = Vec::with_capacity(previous.len().saturating_add(next.len()));
    for stage in previous.iter().rev() {
        mutations.push(PipelineMutation::MutateTransformGraph {
            mutation: TransformGraphMutation::Remove {
                target_stage_key: stage.stage_key.clone(),
            },
        });
    }

    let mut managed_stages = Vec::with_capacity(next.len());
    let mut pre_mix_index = 0usize;
    let mut main_index = 0usize;
    let mut post_mix_index = 0usize;
    for entry in next {
        let (segment, ordinal) = match entry.stage {
            TransformChainStage::PreMix => {
                let index = pre_mix_index;
                pre_mix_index = pre_mix_index.saturating_add(1);
                (TransformSegment::PreMix, index)
            },
            TransformChainStage::Main => {
                let index = main_index;
                main_index = main_index.saturating_add(1);
                (TransformSegment::Main, index)
            },
            TransformChainStage::PostMix => {
                let index = post_mix_index;
                post_mix_index = post_mix_index.saturating_add(1);
                (TransformSegment::PostMix, index)
            },
        };

        let stage_key = make_builtin_transform_stage_key(entry, ordinal);
        let payload = BuiltinTransformStagePayload {
            type_id: entry.type_id.clone(),
            config_json: entry.config_json.clone(),
        };
        mutations.push(PipelineMutation::MutateTransformGraph {
            mutation: TransformGraphMutation::Insert {
                segment,
                position: TransformPosition::Back,
                stage: OpaqueTransformStageSpec::with_payload(stage_key.clone(), payload),
            },
        });
        managed_stages.push(ManagedBuiltinTransform {
            stage_key,
            type_id: entry.type_id.clone(),
        });
    }

    TransformChainApplyPlan {
        mutations,
        managed_stages,
    }
}

fn make_builtin_transform_stage_key(entry: &BuiltinTransformChainEntry, ordinal: usize) -> String {
    let segment = match entry.stage {
        TransformChainStage::PreMix => "pre",
        TransformChainStage::Main => "main",
        TransformChainStage::PostMix => "post",
    };
    let type_id = sanitize_stage_key_fragment(&entry.type_id);
    format!("builtin.transform.{segment}.{type_id}.{ordinal}")
}

fn sanitize_stage_key_fragment(input: &str) -> String {
    input
        .trim()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        BUILTIN_TRANSFORM_CHAIN_SCHEMA_ID, BuiltinTransformChainEntry,
        BuiltinTransformChainLifecycle, TransformChainStage, parse_stage_control_payload,
        plan_replace_managed_transform_chain,
    };
    use stellatune_audio::pipeline::assembly::PipelineMutation;
    use stellatune_audio::pipeline::graph::{TransformGraphMutation, TransformSegment};

    #[test]
    fn parses_payload_into_staged_entries() {
        let payload = br#"{
            "pre_mix":[{"type_id":"eq","config_json":"{}"}],
            "main":[{"type_id":"compressor","config_json":"{}"}],
            "post_mix":[{"type_id":"limiter","config_json":"{}"}]
        }"#;
        let entries = parse_stage_control_payload(BUILTIN_TRANSFORM_CHAIN_SCHEMA_ID, 1, payload)
            .expect("parse failed")
            .expect("must match schema");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].stage, TransformChainStage::PreMix);
        assert_eq!(entries[1].stage, TransformChainStage::Main);
        assert_eq!(entries[2].stage, TransformChainStage::PostMix);
    }

    #[test]
    fn plan_replace_generates_remove_then_insert_mutations() {
        let previous = vec![super::ManagedBuiltinTransform {
            stage_key: "builtin.transform.main.eq.0".to_string(),
            type_id: "eq".to_string(),
        }];
        let next = vec![
            BuiltinTransformChainEntry {
                stage: TransformChainStage::PreMix,
                type_id: "eq".to_string(),
                config_json: "{}".to_string(),
            },
            BuiltinTransformChainEntry {
                stage: TransformChainStage::PostMix,
                type_id: "limiter".to_string(),
                config_json: "{}".to_string(),
            },
        ];

        let plan = plan_replace_managed_transform_chain(&previous, &next);
        assert_eq!(plan.mutations.len(), 3);
        match &plan.mutations[0] {
            PipelineMutation::MutateTransformGraph {
                mutation: TransformGraphMutation::Remove { target_stage_key },
            } => assert_eq!(target_stage_key, "builtin.transform.main.eq.0"),
            other => panic!("unexpected mutation[0]: {other:?}"),
        }
        match &plan.mutations[1] {
            PipelineMutation::MutateTransformGraph {
                mutation: TransformGraphMutation::Insert { segment, stage, .. },
            } => {
                assert_eq!(*segment, TransformSegment::PreMix);
                assert_eq!(stage.stage_key, "builtin.transform.pre.eq.0");
            },
            other => panic!("unexpected mutation[1]: {other:?}"),
        }
    }

    #[test]
    fn lifecycle_replace_and_clear_are_consistent() {
        let mut lifecycle = BuiltinTransformChainLifecycle::default();
        let mutations = lifecycle.replace_transform_chain(&[BuiltinTransformChainEntry {
            stage: TransformChainStage::Main,
            type_id: "eq".to_string(),
            config_json: "{}".to_string(),
        }]);
        assert_eq!(mutations.len(), 1);
        assert_eq!(lifecycle.managed_stages().len(), 1);

        let clear = lifecycle.clear_pipeline();
        assert_eq!(clear.len(), 1);
        assert!(lifecycle.managed_stages().is_empty());
    }
}
