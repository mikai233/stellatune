use crate::{AudioBlock, AudioFormat, FactoryError, SinkError, StageId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkWriteState {
    Ready,
    WouldBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkWriteResult {
    pub consumed_frames: usize,
    pub state: SinkWriteState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SinkClockSnapshot {
    pub consumed_frames: u64,
    pub buffered_frames: u64,
    pub epoch: u64,
}

pub trait SinkStage: Send {
    fn open(&mut self, format: AudioFormat) -> Result<(), SinkError>;
    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError>;
    fn pause(&mut self) -> Result<(), SinkError>;
    fn resume(&mut self) -> Result<(), SinkError>;
    fn drain(&mut self) -> Result<(), SinkError>;
    fn discard(&mut self) -> Result<(), SinkError>;
    fn clock_snapshot(&self) -> SinkClockSnapshot;
    fn close(&mut self);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutputCompatibilityKey {
    pub backend_id: String,
    pub device_id: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    pub route_revision: u64,
}

pub trait SinkFactory: Send + Sync {
    fn id(&self) -> &StageId;
    fn preferred_format(&self, input: AudioFormat) -> Result<AudioFormat, FactoryError> {
        Ok(input)
    }
    fn compatibility_key(
        &self,
        format: AudioFormat,
    ) -> Result<OutputCompatibilityKey, FactoryError>;
    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError>;
}
