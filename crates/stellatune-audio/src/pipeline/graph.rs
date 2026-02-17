#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformSegment {
    PreMix,
    Main,
    PostMix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformPosition {
    Front,
    Back,
    Index(usize),
    Before(String),
    After(String),
}

pub trait TransformGraphStage {
    fn stage_key(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct TransformGraph<T> {
    pub pre_mix: Vec<T>,
    pub main: Vec<T>,
    pub post_mix: Vec<T>,
}

impl<T> Default for TransformGraph<T> {
    fn default() -> Self {
        Self {
            pre_mix: Vec::new(),
            main: Vec::new(),
            post_mix: Vec::new(),
        }
    }
}

impl<T> TransformGraph<T> {
    pub fn new(pre_mix: Vec<T>, main: Vec<T>, post_mix: Vec<T>) -> Self {
        Self {
            pre_mix,
            main,
            post_mix,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransformGraphMutation<T> {
    Insert {
        segment: TransformSegment,
        position: TransformPosition,
        stage: T,
    },
    Replace {
        target_stage_key: String,
        stage: T,
    },
    Remove {
        target_stage_key: String,
    },
    Move {
        target_stage_key: String,
        segment: TransformSegment,
        position: TransformPosition,
    },
}

impl<T> TransformGraph<T>
where
    T: TransformGraphStage,
{
    pub fn apply_mutation(&mut self, mutation: TransformGraphMutation<T>) -> Result<(), String> {
        match mutation {
            TransformGraphMutation::Insert {
                segment,
                position,
                stage,
            } => self.insert_stage(segment, position, stage),
            TransformGraphMutation::Replace {
                target_stage_key,
                stage,
            } => self.replace_stage(&target_stage_key, stage),
            TransformGraphMutation::Remove { target_stage_key } => {
                self.remove_stage(&target_stage_key).map(|_| ())
            },
            TransformGraphMutation::Move {
                target_stage_key,
                segment,
                position,
            } => self.move_stage(&target_stage_key, segment, position),
        }
    }

    pub fn apply_mutations<I>(&mut self, mutations: I) -> Result<(), String>
    where
        I: IntoIterator<Item = TransformGraphMutation<T>>,
    {
        for mutation in mutations {
            self.apply_mutation(mutation)?;
        }
        Ok(())
    }

    pub fn validate_unique_stage_keys(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::<String>::new();
        for key in self.all_stage_keys() {
            if !seen.insert(key.to_string()) {
                return Err(format!("duplicate stage key: {key}"));
            }
        }
        Ok(())
    }

    fn all_stage_keys(&self) -> impl Iterator<Item = &str> {
        self.pre_mix
            .iter()
            .chain(self.main.iter())
            .chain(self.post_mix.iter())
            .map(TransformGraphStage::stage_key)
    }

    fn insert_stage(
        &mut self,
        segment: TransformSegment,
        position: TransformPosition,
        stage: T,
    ) -> Result<(), String> {
        let stage_key = stage.stage_key().trim();
        if stage_key.is_empty() {
            return Err("stage key must not be empty".to_string());
        }
        if self.locate_stage(stage_key)?.is_some() {
            return Err(format!(
                "cannot insert stage '{stage_key}': stage key already exists"
            ));
        }
        let insert_at = self.resolve_insert_index(segment, &position, None)?;
        self.segment_mut(segment).insert(insert_at, stage);
        Ok(())
    }

    fn replace_stage(&mut self, target_stage_key: &str, stage: T) -> Result<(), String> {
        let Some((target_segment, target_index)) = self.locate_stage(target_stage_key)? else {
            return Err(format!("stage key not found: {target_stage_key}"));
        };
        let next_key = stage.stage_key().trim();
        if next_key.is_empty() {
            return Err("stage key must not be empty".to_string());
        }
        if next_key != target_stage_key && self.locate_stage(next_key)?.is_some() {
            return Err(format!(
                "cannot replace '{target_stage_key}' with '{next_key}': stage key already exists"
            ));
        }
        self.segment_mut(target_segment)[target_index] = stage;
        Ok(())
    }

    fn move_stage(
        &mut self,
        target_stage_key: &str,
        target_segment: TransformSegment,
        position: TransformPosition,
    ) -> Result<(), String> {
        let Some((source_segment, source_index)) = self.locate_stage(target_stage_key)? else {
            return Err(format!("stage key not found: {target_stage_key}"));
        };
        if matches!(&position, TransformPosition::Before(anchor) if anchor == target_stage_key)
            || matches!(&position, TransformPosition::After(anchor) if anchor == target_stage_key)
        {
            return Err("cannot move relative to itself".to_string());
        }

        let mut insert_at = self.resolve_insert_index(
            target_segment,
            &position,
            Some((source_segment, source_index)),
        )?;
        let stage = self.segment_mut(source_segment).remove(source_index);
        if source_segment == target_segment && source_index < insert_at {
            insert_at -= 1;
        }
        self.segment_mut(target_segment).insert(insert_at, stage);
        Ok(())
    }

    fn remove_stage(&mut self, stage_key: &str) -> Result<(T, TransformSegment, usize), String> {
        let Some((segment, index)) = self.locate_stage(stage_key)? else {
            return Err(format!("stage key not found: {stage_key}"));
        };
        let stage = self.segment_mut(segment).remove(index);
        Ok((stage, segment, index))
    }

    fn resolve_insert_index(
        &self,
        segment: TransformSegment,
        position: &TransformPosition,
        moving_from: Option<(TransformSegment, usize)>,
    ) -> Result<usize, String> {
        let segment_items = self.segment(segment);
        let len = segment_items.len();
        match position {
            TransformPosition::Front => Ok(0),
            TransformPosition::Back => Ok(len),
            TransformPosition::Index(index) => {
                if *index > len {
                    return Err(format!(
                        "index out of bounds in segment {segment:?}: {index} > {len}"
                    ));
                }
                Ok(*index)
            },
            TransformPosition::Before(anchor) => {
                let Some((anchor_segment, anchor_index)) = self.locate_stage(anchor)? else {
                    return Err(format!("anchor stage key not found: {anchor}"));
                };
                if anchor_segment != segment {
                    return Err(format!(
                        "anchor stage '{anchor}' is in segment {anchor_segment:?}, expected {segment:?}"
                    ));
                }
                if moving_from == Some((anchor_segment, anchor_index)) {
                    return Err("cannot move relative to itself".to_string());
                }
                Ok(anchor_index)
            },
            TransformPosition::After(anchor) => {
                let Some((anchor_segment, anchor_index)) = self.locate_stage(anchor)? else {
                    return Err(format!("anchor stage key not found: {anchor}"));
                };
                if anchor_segment != segment {
                    return Err(format!(
                        "anchor stage '{anchor}' is in segment {anchor_segment:?}, expected {segment:?}"
                    ));
                }
                if moving_from == Some((anchor_segment, anchor_index)) {
                    return Err("cannot move relative to itself".to_string());
                }
                Ok(anchor_index.saturating_add(1))
            },
        }
    }

    fn locate_stage(&self, stage_key: &str) -> Result<Option<(TransformSegment, usize)>, String> {
        let mut found: Option<(TransformSegment, usize)> = None;
        for (segment, items) in [
            (TransformSegment::PreMix, &self.pre_mix),
            (TransformSegment::Main, &self.main),
            (TransformSegment::PostMix, &self.post_mix),
        ] {
            for (index, item) in items.iter().enumerate() {
                if item.stage_key() != stage_key {
                    continue;
                }
                if found.is_some() {
                    return Err(format!(
                        "stage key '{stage_key}' appears multiple times in transform graph"
                    ));
                }
                found = Some((segment, index));
            }
        }
        Ok(found)
    }

    fn segment(&self, segment: TransformSegment) -> &Vec<T> {
        match segment {
            TransformSegment::PreMix => &self.pre_mix,
            TransformSegment::Main => &self.main,
            TransformSegment::PostMix => &self.post_mix,
        }
    }

    fn segment_mut(&mut self, segment: TransformSegment) -> &mut Vec<T> {
        match segment {
            TransformSegment::PreMix => &mut self.pre_mix,
            TransformSegment::Main => &mut self.main,
            TransformSegment::PostMix => &mut self.post_mix,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::pipeline::graph::{
        TransformGraph, TransformGraphMutation, TransformGraphStage, TransformPosition,
        TransformSegment,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestStage {
        key: String,
    }

    impl TestStage {
        fn new(key: &str) -> Self {
            Self {
                key: key.to_string(),
            }
        }
    }

    impl TransformGraphStage for TestStage {
        fn stage_key(&self) -> &str {
            &self.key
        }
    }

    fn keys(graph: &TransformGraph<TestStage>) -> (Vec<String>, Vec<String>, Vec<String>) {
        (
            graph.pre_mix.iter().map(|s| s.key.clone()).collect(),
            graph.main.iter().map(|s| s.key.clone()).collect(),
            graph.post_mix.iter().map(|s| s.key.clone()).collect(),
        )
    }

    #[test]
    fn replace_remove_move_and_insert_work_across_segments() {
        let mut graph = TransformGraph::new(
            vec![TestStage::new("pre-a"), TestStage::new("pre-b")],
            vec![TestStage::new("main-a"), TestStage::new("main-b")],
            vec![TestStage::new("post-a")],
        );

        graph
            .apply_mutation(TransformGraphMutation::Replace {
                target_stage_key: "main-b".to_string(),
                stage: TestStage::new("main-c"),
            })
            .expect("replace should succeed");
        graph
            .apply_mutation(TransformGraphMutation::Remove {
                target_stage_key: "pre-a".to_string(),
            })
            .expect("remove should succeed");
        graph
            .apply_mutation(TransformGraphMutation::Move {
                target_stage_key: "main-a".to_string(),
                segment: TransformSegment::PostMix,
                position: TransformPosition::Before("post-a".to_string()),
            })
            .expect("move should succeed");
        graph
            .apply_mutation(TransformGraphMutation::Insert {
                segment: TransformSegment::Main,
                position: TransformPosition::Front,
                stage: TestStage::new("main-z"),
            })
            .expect("insert should succeed");

        assert_eq!(
            keys(&graph),
            (
                vec!["pre-b".to_string()],
                vec!["main-z".to_string(), "main-c".to_string()],
                vec!["main-a".to_string(), "post-a".to_string()],
            )
        );
    }

    #[test]
    fn reject_duplicate_stage_keys_on_insert_and_replace() {
        let mut graph = TransformGraph::new(
            vec![TestStage::new("pre-a")],
            vec![TestStage::new("main-a")],
            Vec::new(),
        );

        let insert_duplicate = graph.apply_mutation(TransformGraphMutation::Insert {
            segment: TransformSegment::PostMix,
            position: TransformPosition::Back,
            stage: TestStage::new("main-a"),
        });
        assert!(insert_duplicate.is_err());

        let replace_duplicate = graph.apply_mutation(TransformGraphMutation::Replace {
            target_stage_key: "pre-a".to_string(),
            stage: TestStage::new("main-a"),
        });
        assert!(replace_duplicate.is_err());
    }

    #[test]
    fn move_within_same_segment_preserves_expected_order() {
        let mut graph = TransformGraph::new(
            Vec::new(),
            vec![
                TestStage::new("a"),
                TestStage::new("b"),
                TestStage::new("c"),
                TestStage::new("d"),
            ],
            Vec::new(),
        );

        graph
            .apply_mutation(TransformGraphMutation::Move {
                target_stage_key: "b".to_string(),
                segment: TransformSegment::Main,
                position: TransformPosition::After("d".to_string()),
            })
            .expect("move to tail should succeed");
        graph
            .apply_mutation(TransformGraphMutation::Move {
                target_stage_key: "d".to_string(),
                segment: TransformSegment::Main,
                position: TransformPosition::Front,
            })
            .expect("move to front should succeed");

        assert_eq!(
            keys(&graph).1,
            vec![
                "d".to_string(),
                "a".to_string(),
                "c".to_string(),
                "b".to_string(),
            ]
        );
    }
}
