use crate::{AudioBlock, AudioFormat, FactoryError, StageId, TransformError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformStatus {
    Produced,
    Buffered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainStatus {
    Produced,
    Complete,
}

pub trait TransformStage: Send {
    fn configure(&mut self, input: AudioFormat) -> Result<AudioFormat, TransformError>;
    fn process(&mut self, block: &mut AudioBlock) -> Result<TransformStatus, TransformError>;
    fn drain(&mut self, output: &mut AudioBlock) -> Result<DrainStatus, TransformError>;
    fn reset(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransformPlacement {
    PreMix,
    PostMix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformDescriptor {
    pub id: StageId,
    pub placement: TransformPlacement,
}

pub trait TransformFactory: Send + Sync {
    fn descriptor(&self) -> &TransformDescriptor;
    fn create(&self) -> Result<Box<dyn TransformStage>, FactoryError>;
}
