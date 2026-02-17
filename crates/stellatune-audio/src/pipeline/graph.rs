//! Transform graph model and mutation helpers.

use thiserror::Error;

/// Logical transform segment in the decode pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformSegment {
    /// Segment before mixer/resampler.
    PreMix,
    /// Main transform segment.
    Main,
    /// Segment after mixer/resampler.
    PostMix,
}

/// Insert/move target position within a transform segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformPosition {
    /// Insert at segment front.
    Front,
    /// Insert at segment back.
    Back,
    /// Insert at explicit index.
    Index(usize),
    /// Insert before the specified stage key.
    Before(String),
    /// Insert after the specified stage key.
    After(String),
}

/// Trait required by transform graph entries.
pub trait TransformGraphStage {
    /// Returns the stable stage key.
    fn stage_key(&self) -> &str;
}

/// Three-segment transform graph.
#[derive(Debug, Clone)]
pub struct TransformGraph<T> {
    /// Pre-mix segment.
    pub pre_mix: Vec<T>,
    /// Main segment.
    pub main: Vec<T>,
    /// Post-mix segment.
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
    /// Creates a graph from explicit segment vectors.
    pub fn new(pre_mix: Vec<T>, main: Vec<T>, post_mix: Vec<T>) -> Self {
        Self {
            pre_mix,
            main,
            post_mix,
        }
    }
}

/// Mutable transform graph operation.
#[derive(Debug, Clone)]
pub enum TransformGraphMutation<T> {
    /// Inserts a stage into the selected segment and position.
    Insert {
        /// Target segment.
        segment: TransformSegment,
        /// Target position within the segment.
        position: TransformPosition,
        /// Stage payload.
        stage: T,
    },
    /// Replaces an existing stage identified by key.
    Replace {
        /// Target stage key.
        target_stage_key: String,
        /// Replacement stage payload.
        stage: T,
    },
    /// Removes an existing stage identified by key.
    Remove {
        /// Target stage key.
        target_stage_key: String,
    },
    /// Moves an existing stage to another segment/position.
    Move {
        /// Stage key to move.
        target_stage_key: String,
        /// Destination segment.
        segment: TransformSegment,
        /// Destination position.
        position: TransformPosition,
    },
}

/// Transform graph mutation/validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransformGraphError {
    /// Stage key is empty.
    #[error("stage key must not be empty")]
    StageKeyMustNotBeEmpty,
    /// Duplicate stage key exists in graph.
    #[error("duplicate stage key: {stage_key}")]
    DuplicateStageKey {
        /// Duplicate stage key.
        stage_key: String,
    },
    /// Insert attempted with an already existing stage key.
    #[error("cannot insert stage '{stage_key}': stage key already exists")]
    CannotInsertExistingStageKey {
        /// Existing conflicting stage key.
        stage_key: String,
    },
    /// Replace attempted with a stage key that exists elsewhere in graph.
    #[error(
        "cannot replace '{target_stage_key}' with '{next_stage_key}': stage key already exists"
    )]
    CannotReplaceWithExistingStageKey {
        /// Stage key being replaced.
        target_stage_key: String,
        /// New stage key that conflicts with an existing entry.
        next_stage_key: String,
    },
    /// Requested stage key does not exist.
    #[error("stage key not found: {stage_key}")]
    StageKeyNotFound {
        /// Missing stage key.
        stage_key: String,
    },
    /// Requested anchor key does not exist.
    #[error("anchor stage key not found: {anchor}")]
    AnchorStageKeyNotFound {
        /// Missing anchor key.
        anchor: String,
    },
    /// Anchor exists in a different segment than expected.
    #[error(
        "anchor stage '{anchor}' is in segment {anchor_segment:?}, expected {expected_segment:?}"
    )]
    AnchorSegmentMismatch {
        /// Anchor stage key.
        anchor: String,
        /// Segment where the anchor was found.
        anchor_segment: TransformSegment,
        /// Segment required by the operation.
        expected_segment: TransformSegment,
    },
    /// Relative move requested with the same source and anchor stage.
    #[error("cannot move relative to itself")]
    CannotMoveRelativeToItself,
    /// Insert/move index exceeded segment bounds.
    #[error("index out of bounds in segment {segment:?}: {index} > {len}")]
    IndexOutOfBounds {
        /// Segment involved in index resolution.
        segment: TransformSegment,
        /// Requested index.
        index: usize,
        /// Segment length.
        len: usize,
    },
    /// Validation found the same stage key multiple times.
    #[error("stage key '{stage_key}' appears multiple times in transform graph")]
    StageKeyAppearsMultipleTimes {
        /// Duplicated stage key.
        stage_key: String,
    },
}

impl<T> TransformGraph<T>
where
    T: TransformGraphStage,
{
    /// Applies a single graph mutation.
    pub fn apply_mutation(
        &mut self,
        mutation: TransformGraphMutation<T>,
    ) -> Result<(), TransformGraphError> {
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

    /// Applies multiple mutations in order.
    pub fn apply_mutations<I>(&mut self, mutations: I) -> Result<(), TransformGraphError>
    where
        I: IntoIterator<Item = TransformGraphMutation<T>>,
    {
        for mutation in mutations {
            self.apply_mutation(mutation)?;
        }
        Ok(())
    }

    /// Verifies that each stage key appears exactly once.
    pub fn validate_unique_stage_keys(&self) -> Result<(), TransformGraphError> {
        let mut seen = std::collections::HashSet::<String>::new();
        for key in self.all_stage_keys() {
            if !seen.insert(key.to_string()) {
                return Err(TransformGraphError::DuplicateStageKey {
                    stage_key: key.to_string(),
                });
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
    ) -> Result<(), TransformGraphError> {
        let stage_key = stage.stage_key().trim();
        if stage_key.is_empty() {
            return Err(TransformGraphError::StageKeyMustNotBeEmpty);
        }
        if self.locate_stage(stage_key)?.is_some() {
            return Err(TransformGraphError::CannotInsertExistingStageKey {
                stage_key: stage_key.to_string(),
            });
        }
        let insert_at = self.resolve_insert_index(segment, &position, None)?;
        self.segment_mut(segment).insert(insert_at, stage);
        Ok(())
    }

    fn replace_stage(
        &mut self,
        target_stage_key: &str,
        stage: T,
    ) -> Result<(), TransformGraphError> {
        let Some((target_segment, target_index)) = self.locate_stage(target_stage_key)? else {
            return Err(TransformGraphError::StageKeyNotFound {
                stage_key: target_stage_key.to_string(),
            });
        };
        let next_key = stage.stage_key().trim();
        if next_key.is_empty() {
            return Err(TransformGraphError::StageKeyMustNotBeEmpty);
        }
        if next_key != target_stage_key && self.locate_stage(next_key)?.is_some() {
            return Err(TransformGraphError::CannotReplaceWithExistingStageKey {
                target_stage_key: target_stage_key.to_string(),
                next_stage_key: next_key.to_string(),
            });
        }
        self.segment_mut(target_segment)[target_index] = stage;
        Ok(())
    }

    fn move_stage(
        &mut self,
        target_stage_key: &str,
        target_segment: TransformSegment,
        position: TransformPosition,
    ) -> Result<(), TransformGraphError> {
        let Some((source_segment, source_index)) = self.locate_stage(target_stage_key)? else {
            return Err(TransformGraphError::StageKeyNotFound {
                stage_key: target_stage_key.to_string(),
            });
        };
        if matches!(&position, TransformPosition::Before(anchor) if anchor == target_stage_key)
            || matches!(&position, TransformPosition::After(anchor) if anchor == target_stage_key)
        {
            return Err(TransformGraphError::CannotMoveRelativeToItself);
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

    fn remove_stage(
        &mut self,
        stage_key: &str,
    ) -> Result<(T, TransformSegment, usize), TransformGraphError> {
        let Some((segment, index)) = self.locate_stage(stage_key)? else {
            return Err(TransformGraphError::StageKeyNotFound {
                stage_key: stage_key.to_string(),
            });
        };
        let stage = self.segment_mut(segment).remove(index);
        Ok((stage, segment, index))
    }

    fn resolve_insert_index(
        &self,
        segment: TransformSegment,
        position: &TransformPosition,
        moving_from: Option<(TransformSegment, usize)>,
    ) -> Result<usize, TransformGraphError> {
        let segment_items = self.segment(segment);
        let len = segment_items.len();
        match position {
            TransformPosition::Front => Ok(0),
            TransformPosition::Back => Ok(len),
            TransformPosition::Index(index) => {
                if *index > len {
                    return Err(TransformGraphError::IndexOutOfBounds {
                        segment,
                        index: *index,
                        len,
                    });
                }
                Ok(*index)
            },
            TransformPosition::Before(anchor) => {
                let Some((anchor_segment, anchor_index)) = self.locate_stage(anchor)? else {
                    return Err(TransformGraphError::AnchorStageKeyNotFound {
                        anchor: anchor.to_string(),
                    });
                };
                if anchor_segment != segment {
                    return Err(TransformGraphError::AnchorSegmentMismatch {
                        anchor: anchor.to_string(),
                        anchor_segment,
                        expected_segment: segment,
                    });
                }
                if moving_from == Some((anchor_segment, anchor_index)) {
                    return Err(TransformGraphError::CannotMoveRelativeToItself);
                }
                Ok(anchor_index)
            },
            TransformPosition::After(anchor) => {
                let Some((anchor_segment, anchor_index)) = self.locate_stage(anchor)? else {
                    return Err(TransformGraphError::AnchorStageKeyNotFound {
                        anchor: anchor.to_string(),
                    });
                };
                if anchor_segment != segment {
                    return Err(TransformGraphError::AnchorSegmentMismatch {
                        anchor: anchor.to_string(),
                        anchor_segment,
                        expected_segment: segment,
                    });
                }
                if moving_from == Some((anchor_segment, anchor_index)) {
                    return Err(TransformGraphError::CannotMoveRelativeToItself);
                }
                Ok(anchor_index.saturating_add(1))
            },
        }
    }

    fn locate_stage(
        &self,
        stage_key: &str,
    ) -> Result<Option<(TransformSegment, usize)>, TransformGraphError> {
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
                    return Err(TransformGraphError::StageKeyAppearsMultipleTimes {
                        stage_key: stage_key.to_string(),
                    });
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
